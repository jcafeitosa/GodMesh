//! Integração local do Inbox de sync (`POST /v1/sync/inbox`, `GET /v1/sync/state`).
//!
//! Os testes falam HTTP/1.1 cru de propósito: status de transporte, código de
//! erro nomeado e corpo são o contrato observável, e precisam ser provados no fio.
//!
//! Banco próprio desta suíte, nunca o de desenvolvimento compartilhado. O
//! esquema é aplicado pelo runner oficial (`tests/common/mod.rs`), que aplica
//! 0001..0005 na ordem:
//!   psql postgres://godmesh:godmesh_dev_only@localhost:55432/postgres \
//!        -c 'CREATE DATABASE godmesh_se'
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_se \
//!     cargo test -p godmesh-server --test inbox -- --nocapture
//!
//! Sem `DATABASE_URL` os testes são ignorados (não fingem sucesso).

mod common;

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{GrantRegistry, Scope, Subject, VerifiedGrant};
use serde_json::Value;
use sqlx::PgPool;

/// Tenant de teste. A jurisdição é dado do servidor (ADR-066); o teste a declara
/// no banco antes do caminho feliz.
const TENANT: &str = "tenant-inbox-br";
/// Token do principal que envia e consulta. Não é segredo real: vive no processo.
const TOKEN: &str = "grant-inbox-principal-7";
/// Classe semeada pela migração 0001 para BR, com cópia primária.
const CLASS: &str = "mensagem";

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

fn grant() -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: TENANT.to_owned(),
            principal: "principal-7".to_owned(),
        },
        predicate: allow_predicate(),
        token_id: "token-inbox-principal-7".to_owned(),
        // Lote é escrita do Inbox; a leitura do estado fica no escopo `sync:read`.
        scopes: vec![Scope::SyncWrite, Scope::SyncRead],
        authority_epoch: 3,
    }
}

/// Banco de teste migrado pelo runner oficial (`tests/common/mod.rs`).
async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(4).await
}

/// Pool novo para o mesmo banco: conexões novas, nenhum estado em memória do
/// servidor anterior. É a forma usada aqui de provar que o prefixo vem do banco.
/// Passa pelo mesmo runner de migrações (idempotente: esquema já aplicado).
async fn fresh_pool(url: &str) -> PgPool {
    common::pool_para(url, 2).await
}

async fn declare_tenant(pool: &PgPool, tenant: &str, jurisdiction: &str) {
    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, $2) ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction")
        .bind(tenant)
        .bind(jurisdiction)
        .execute(pool)
        .await
        .expect("jurisdição de teste declarada");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ($1, $2, 'historico', 24, 'expiry') ON CONFLICT DO NOTHING",
    )
    .bind(CLASS)
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("classe mensagem presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ($1, $2, 'primary') ON CONFLICT DO NOTHING",
    )
    .bind(CLASS)
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("cópia primária presente");
}

async fn serve(pool: PgPool) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool, registry());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_entries(vec![(TOKEN.to_owned(), grant())])
}

fn roundtrip(addr: SocketAddr, request: String) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de escrita");
    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");
    let status: u16 = raw
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    (status, raw)
}

fn http_post(addr: SocketAddr, authorization: Option<&str>, body: &str) -> (u16, String) {
    let mut request = format!(
        "POST /v1/sync/inbox HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    roundtrip(addr, request)
}

fn http_get(addr: SocketAddr, authorization: Option<&str>, path: &str) -> (u16, String) {
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    roundtrip(addr, request)
}

fn json_body(raw: &str) -> Value {
    let start = raw.find("\r\n\r\n").map_or(0, |i| i + 4);
    let body = &raw[start..];
    serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("corpo não é JSON ({e}): {body:?} no bruto {raw:?}"))
}

/// Namespace de teste: uma origem/stream/partição/epoch por chamada.
struct Ns {
    origin: String,
    stream: String,
    partition: u32,
    epoch: u64,
}

impl Ns {
    fn new() -> Self {
        Self {
            origin: unique("node"),
            stream: unique("stream"),
            partition: 1,
            epoch: 7,
        }
    }

    fn position(&self, seq: u64) -> Value {
        serde_json::json!({
            "origin_node": self.origin,
            "stream_id": self.stream,
            "partition": self.partition,
            "authority_epoch": self.epoch,
            "seq": seq,
        })
    }

