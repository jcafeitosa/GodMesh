//! Integração local do portão de abuso (cotas, payload, admissão por classe).
//!
//! Cobre o backlog nº 1 e nº 2 de `docs/04-seguranca/abuso-e-malha-maligna.md` §4:
//! um grant válido não pode inundar o servidor, e o tráfego de emergência não
//! pode ser consumido pelo tráfego normal.
//!
//! O que é provado aqui, no fio:
//!  * corpo acima do teto declarado → **413 `payload_too_large`**, sem INSERT;
//!  * rajada acima da cota → **429 `rate_limited`** com `Retry-After`, sem efeito;
//!  * cota de eventos por lote → **429** antes do handler (nada parcial);
//!  * sob pressão, classe baixa → **503 `admission_control`**, enquanto
//!    **SOS continua 201** (invariante);
//!  * bucket por *(tenant, token, classe)*: um tenant não esgota o outro, e o
//!    flood de uma classe não consome a cota de SOS;
//!  * requisição não autenticada limitada por IP (com `ConnectInfo`);
//!  * timeout por rota → **503 `request_timeout`** com `effect: unknown` e
//!    reenvio com a mesma idempotência (leitura da RFC 9110 em `gate`);
//!  * teto configurado acima de 2 MiB é respeitado também pelo extrator do
//!    axum (`DefaultBodyLimit` alinhado) — nenhuma recusa vem do framework.
//!
//! O que NÃO é provado aqui (declarado na evidência): cota é **por processo**,
//! não distribuída; não há teste de carga real.
//!
//! Banco: um dedicado à suíte, com tenants próprios `tenant-limits-*` e ids
//! únicos por teste (o runner oficial aplica o esquema via `tests/common`):
//!   psql ... -c 'CREATE DATABASE godmesh_limits'
//!   DATABASE_URL=postgres://.../godmesh_limits cargo test -p godmesh-server --test limits

mod common;

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::admission::{AdmissionConfig, RequestClass, ScopeLimits};
use godmesh_server::auth::{GrantRegistry, Scope, Subject, VerifiedGrant};
use serde_json::{Value, json};
use sqlx::PgPool;

const CLASS: &str = "mensagem";
const TENANT_EVENTOS: &str = "tenant-limits-eventos";
const TENANT_SYNC: &str = "tenant-limits-sync";
const TENANT_A: &str = "tenant-limits-a";
const TENANT_B: &str = "tenant-limits-b";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn allow_predicate() -> GrantPredicate {
    GrantPredicate {
        operation: OfflineOp::RecordLocalObservation,
        epoch_comparable: true,
        revocation: RevocationState::Absent,
        freshness: FreshnessSource::F3VerifiedOnline,
        retention_class_known: true,
        action_enumerated: true,
    }
}

fn grant(tenant: &str, token_id: &str, scopes: Vec<Scope>) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant.to_owned(),
            principal: format!("principal-{token_id}"),
        },
        predicate: allow_predicate(),
        token_id: token_id.to_owned(),
        scopes,
        authority_epoch: 1,
    }
}

/// Um registro com um token por papel. Os tokens são literais de teste — a
/// cota é a mesma máquina de estados para tokens emitidos pela API.
struct Tokens {
    eventos: String,
    sync: String,
    metrics: String,
    a: String,
    b: String,
}

fn registry() -> (GrantRegistry, Tokens) {
    let tokens = Tokens {
        eventos: unique("grant-eventos"),
        sync: unique("grant-sync"),
        metrics: unique("grant-metrics"),
        a: unique("grant-a"),
        b: unique("grant-b"),
    };
    let registry = GrantRegistry::from_entries(vec![
        (
            tokens.eventos.clone(),
            grant(TENANT_EVENTOS, "tok-eventos", vec![Scope::EventWrite, Scope::EventRead]),
        ),
        (
            tokens.sync.clone(),
            grant(
                TENANT_SYNC,
                "tok-sync",
                vec![Scope::SyncWrite, Scope::SyncRead],
            ),
        ),
        (
            tokens.metrics.clone(),
            grant(TENANT_SYNC, "tok-metrics", vec![Scope::AdminMetrics]),
        ),
        (
            tokens.a.clone(),
            grant(TENANT_A, "tok-a", vec![Scope::IncidentWrite]),
        ),
        (
            tokens.b.clone(),
            grant(TENANT_B, "tok-b", vec![Scope::IncidentWrite]),
        ),
    ]);
    (registry, tokens)
}

