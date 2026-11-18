//! Integração local dos endpoints de evento (`POST/GET /v1/events`).
//!
//! Os testes falam HTTP/1.1 cru de propósito: status de transporte e código de
//! erro nomeado são o contrato observável, e precisam ser provados no fio.
//!
//! Banco próprio desta suíte, nunca o de desenvolvimento compartilhado. O
//! esquema é aplicado pelo runner oficial (`tests/common/mod.rs`):
//!   psql postgres://godmesh:godmesh_dev_only@localhost:55432/postgres \
//!        -c 'CREATE DATABASE godmesh_sb'
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sb \
//!     cargo test -p godmesh-server --test events -- --nocapture
//!
//! Sem `DATABASE_URL` os testes são ignorados (não fingem sucesso).

mod common;

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use godmesh_server::auth::{GrantRegistry, Scope, Subject, VerifiedGrant};
use serde_json::Value;
use sqlx::PgPool;

/// Tenant BR de teste. A jurisdição é dado do servidor (ADR-066); os testes a
/// declaram no banco antes de esperar caminho feliz.
const TENANT: &str = "tenant-eventos-br";
/// Token do principal que grava e lê. Não é segredo real: vive no processo de teste.
const TOKEN_MAIN: &str = "grant-eventos-principal-42";
/// Token de outro principal do **mesmo** tenant: prova que o cursor é vinculado
/// ao principal, não só ao tenant.
const TOKEN_OTHER: &str = "grant-eventos-principal-99";
/// Classe semeada pela migração 0001 para BR, com cópia primária.
const CLASS: &str = "mensagem";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn unique_stream() -> String {
    unique("stream")
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

fn grant(principal: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: TENANT.to_owned(),
            principal: principal.to_owned(),
        },
        predicate: allow_predicate(),
        token_id: "test-grant".to_owned(),
        scopes: Scope::ALL.to_vec(),
        authority_epoch: 1,
    }
}

/// Banco de teste migrado pelo runner oficial (`tests/common/mod.rs`).
async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(4).await
}

/// Declara a jurisdição do tenant de teste (ADR-066) e garante a semente de
/// retenção que a API resolve. Idempotente com as migrações.
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

async fn serve(pool: PgPool, grants: GrantRegistry) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool, grants);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_entries(vec![
        (TOKEN_MAIN.to_owned(), grant("principal-42")),
        (TOKEN_OTHER.to_owned(), grant("principal-99")),
    ])
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
        "POST /v1/events HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
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
    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n"
    );
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    roundtrip(addr, request)
}

/// Corpo HTTP separado do JSON: comparar o JSON inteiro evita depender de
/// formatação do servidor.
fn json_body(raw: &str) -> Value {
    let start = raw.find("\r\n\r\n").map_or(0, |i| i + 4);
    let body = &raw[start..];
    // Respostas usam Content-Length; o corpo é o resto após os headers.
    serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("corpo não é JSON ({e}): {body:?} no bruto {raw:?}"))
}

fn event_body(stream: &str, partition: u32, epoch: u64, seq: u64, class: &str) -> String {
    // S-sec-2: o corpo aceito pela rota é o envelope assinado pelo nó local de
    // teste (`common::LOCAL_NODE`). A origem da posição **é** o assinante — o
    // `"node-sb"` literal desta suíte morreu com a fatia.
    common::envelope_assinado(
        &common::LOCAL_NODE,
        TENANT,
        stream,
        partition,
        epoch,
        seq,
        class,
    )
}

async fn rows(pool: &PgPool, tenant: &str, stream: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM event_position WHERE tenant_id = $1 AND stream_id = $2")
        .bind(tenant)
        .bind(stream)
        .fetch_one(pool)
        .await
        .expect("contagem de event_position")
}

fn auth() -> String {
    format!("Bearer {TOKEN_MAIN}")
}