    fn batch(&self, seqs: &[u64]) -> String {
        let positions: Vec<Value> = seqs.iter().map(|s| self.position(*s)).collect();
        serde_json::json!({"retention_class": CLASS, "positions": positions}).to_string()
    }

    fn state_path(&self) -> String {
        format!(
            "/v1/sync/state?stream_id={}&partition={}&authority_epoch={}",
            self.stream, self.partition, self.epoch
        )
    }
}

fn auth() -> String {
    format!("Bearer {TOKEN}")
}

async fn post(addr: SocketAddr, body: String) -> (u16, String) {
    tokio::task::spawn_blocking(move || {
        let auth = auth();
        http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP")
}

async fn get(addr: SocketAddr, path: String) -> (u16, String) {
    tokio::task::spawn_blocking(move || {
        let auth = auth();
        http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP")
}

async fn seqs_in_db(pool: &PgPool, ns: &Ns) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT seq FROM event_position \
          WHERE tenant_id = $1 AND origin_node = $2 AND stream_id = $3 AND partition = $4 \
            AND authority_epoch = $5 ORDER BY seq",
    )
    .bind(TENANT)
    .bind(&ns.origin)
    .bind(&ns.stream)
    .bind(i32::try_from(ns.partition).unwrap())
    .bind(i64::try_from(ns.epoch).unwrap())
    .fetch_all(pool)
    .await
    .expect("posições persistidas")
}

async fn prefix_in_db(pool: &PgPool, ns: &Ns) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT contiguous_prefix FROM sync_inbox_state \
          WHERE tenant_id = $1 AND origin_node = $2 AND stream_id = $3 AND partition = $4 \
            AND authority_epoch = $5",
    )
    .bind(TENANT)
    .bind(&ns.origin)
    .bind(&ns.stream)
    .bind(i32::try_from(ns.partition).unwrap())
    .bind(i64::try_from(ns.epoch).unwrap())
    .fetch_optional(pool)
    .await
    .expect("checkpoint do Inbox")
}

/// Prepara banco + servidor e devolve o endereço e o pool.
async fn setup() -> Option<(PgPool, SocketAddr)> {
    let pool = pool().await?;
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone()).await;
    Some((pool, addr))
}