async fn declare_tenant(pool: &PgPool, tenant: &str) {
    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = 'BR'")
        .bind(tenant)
        .execute(pool)
        .await
        .expect("jurisdição declarada");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ($1, 'BR', 'historico', 24, 'expiry') ON CONFLICT DO NOTHING",
    )
    .bind(CLASS)
    .execute(pool)
    .await
    .expect("classe semeada");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ($1, 'BR', 'primary') ON CONFLICT DO NOTHING",
    )
    .bind(CLASS)
    .execute(pool)
    .await
    .expect("cópia semeada");
}

/// Servidor com configuração explícita do portão (sem tocar no ambiente).
async fn serve(
    pool: PgPool,
    grants: GrantRegistry,
    config: AdmissionConfig,
) -> SocketAddr {
    serve_inner(pool, grants, config, false).await
}

/// Igual a [`serve`], mas expondo `ConnectInfo` — é o que a produção tem, via
/// `runtime::serve`, e o que habilita a cota por IP.
async fn serve_with_connect_info(
    pool: PgPool,
    grants: GrantRegistry,
    config: AdmissionConfig,
) -> SocketAddr {
    serve_inner(pool, grants, config, true).await
}

async fn serve_inner(
    pool: PgPool,
    grants: GrantRegistry,
    config: AdmissionConfig,
    connect_info: bool,
) -> SocketAddr {
    for tenant in [TENANT_EVENTOS, TENANT_SYNC, TENANT_A, TENANT_B] {
        declare_tenant(&pool, tenant).await;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router_with_admission(pool, grants, config);
    tokio::spawn(async move {
        if connect_info {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        } else {
            let _ = axum::serve(listener, app).await;
        }
    });
    addr
}

// --- cliente HTTP/1.1 cru, com headers e acesso ao Retry-After ---------------

struct Reply {
    status: u16,
    headers: String,
    body: Value,
}

impl Reply {
    fn error(&self) -> &str {
        self.body["error"].as_str().unwrap_or("<sem campo error>")
    }

    fn retry_after(&self) -> Option<u64> {
        self.headers
            .lines()
            .find(|line| line.to_ascii_lowercase().starts_with("retry-after:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
    }
}

fn send(addr: SocketAddr, request: String) -> Reply {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de escrita");
    // O servidor pode encerrar cedo (413 no meio de um corpo chunked); escrever
    // em pedaços quebrados é parte do teste, não um erro dele. `Connection:
    // close` encerra por si — não dar `shutdown(Write)` primeiro: hyper mata a
    // conexão no EOF de leitura antes de responder (comportamento observado,
    // inclusive em `/healthz` sem portão nenhum).
    let _ = stream.write_all(request.as_bytes());
    let mut raw = String::new();
    let leu = stream.read_to_string(&mut raw);
    if leu.unwrap_or(0) == 0 {
        panic!("resposta vazia do servidor");
    }
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("resposta HTTP sem headers: {raw:?}"));
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta sem status: {raw:?}"));
    let body: Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!("corpo não é JSON ({e}): {body:?} (status {status})")
    });
    Reply {
        status,
        headers: head.to_owned(),
        body,
    }
}

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    class: Option<&str>,
    body: Option<&str>,
) -> String {
    let mut out = format!("{method} {path} HTTP/1.1\r\nHost: teste\r\n");
    if let Some(token) = token {
        out.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    if let Some(class) = class {
        out.push_str(&format!("x-godmesh-class: {class}\r\n"));
    }
    if let Some(body) = body {
        out.push_str("Content-Type: application/json\r\n");
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        out.push_str(body);
    }
    out
}

/// Pedido que **declara** um corpo acima do teto via `Content-Length` sem
/// entregá-lo. Não enviar o corpo integral é deliberado: ao responder 413 sem
/// ler o corpo declarado, o servidor fecha a conexão com bytes não lidos e o
/// kernel manda RST — que pode engolir a resposta antes do teste lê-la. Este é
/// o comportamento HTTP padrão de recusa por tamanho (o nginx faz o mesmo); um
/// cliente real interrompe o envio quando a resposta chega. O caminho chunked,
/// onde o corpo flui de verdade, é provado no teste próprio.
fn declared_oversized(method: &str, path: &str, token: &str, declared_len: usize) -> String {
    format!(
        "{method} {path} HTTP/1.1\r\nHost: teste\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {declared_len}\r\nConnection: close\r\n\r\n"
    )
}

async fn send_async(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    class: Option<&str>,
    body: Option<String>,
) -> Reply {
    let raw = request(method, path, token, class, body.as_deref());
    tokio::task::spawn_blocking(move || send(addr, raw))
        .await
        .expect("cliente HTTP")
}

fn event_body(stream: &str, seq: u64) -> String {
    // S-sec-2: corpo é envelope assinado pelo nó local de teste. A origem da
    // posição é o próprio assinante; o tenant vem do grant do token `eventos`.
    common::envelope_assinado(&common::LOCAL_NODE, TENANT_EVENTOS, stream, 1, 7, seq, CLASS)
}

fn inbox_body(stream: &str, origin: &str, seqs: &[u64]) -> String {
    let positions: Vec<Value> = seqs
        .iter()
        .map(|s| {
            json!({
                "origin_node": origin,
                "stream_id": stream,
                "partition": 1,
                "authority_epoch": 7,
                "seq": *s,
            })
        })
        .collect();
    json!({"retention_class": CLASS, "positions": positions}).to_string()
}

async fn event_rows(pool: &PgPool, tenant: &str, stream: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM event_position WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(tenant)
    .bind(stream)
    .fetch_one(pool)
    .await
    .expect("contagem de event_position")
}

async fn incident_rows(pool: &PgPool, incident_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(incident_id)
        .fetch_one(pool)
        .await
        .expect("contagem de incident")
}

/// Configuração com cotas baixas apenas onde o teste precisa; o resto fica no
/// default documentado.
fn config_with(scope: Scope, limits: ScopeLimits) -> AdmissionConfig {
    let mut c = AdmissionConfig::default();
    c.per_scope.insert(scope, limits);
    c
}

fn tight_limits() -> ScopeLimits {
    ScopeLimits {
        requests_per_sec: 1.0,
        request_burst: 2.0,
        events_per_sec: 1.0,
        event_burst: 2.0,
    }
}

// ---------------------------------------------------------------------------
// 1. Payload
// ---------------------------------------------------------------------------

/// 413 **antes** de desserializar: um corpo que nem é JSON possível, maior do
/// que o teto, não pode chegar ao handler — e, portanto, não pode gravar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn corpo_acima_do_teto_de_evento_e_413_payload_too_large_sem_insert() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(pool.clone(), grants, AdmissionConfig::default()).await;
    let stream = unique("stream-413");

    let reply = send(addr, declared_oversized("POST", "/v1/events", &tokens.eventos, 70 * 1024));

    assert_eq!(reply.status, 413, "corpo: {}", reply.body);
    assert_eq!(reply.error(), "payload_too_large");
    assert_eq!(reply.body["ack_class"], "transport_accepted");
    assert_eq!(
        event_rows(&pool, TENANT_EVENTOS, &stream).await,
        0,
        "413 não pode deixar INSERT"
    );
}