// --- caminho feliz ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persists_position_with_tenant_from_grant_and_jurisdiction_from_tenant_region() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let (status, raw) = tokio::task::spawn_blocking({
        let body = event_body(&stream, 1, 7, 1, CLASS);
        let auth = auth();
        move || http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 201, "posição nova precisa ser 201: {raw}");
    let body = json_body(&raw);
    assert_eq!(body["ack_class"], "persisted_local", "ACK máximo desta fatia");
    assert_eq!(body["seq"], 1);
    assert_eq!(body["retention_class"], CLASS);
    assert_eq!(body["partition"], 1);
    assert_eq!(body["authority_epoch"], 7);

    let row: (String, String, String) = sqlx::query_as(
        "SELECT tenant_id, retention_class, jurisdiction FROM event_position \
          WHERE stream_id = $1 AND seq = 1",
    )
    .bind(&stream)
    .fetch_one(&pool)
    .await
    .expect("linha gravada");
    assert_eq!(row.0, TENANT, "tenant vem do grant verificado");
    assert_eq!(row.1, CLASS);
    assert_eq!(row.2, "BR", "jurisdição vem de tenant_region");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reads_pages_with_next_cursor_and_ends_without_one() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    for seq in 1..=3 {
        let (status, raw) = tokio::task::spawn_blocking({
            let body = event_body(&stream, 1, 7, seq, CLASS);
            let auth = auth();
            move || http_post(addr, Some(&auth), &body)
        })
        .await
        .expect("cliente HTTP");
        assert_eq!(status, 201, "seq {seq} precisa ser aceita: {raw}");
    }

    let path = format!("/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=2");
    let (status, raw) = tokio::task::spawn_blocking({
        let path = path.clone();
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 200, "leitura autorizada: {raw}");
    let page = json_body(&raw);
    assert_eq!(page["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(page["events"][0]["seq"], 1);
    assert_eq!(page["events"][1]["seq"], 2);
    let next = page["next_cursor"]
        .as_str()
        .expect("há próxima página: next_cursor presente")
        .to_owned();

    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=2&cursor={next}"
    );
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 200, "cursor próprio precisa ser aceito: {raw}");
    let page = json_body(&raw);
    assert_eq!(page["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["events"][0]["seq"], 3);
    assert!(
        page["next_cursor"].is_null(),
        "fim do intervalo não emite cursor vazio: {page:?}"
    );
}

// --- classe de retenção inexistente ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_retention_class_is_409_and_writes_nothing() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let (status, raw) = tokio::task::spawn_blocking({
        let body = event_body(&stream, 1, 7, 1, "classe-que-nao-existe");
        let auth = auth();
        move || http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 409, "classe inexistente precisa ser 409: {raw}");
    assert!(
        raw.contains("retention_policy_missing"),
        "código precisa ser nomeado: {raw}"
    );
    assert_eq!(rows(&pool, TENANT, &stream).await, 0, "nada é gravado");
}