// --- caminho feliz -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contiguous_batch_is_accepted_and_advances_the_prefix() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = post(addr, ns.batch(&[1, 2, 3])).await;
    assert_eq!(status, 200, "lote contíguo precisa ser 200: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["accepted"], 3);
    assert_eq!(body["duplicates"], 0);
    assert_eq!(body["contiguous_prefix"], 3);
    assert_eq!(body["ack_class"], "persisted_local", "ACK máximo desta fatia");

    assert_eq!(seqs_in_db(&pool, &ns).await, vec![1, 2, 3]);
    assert_eq!(prefix_in_db(&pool, &ns).await, Some(3));

    // tenant vem do grant; jurisdição da região do tenant; classe do envelope.
    let row: (String, String, String) = sqlx::query_as(
        "SELECT tenant_id, jurisdiction, retention_class FROM event_position \
          WHERE stream_id = $1 AND seq = 2",
    )
    .bind(&ns.stream)
    .fetch_one(&pool)
    .await
    .expect("linha gravada");
    assert_eq!(row.0, TENANT);
    assert_eq!(row.1, "BR");
    assert_eq!(row.2, CLASS);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_batch_is_all_duplicates_and_writes_nothing_new() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = post(addr, ns.batch(&[1, 2])).await;
    assert_eq!(status, 200, "primeiro lote: {raw}");

    let (status, raw) = post(addr, ns.batch(&[1, 2])).await;
    assert_eq!(status, 200, "reenvio do lote precisa ser 200: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["accepted"], 0, "evento já visto não é reprocessado");
    assert_eq!(body["duplicates"], 2);
    assert_eq!(body["contiguous_prefix"], 2);
    assert_eq!(body["ack_class"], "persisted_local");

    assert_eq!(seqs_in_db(&pool, &ns).await, vec![1, 2], "sem linha nova");
}

// --- lacuna ------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gap_is_409_sequence_gap_and_does_not_advance_past_it() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = post(addr, ns.batch(&[1, 3])).await;
    assert_eq!(status, 409, "lacuna precisa ser 409: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["error"], "sequence_gap");
    assert_eq!(body["expected"], 2, "posição que a stream espera");
    assert_eq!(body["received"], 3, "posição que chegou");
    assert_eq!(body["contiguous_prefix"], 1, "prefixo não passa da lacuna");
    assert_eq!(body["ack_class"], "transport_accepted", "erro não confirma nada");

    assert_eq!(seqs_in_db(&pool, &ns).await, vec![1], "seq 3 não é aceita");
    assert_eq!(prefix_in_db(&pool, &ns).await, Some(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_position_of_an_empty_stream_must_be_one() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = post(addr, ns.batch(&[5])).await;
    assert_eq!(status, 409, "stream vazia só aceita a posição 1: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["expected"], 1);
    assert_eq!(body["received"], 5);
    assert_eq!(body["contiguous_prefix"], 0);
    assert!(seqs_in_db(&pool, &ns).await.is_empty());
    assert_eq!(
        prefix_in_db(&pool, &ns).await,
        None,
        "nada aceito, nenhum checkpoint gravado"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_batch_accepts_the_contiguous_prefix_and_reports_the_gap() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = post(addr, ns.batch(&[1, 2])).await;
    assert_eq!(status, 200, "prefixo inicial: {raw}");

    let (status, raw) = post(addr, ns.batch(&[3, 4, 6, 7])).await;
    assert_eq!(status, 409, "lote misto termina em lacuna: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["error"], "sequence_gap");
    assert_eq!(body["accepted"], 2, "aceita até a lacuna");
    assert_eq!(body["duplicates"], 0);
    assert_eq!(body["expected"], 5);
    assert_eq!(body["received"], 6);
    assert_eq!(body["contiguous_prefix"], 4);

    assert_eq!(
        seqs_in_db(&pool, &ns).await,
        vec![1, 2, 3, 4],
        "6 e 7 não são aceitas"
    );
    assert_eq!(prefix_in_db(&pool, &ns).await, Some(4));

    // A posição que faltava destrava o resto: reenvio só a partir dela.
    let (status, raw) = post(addr, ns.batch(&[5, 6, 7])).await;
    assert_eq!(status, 200, "sem lacuna o lote passa: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["accepted"], 3);
    assert_eq!(body["contiguous_prefix"], 7);
    assert_eq!(seqs_in_db(&pool, &ns).await, vec![1, 2, 3, 4, 5, 6, 7]);
}

// --- escopo ------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_mixing_partitions_is_400_cursor_incomparable_and_writes_nothing() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let outro = Value::Object(
        serde_json::json!({
            "origin_node": ns.origin,
            "stream_id": ns.stream,
            "partition": ns.partition + 1,
            "authority_epoch": ns.epoch,
            "seq": 2,
        })
        .as_object()
        .expect("objeto")
        .clone(),
    );
    let body = serde_json::json!({
        "retention_class": CLASS,
        "positions": [ns.position(1), outro],
    })
    .to_string();

    let (status, raw) = post(addr, body).await;
    assert_eq!(status, 400, "outra partição é incomparável: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["error"], "cursor_incomparable");
    assert!(
        body["detail"]
            .as_str()
            .is_some_and(|d| d.contains("partition")),
        "o detalhe precisa nomear o campo divergente: {body}"
    );
    assert!(seqs_in_db(&pool, &ns).await.is_empty(), "nada é aceito");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_mixing_epochs_is_400_cursor_incomparable() {
    let Some((_pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let antigo = serde_json::json!({
        "origin_node": ns.origin,
        "stream_id": ns.stream,
        "partition": ns.partition,
        "authority_epoch": ns.epoch - 1,
        "seq": 2,
    });
    let body = serde_json::json!({
        "retention_class": CLASS,
        "positions": [ns.position(1), antigo],
    })
    .to_string();

    let (status, raw) = post(addr, body).await;
    assert_eq!(status, 400, "epoch antigo é incomparável: {raw}");
    assert_eq!(json_body(&raw)["error"], "cursor_incomparable");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_above_the_documented_ceiling_is_422() {
    let Some((_pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let seqs: Vec<u64> = (1..=101).collect();

    let (status, raw) = post(addr, ns.batch(&seqs)).await;
    assert_eq!(status, 422, "101 posições excedem o teto de 100: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["error"], "invalid_request");
    assert!(
        body["detail"].as_str().is_some_and(|d| d.contains("100")),
        "o detalhe precisa citar o teto: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unauthenticated_batch_is_401_and_writes_nothing() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let body = ns.batch(&[1]);
    let (status, raw) = tokio::task::spawn_blocking(move || http_post(addr, None, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 401, "sem grant nada é aceito: {raw}");
    assert!(seqs_in_db(&pool, &ns).await.is_empty());
}

// --- estado e persistência ---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_stream_is_zero_prefix_not_an_error() {
    let Some((_pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();

    let (status, raw) = get(addr, ns.state_path()).await;
    assert_eq!(status, 200, "stream desconhecida é estado vazio: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["contiguous_prefix"], 0);
    assert!(body["last_recorded_at"].is_null());
    assert_eq!(body["gaps"].as_array().map(Vec::len), Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_reports_prefix_last_recorded_at_and_gaps() {
    let Some((pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let (status, raw) = post(addr, ns.batch(&[1, 2])).await;
    assert_eq!(status, 200, "prefixo inicial: {raw}");

    let (status, raw) = get(addr, ns.state_path()).await;
    assert_eq!(status, 200, "estado após lote: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["contiguous_prefix"], 2);
    assert!(
        body["last_recorded_at"]
            .as_str()
            .is_some_and(|t| t.ends_with('Z')),
        "instante ISO-8601 UTC do avanço do prefixo: {body}"
    );
    assert_eq!(body["gaps"].as_array().map(Vec::len), Some(0));

    // Buraco gravado fora da API (não há caminho HTTP que o produza): o estado
    // deriva as lacunas das posições duráveis, não da memória do processo.
    sqlx::query(
        "INSERT INTO event_position \
             (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, \
              retention_class, jurisdiction) \
         VALUES ($1, $2, $3, $4, $5, 4, $6, 'BR')",
    )
    .bind(&ns.origin)
    .bind(TENANT)
    .bind(&ns.stream)
    .bind(i32::try_from(ns.partition).unwrap())
    .bind(i64::try_from(ns.epoch).unwrap())
    .bind(CLASS)
    .execute(&pool)
    .await
    .expect("buraco inserido direto");

    let (status, raw) = get(addr, ns.state_path()).await;
    assert_eq!(status, 200, "estado com buraco: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["gaps"][0]["from"], 3, "intervalo ausente: {body}");
    assert_eq!(body["gaps"][0]["to"], 3);
    assert_eq!(body["gaps"][0]["origin_node"], ns.origin.as_str());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefix_survives_a_new_process_state_by_being_read_from_the_database() {
    let Some((_pool, addr)) = setup().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let ns = Ns::new();
    let (status, raw) = post(addr, ns.batch(&[1, 2, 3])).await;
    assert_eq!(status, 200, "prefixo inicial: {raw}");

    // "Reinício": outro pool (conexões novas) e outro roteador, sem nenhum estado
    // em memória do primeiro. O prefixo só pode vir do banco.
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL presente");
    let segundo_pool = fresh_pool(&url).await;
    let segundo_addr = serve(segundo_pool.clone()).await;

    let (status, raw) = get(segundo_addr, ns.state_path()).await;
    assert_eq!(status, 200, "estado no processo novo: {raw}");
    assert_eq!(
        json_body(&raw)["contiguous_prefix"], 3,
        "prefixo lido do banco, não da memória"
    );

    // E a retomada é coerente: o lote repetido é duplicata, o próximo lote continua.
    let (status, raw) = post(segundo_addr, ns.batch(&[2, 3])).await;
    assert_eq!(status, 200, "reenvio após reinício: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["accepted"], 0);
    assert_eq!(body["duplicates"], 2);
    assert_eq!(body["contiguous_prefix"], 3);

    let (status, raw) = post(segundo_addr, ns.batch(&[4])).await;
    assert_eq!(status, 200, "continuação após reinício: {raw}");
    assert_eq!(json_body(&raw)["contiguous_prefix"], 4);
    assert_eq!(prefix_in_db(&segundo_pool, &ns).await, Some(4));
}