/// O teto é **por rota**: 200 KiB passa no inbox (256 KiB) e estoura no evento
/// (64 KiB). O corpo de 200 KiB aqui é JSON válido com classe de retenção
/// desconhecida — o que se prova é o código de chegada, não a semântica.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn teto_de_payload_e_por_rota_inbox_tem_256_kib_evento_64_kib() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(pool.clone(), grants, AdmissionConfig::default()).await;

    let pad = "r".repeat(200 * 1024);
    let corpo_200k = json!({"retention_class": pad, "positions": []}).to_string();
    assert!(corpo_200k.len() > 200 * 1024 && corpo_200k.len() < 256 * 1024);

    // Na rota de evento o MESMO corpo estoura o teto de 64 KiB.
    let reply_eventos = send(
        addr,
        declared_oversized("POST", "/v1/events", &tokens.eventos, corpo_200k.len()),
    );
    assert_eq!(reply_eventos.status, 413);
    assert_eq!(reply_eventos.error(), "payload_too_large");

    // No inbox, 200 KiB passa pelo portão: a recusa que vem é do handler
    // (classe de retenção inexistente), nunca 413.
    let reply_inbox =
        send_async(addr, "POST", "/v1/sync/inbox", Some(&tokens.sync), None, Some(corpo_200k)).await;
    assert_ne!(reply_inbox.status, 413, "inbox admite até 256 KiB: {:?}", reply_inbox.body);

    // E acima de 256 KiB, o inbox também cai no teto, antes do handler.
    let reply_grande = send(
        addr,
        declared_oversized("POST", "/v1/sync/inbox", &tokens.sync, 300 * 1024),
    );
    assert_eq!(reply_grande.status, 413);
    assert_eq!(reply_grande.error(), "payload_too_large");
}