// --- repetição idempotente ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_position_is_200_with_the_original_ack_and_no_duplicate() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let body = event_body(&stream, 1, 7, 1, CLASS);
    let auth = auth();
    let (first_status, first_raw) = tokio::task::spawn_blocking({
        let body = body.clone();
        let auth = auth.clone();
        move || http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(first_status, 201, "primeira gravação: {first_raw}");

    let (status, raw) = tokio::task::spawn_blocking(move || http_post(addr, Some(&auth), &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 200, "repetição da mesma posição precisa ser 200: {raw}");
    assert_eq!(
        json_body(&raw)["ack_class"],
        json_body(&first_raw)["ack_class"],
        "replay repete o ACK original"
    );
    assert_eq!(rows(&pool, TENANT, &stream).await, 1, "replay não duplica");
}

// --- salto de sequência -------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sequence_jump_is_409_and_the_expected_position_is_named() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let (status, raw) = tokio::task::spawn_blocking({
        let body = event_body(&stream, 1, 7, 2, CLASS);
        let auth = auth();
        move || http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 409, "salto de sequência precisa ser 409: {raw}");
    assert!(raw.contains("sequence_gap"), "código precisa ser nomeado: {raw}");
    assert!(raw.contains("espera 1"), "lacuna precisa nomear a posição esperada: {raw}");
    assert_eq!(rows(&pool, TENANT, &stream).await, 0, "lacuna não é saltada");

    // Controle positivo: a posição esperada é aceita e destrava a sequência.
    let accepted = tokio::task::spawn_blocking({
        let body = event_body(&stream, 1, 7, 1, CLASS);
        let auth = auth();
        move || http_post(addr, Some(&auth), &body)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(accepted.0, 201, "a posição esperada precisa ser aceita: {}", accepted.1);
    assert_eq!(rows(&pool, TENANT, &stream).await, 1);
}

// --- cursor -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cursor_from_another_partition_is_400_cursor_incomparable() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    for seq in 1..=2 {
        let (status, raw) = tokio::task::spawn_blocking({
            let body = event_body(&stream, 1, 7, seq, CLASS);
            let auth = auth();
            move || http_post(addr, Some(&auth), &body)
        })
        .await
        .expect("cliente HTTP");
        assert_eq!(status, 201, "preparação da partição 1: {raw}");
    }

    let path = format!("/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1");
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 200, "primeira página: {raw}");
    let next = json_body(&raw)["next_cursor"]
        .as_str()
        .expect("cursor emitido")
        .to_owned();

    // Cursor emitido para a partição 1, reapresentado na partição 2: recusa, não
    // tradução. Nenhum evento da partição 2 pode aparecer.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=2&authority_epoch=7&limit=1&cursor={next}"
    );
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 400, "cursor de outra partição precisa ser 400: {raw}");
    assert!(
        raw.contains("cursor_incomparable"),
        "código precisa ser incomparável, não inválido: {raw}"
    );
    assert!(!raw.contains("\"events\""), "nada é lido com cursor incomparável: {raw}");

    // Cursor de outro epoch: mesma recusa, causa diferente.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=8&limit=1&cursor={next}"
    );
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 400, "cursor de epoch incomparável precisa ser 400: {raw}");
    assert!(raw.contains("cursor_incomparable"), "{raw}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_cursor_is_400_and_absent_cursor_is_a_fresh_read() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    // Cursor ausente: leitura do início, 200 mesmo sem nenhum evento.
    let path = format!("/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=10");
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 200, "sem cursor a leitura começa do início: {raw}");
    let page = json_body(&raw);
    assert_eq!(page["events"].as_array().map(Vec::len), Some(0));
    assert!(page["next_cursor"].is_null());

    for token in ["lixo", "v1.nao-e-base64!", "v9.QUJD"] {
        let path = format!(
            "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=10&cursor={token}"
        );
        let (status, raw) = tokio::task::spawn_blocking({
            let auth = auth();
            move || http_get(addr, Some(&auth), &path)
        })
        .await
        .expect("cliente HTTP");
        assert_eq!(status, 400, "cursor {token:?} precisa ser 400: {raw}");
        assert!(
            raw.contains("cursor_invalid"),
            "cursor {token:?} precisa ser inválido, não incomparável: {raw}"
        );
    }

    // Cursor vazio é "cursor apresentado", não ausência: 400, não 200.
    let path = format!("/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=10&cursor=");
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 400, "cursor vazio é inválido: {raw}");
    assert!(raw.contains("cursor_invalid"), "{raw}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cursor_of_another_principal_is_400_cursor_invalid() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT, "BR").await;
    let addr = serve(pool.clone(), registry()).await;
    let stream = unique_stream();
    common::no_local_pronto(&pool, TENANT, &stream).await;

    for seq in 1..=2 {
        let (status, raw) = tokio::task::spawn_blocking({
            let body = event_body(&stream, 1, 7, seq, CLASS);
            let auth = auth();
            move || http_post(addr, Some(&auth), &body)
        })
        .await
        .expect("cliente HTTP");
        assert_eq!(status, 201, "preparação: {raw}");
    }

    let path = format!("/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1");
    let (status, raw) = tokio::task::spawn_blocking({
        let auth = auth();
        move || http_get(addr, Some(&auth), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 200, "primeira página: {raw}");
    let next = json_body(&raw)["next_cursor"]
        .as_str()
        .expect("cursor emitido")
        .to_owned();

    // Mesmo tenant, outro principal: o token não é transferível.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1&cursor={next}"
    );
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_get(addr, Some(&format!("Bearer {TOKEN_OTHER}")), &path)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 400, "cursor de outro principal precisa ser 400: {raw}");
    assert!(raw.contains("cursor_invalid"), "vínculo é do principal: {raw}");
}