/// Sem `Content-Length` (chunked) o corpo é lido com teto — a recusa continua
/// sendo 413 antes da desserialização, não um estouro de memória.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn corpo_chunked_acima_do_teto_e_413_sem_content_length() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(pool.clone(), grants, AdmissionConfig::default()).await;

    let payload = "z".repeat(80 * 1024);
    let mut chunked = format!(
        "POST /v1/events HTTP/1.1\r\nHost: teste\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
        tokens.eventos
    );
    // Chunks até estourar o teto — sem finalizar o stream: o portão responde
    // 413 no meio do corpo, e o encerramento precoce é parte do teste.
    for slice in payload.as_bytes().chunks(16 * 1024) {
        chunked.push_str(&format!("{:x}\r\n", slice.len()));
        chunked.push_str(std::str::from_utf8(slice).expect("ascii"));
        chunked.push_str("\r\n");
    }

    let reply = tokio::task::spawn_blocking(move || send(addr, chunked))
        .await
        .expect("cliente HTTP");
    assert_eq!(reply.status, 413, "chunked acima do teto: {}", reply.body);
    assert_eq!(reply.error(), "payload_too_large");
}

/// O teto **configurado** acima de 2 MiB tem que valer de ponta a ponta. O axum
/// impõe 2 MiB por conta própria aos extratores `Json`/`Bytes` (`DefaultBodyLimit`)
/// e responde com corpo **dele**, fora do contrato — o portão alinha o teto do
/// framework ao teto declarado (`extractor_body_limit`). Prova no fio:
///  * corpo de 2,5 MiB **dentro** do teto de 3 MiB chega ao handler (a recusa
///    que vem é de domínio — `retention_policy_missing` — nunca de tamanho);
///  * corpo **acima** do teto de 3 MiB cai no portão, ainda em chunked, e a
///    recusa continua no formato do contrato `{error, detail, hint, ack_class}`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn teto_acima_de_2_mib_e_respeitado_e_a_recusa_continua_no_formato_do_contrato() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let config = AdmissionConfig {
        max_inbox_body_bytes: 3 * 1024 * 1024,
        ..AdmissionConfig::default()
    };
    let addr = serve(pool.clone(), grants, config).await;

    // 2,5 MiB: acima do teto silencioso do axum, dentro do teto declarado.
    // A classe de retenção do envelope é um padding de 2,5 MiB: a recusa de
    // domínio `retention_policy_missing` (409) só existe DEPOIS do `Json` do
    // axum ter desserializado o corpo inteiro — prova de que o extrator aceitou
    // o tamanho. Se o teto silencioso de 2 MiB do framework ainda valesse, a
    // resposta seria uma rejeição de corpo do próprio axum, nunca 409.
    let mut corpo_2m5 = inbox_body(&unique("stream-2mib"), "node-limits", &[1]);
    corpo_2m5 = corpo_2m5.replace(
        &format!("\"retention_class\":\"{CLASS}\""),
        &format!("\"retention_class\":\"{}\"", "r".repeat(2_600_000)),
    );
    assert!(corpo_2m5.len() > 2 * 1024 * 1024 && corpo_2m5.len() < 3 * 1024 * 1024);
    let reply = send_async(
        addr,
        "POST",
        "/v1/sync/inbox",
        Some(&tokens.sync),
        None,
        Some(corpo_2m5),
    )
    .await;
    assert_ne!(reply.status, 413, "dentro do teto declarado não pode haver 413: {:?}", reply.body);
    assert_eq!(
        reply.error(),
        "retention_policy_missing",
        "o corpo tem que chegar ao handler, não ser recusado pelo extrator do axum: {:?}",
        reply.body
    );

    // 3 MiB + 1 declarado: acima do teto configurado — recusa do portão, no
    // formato do contrato, com `hint` acionável (ADR-074). `declared_oversized`
    // evita o RST engolir a resposta quando o corpo ainda estaria por vir.
    let reply = send(
        addr,
        declared_oversized("POST", "/v1/sync/inbox", &tokens.sync, 3 * 1024 * 1024 + 1),
    );
    assert_eq!(reply.status, 413, "acima do teto de 3 MiB: {:?}", reply.body);
    assert_eq!(reply.error(), "payload_too_large");
    assert!(reply.body["detail"].is_string(), "detail presente: {:?}", reply.body);
    assert!(
        reply.body["hint"].as_str().is_some_and(|h| !h.is_empty()),
        "hint acionável no corpo da recusa: {:?}",
        reply.body
    );
    assert_eq!(reply.body["ack_class"], "transport_accepted");
}

// ---------------------------------------------------------------------------
// 2. Cotas
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rajada_acima_da_cota_de_requisicoes_e_429_com_retry_after_sem_efecto() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(pool.clone(), grants, config_with(Scope::EventWrite, tight_limits())).await;
    let stream = unique("stream-cota");
    common::no_local_pronto(&pool, TENANT_EVENTOS, &stream).await;

    let r1 = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 1))).await;
    let r2 = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 2))).await;
    assert_eq!((r1.status, r2.status), (201, 201), "burst 2: {:?}", (r1.body, r2.body));

    // A terceira, imediatamente, estoura o bucket vazio.
    let r3 = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 3))).await;
    assert_eq!(r3.status, 429, "corpo: {}", r3.body);
    assert_eq!(r3.error(), "rate_limited");
    let retry = r3.retry_after().expect("429 precisa de Retry-After");
    assert!(retry >= 1, "Retry-After em segundos: {retry}");
    assert_eq!(r3.body["ack_class"], "transport_accepted");

    assert_eq!(
        event_rows(&pool, TENANT_EVENTOS, &stream).await,
        2,
        "a requisição recusada não pode ter escrito a posição 3"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM event_position WHERE tenant_id = $1 AND stream_id = $2 AND seq = 3",
        )
        .bind(TENANT_EVENTOS)
        .bind(&stream)
        .fetch_one(&pool)
        .await
        .expect("contagem da posição 3"),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lote_grande_esgota_a_cota_de_eventos_antes_do_handler() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    // Cota de eventos do sync: 10/s com burst 10 — um lote de 20 posições custa
    // 20 e é recusado por inteiro, sem aceitação parcial.
    let addr = serve(
        pool.clone(),
        grants,
        config_with(
            Scope::SyncWrite,
            ScopeLimits {
                requests_per_sec: 100.0,
                request_burst: 100.0,
                events_per_sec: 10.0,
                event_burst: 10.0,
            },
        ),
    )
    .await;
    let (stream, origin) = (unique("stream-eps"), unique("origin-eps"));

    let body = inbox_body(&stream, &origin, &(1..=20).collect::<Vec<_>>());
    let reply = send_async(addr, "POST", "/v1/sync/inbox", Some(&tokens.sync), None, Some(body)).await;
    assert_eq!(reply.status, 429, "lote de 20 com burst 10: {}", reply.body);
    assert_eq!(reply.error(), "rate_limited");
    assert!(reply.retry_after().is_some_and(|v| v >= 1));
    assert_eq!(
        event_rows(&pool, TENANT_SYNC, &stream).await,
        0,
        "cota de eventos recusa o lote inteiro, não um prefixo dele"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cota_de_um_tenant_nao_afeta_outro_tenant() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(
        pool.clone(),
        grants,
        config_with(
            Scope::IncidentWrite,
            ScopeLimits {
                requests_per_sec: 1.0,
                request_burst: 1.0,
                events_per_sec: 100.0,
                event_burst: 100.0,
            },
        ),
    )
    .await;

    let id_a1 = unique("inc-a1");
    let id_a2 = unique("inc-a2");
    let id_b1 = unique("inc-b1");

    let r1 = send_async(addr, "POST", "/v1/incidents", Some(&tokens.a), None, Some(json!({"incident_id": id_a1}).to_string())).await;
    assert_eq!(r1.status, 201, "{:?}", r1.body);
    let r2 = send_async(addr, "POST", "/v1/incidents", Some(&tokens.a), None, Some(json!({"incident_id": id_a2}).to_string())).await;
    assert_eq!(r2.status, 429, "tenant A esgotou a própria cota: {:?}", r2.body);
    assert_eq!(r2.error(), "rate_limited");

    // O mesmo burst no tenant B não sente o bucket do tenant A.
    let r3 = send_async(addr, "POST", "/v1/incidents", Some(&tokens.b), None, Some(json!({"incident_id": id_b1}).to_string())).await;
    assert_eq!(r3.status, 201, "tenant B segue livre: {:?}", r3.body);

    // E a recusada por cota não existe no banco.
    assert_eq!(incident_rows(&pool, &id_a2).await, 0);
}

// ---------------------------------------------------------------------------
// 3. Admissão por classe
// ---------------------------------------------------------------------------

/// O invariante que importa: sob pressão declarada (limiar 0 = sempre), as
/// classes baixas são recusadas com 503 e **SOS continua 201**.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sob_pressao_classe_baixa_e_503_enquanto_sos_continua_201() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let config = AdmissionConfig { pressure_threshold: 0, ..AdmissionConfig::default() };
    let addr = serve(pool.clone(), grants, config).await;

    // Sincronização (classe baixa): recusada, sem efeito.
    let sync_state = send_async(addr, "GET", "/v1/sync/state", Some(&tokens.sync), None, None).await;
    assert_eq!(sync_state.status, 503, "{:?}", sync_state.body);
    assert_eq!(sync_state.error(), "admission_control");
    assert!(sync_state.retry_after().is_some_and(|v| v >= 1));

    // Métricas idem.
    let metrics = send_async(addr, "GET", "/metrics", Some(&tokens.metrics), None, None).await;
    assert_eq!(metrics.status, 503);
    assert_eq!(metrics.error(), "admission_control");

    // Evento (classe baixa): recusado **antes** do handler.
    let stream = unique("stream-pressao");
    common::no_local_pronto(&pool, TENANT_EVENTOS, &stream).await;
    let evento = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 1))).await;
    assert_eq!(evento.status, 503, "{:?}", evento.body);
    assert_eq!(event_rows(&pool, TENANT_EVENTOS, &stream).await, 0);

    // Incidente sem header: classe da rota (`incident`) não é recusada.
    let id_plain = unique("inc-plain");
    let incidente = send_async(
        addr,
        "POST",
        "/v1/incidents",
        Some(&tokens.a),
        None,
        Some(json!({"incident_id": id_plain}).to_string()),
    )
    .await;
    assert_eq!(incidente.status, 201, "{:?}", incidente.body);

    // SOS sob a MESMA pressão: 201. É o invariante §4-2 provado no fio.
    let id_sos = unique("inc-sos");
    let sos = send_async(
        addr,
        "POST",
        "/v1/incidents",
        Some(&tokens.a),
        Some(RequestClass::Sos.as_str()),
        Some(json!({"incident_id": id_sos}).to_string()),
    )
    .await;
    assert_eq!(sos.status, 201, "SOS não pode ser recusado por pressão: {:?}", sos.body);
    assert_eq!(incident_rows(&pool, &id_sos).await, 1);
}

/// Flood de `event` não consome a cota de SOS: buckets são por
/// *(tenant, token, classe)*. Com burst 1 no escopo `event:write` (que governa
/// `event`), o segundo evento cai; um SOS na mesma situação não toca nesse
/// bucket e passa.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flood_de_evento_nao_consome_a_cota_de_sos() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(
        pool.clone(),
        grants,
        config_with(
            Scope::EventWrite,
            ScopeLimits {
                requests_per_sec: 1.0,
                request_burst: 1.0,
                events_per_sec: 1.0,
                event_burst: 1.0,
            },
        ),
    )
    .await;
    let stream = unique("stream-sos-cota");
    common::no_local_pronto(&pool, TENANT_EVENTOS, &stream).await;

    let primeiro = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 1))).await;
    assert_eq!(primeiro.status, 201, "{:?}", primeiro.body);
    let enchente = send_async(addr, "POST", "/v1/events", Some(&tokens.eventos), None, Some(event_body(&stream, 2))).await;
    assert_eq!(enchente.status, 429, "segunda rajada do mesmo bucket: {:?}", enchente.body);

    // SOS usa bucket próprio (governado por `incident:write`, default alto):
    // a enchente de `event` não o alcança.
    let id_sos = unique("inc-sos-independente");
    let sos = send_async(
        addr,
        "POST",
        "/v1/incidents",
        Some(&tokens.a),
        Some("sos"),
        Some(json!({"incident_id": id_sos}).to_string()),
    )
    .await;
    assert_eq!(sos.status, 201, "{:?}", sos.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classe_declarada_invalida_e_recusada_422_sem_efecto() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let addr = serve(pool.clone(), grants, AdmissionConfig::default()).await;

    let id = unique("inc-classe-invalida");
    let reply = send_async(
        addr,
        "POST",
        "/v1/incidents",
        Some(&tokens.a),
        Some("banana"),
        Some(json!({"incident_id": id}).to_string()),
    )
    .await;
    assert_eq!(reply.status, 422, "{:?}", reply.body);
    assert_eq!(reply.error(), "invalid_request");
    assert_eq!(incident_rows(&pool, &id).await, 0);
}

// ---------------------------------------------------------------------------
// 4. Cota por IP (pré-autenticação)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_credencial_a_cota_e_por_ip_com_connectinfo() {
    let Some(pool) = common::pool_com_conexoes(4).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, _tokens) = registry();
    let config = AdmissionConfig {
        ip_limits: ScopeLimits {
            requests_per_sec: 1.0,
            request_burst: 2.0,
            events_per_sec: 1.0,
            event_burst: 2.0,
        },
        ..AdmissionConfig::default()
    };
    let addr = serve_with_connect_info(pool.clone(), grants, config).await;

    // Três pedidos **sem** Authorization: 401, 401 (o portão de auth responde
    // com 401 independentemente da cota), e o terceiro cai na cota por IP.
    let id1 = unique("inc-ip");
    let r1 = send_async(addr, "POST", "/v1/incidents", None, None, Some(json!({"incident_id": id1}).to_string())).await;
    let r2 = send_async(addr, "POST", "/v1/incidents", None, None, Some(json!({"incident_id": id1}).to_string())).await;
    assert_eq!((r1.status, r2.status), (401, 401), "{:?}", (r1.body, r2.body));

    let r3 = send_async(addr, "POST", "/v1/incidents", None, None, Some(json!({"incident_id": unique("inc-ip-3")}).to_string())).await;
    assert_eq!(r3.status, 429, "cota por IP estourada: {:?}", r3.body);
    assert_eq!(r3.error(), "rate_limited");
    assert!(r3.retry_after().is_some_and(|v| v >= 1));
}

// ---------------------------------------------------------------------------
// 5. Timeout por rota
// ---------------------------------------------------------------------------

/// Com o banco ocupado (pool de 1 conexão retida pelo teste), o handler não
/// termina dentro do teto declarado: **503** `request_timeout` com corpo
/// estável — não 504 (RFC 9110 §15.6.5 reserva 504 para gateway/proxy sem
/// resposta de upstream; aqui o atraso é do próprio servidor) nem 408 (§15.5.9
/// pressupõe requisição incompleta, o que não é o caso). A prova é a resposta
/// do portão, não a ausência do efeito: o corpo declara `effect: unknown`,
/// manda reenviar com a mesma idempotência e traz `Retry-After` (autorizado
/// pela §15.6.4 para o 503).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handler_lento_alcanca_o_teto_e_recebe_503_request_timeout_com_effect_unknown() {
    let Some(pool) = common::pool_com_conexoes(1).await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let (grants, tokens) = registry();
    let config = AdmissionConfig {
        request_timeout: Duration::from_millis(300),
        ..AdmissionConfig::default()
    };
    let addr = serve(pool.clone(), grants, config).await;

    // Retém a única conexão do pool: qualquer query do servidor espera.
    let amarra = pool.acquire().await.expect("retém a conexão única do pool");

    let reply = send_async(
        addr,
        "POST",
        "/v1/incidents",
        Some(&tokens.a),
        None,
        Some(json!({"incident_id": unique("inc-timeout")}).to_string()),
    )
    .await;
    assert_eq!(reply.status, 503, "{:?}", reply.body);
    assert_eq!(reply.error(), "request_timeout");
    assert_eq!(reply.body["ack_class"], "transport_accepted");
    assert_eq!(
        reply.body["effect"], "unknown",
        "timeout deve declarar efeito desconhecido: {:?}",
        reply.body
    );
    assert!(
        reply.body["hint"].as_str().is_some_and(|h| h.contains("idempot")),
        "hint deve mandar reenviar com a mesma idempotência: {:?}",
        reply.body
    );
    assert_eq!(reply.retry_after(), Some(1), "503 com Retry-After (§15.6.4)");

    drop(amarra);
    // O timeout em si não é o que se prova; provamos o corpo estável, o status
    // e a declaração de efeito desconhecido. O reenvio com a mesma
    // idempotência é a estratégia declarada ao cliente.
}
