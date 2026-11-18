//! `GET /v1/incidents` — listagem paginada por cursor, no fio.
//!
//! Os testes falam HTTP/1.1 cru de propósito: status de transporte e código de
//! erro nomeado são o contrato observável.
//!
//! Cada teste usa **tenant próprio** (sufixo único), então nenhuma asserção de
//! conjunto depende de o banco estar vazio nem de outro teste rodando em
//! paralelo.
//!
//! Banco próprio desta suíte, migrado pelo runner oficial (`tests/common/mod.rs`):
//!   createdb godmesh_t2
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_t2 \
//!     cargo test -p godmesh-server --test incidents_list -- --nocapture
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

/// Sufixo único por execução: tenant novo por teste, sem limpeza destrutiva.
static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn allow_grant(tenant_id: &str, token_id: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-list".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::OpenSosIncident,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
        token_id: token_id.to_owned(),
        scopes: Scope::ALL.to_vec(),
        authority_epoch: 1,
    }
}

async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(8).await
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
         VALUES ('incidente_sos', $1, 'dever_de_cuidado', 12, 'incident_closed + 12m') \
         ON CONFLICT DO NOTHING",
    )
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("classe incidente_sos presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ('incidente_sos', $1, 'primary') ON CONFLICT DO NOTHING",
    )
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("cópia primária presente");
}

/// Um servidor por teste, com os grants montados na hora (token → tenant).
async fn serve(pool: PgPool, entries: Vec<(String, VerifiedGrant)>) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool.clone(), GrantRegistry::from_entries(entries));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de escrita");
    let body = body.unwrap_or("");
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
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

fn json_body(raw: &str) -> Value {
    let start = raw.find("\r\n\r\n").map_or(0, |i| i + 4);
    let body = &raw[start..];
    serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("corpo não é JSON ({e}): {body:?} no bruto {raw:?}"))
}

fn create(addr: SocketAddr, token: &str, incident_id: &str) -> (u16, String) {
    let body = serde_json::json!({ "incident_id": incident_id }).to_string();
    let authorization = format!("Bearer {token}");
    http(
        addr,
        "POST",
        "/v1/incidents",
        Some(&authorization),
        Some(&body),
    )
}

fn list(addr: SocketAddr, token: &str, query: &str) -> (u16, String) {
    let authorization = format!("Bearer {token}");
    let path = if query.is_empty() {
        "/v1/incidents".to_owned()
    } else {
        format!("/v1/incidents?{query}")
    };
    http(addr, "GET", &path, Some(&authorization), None)
}

/// ids da página, na ordem devolvida.
fn ids(page: &Value) -> Vec<String> {
    page["incidents"]
        .as_array()
        .unwrap_or_else(|| panic!("página sem incidents: {page:?}"))
        .iter()
        .map(|item| {
            item["incident_id"]
                .as_str()
                .unwrap_or_else(|| panic!("item sem incident_id: {item:?}"))
                .to_owned()
        })
        .collect()
}

// --- isolamento entre tenants ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lista_somente_incidentes_do_tenant_do_grant() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant_a = unique("tenant-list-a");
    let tenant_b = unique("tenant-list-b");
    declare_tenant(&pool, &tenant_a, "BR").await;
    declare_tenant(&pool, &tenant_b, "BR").await;

    let token_a = unique("grant-a");
    let token_b = unique("grant-b");
    let addr = serve(
        pool.clone(),
        vec![
            (token_a.clone(), allow_grant(&tenant_a, "grant-list-a")),
            (token_b.clone(), allow_grant(&tenant_b, "grant-list-b")),
        ],
    )
    .await;

    // Mais recente por último: a ordem publicada é a mais nova primeiro.
    let a1 = unique("inc-a1");
    let a2 = unique("inc-a2");
    let b1 = unique("inc-b1");
    assert_eq!(create(addr, &token_a, &a1).0, 201);
    assert_eq!(create(addr, &token_a, &a2).0, 201);
    assert_eq!(create(addr, &token_b, &b1).0, 201);

    let (status, raw) = list(addr, &token_a, "limit=100");
    assert_eq!(status, 200, "listagem autorizada: {raw}");
    let page = json_body(&raw);
    assert_eq!(
        ids(&page),
        vec![a2.clone(), a1.clone()],
        "só o tenant do grant, mais novo primeiro"
    );
    assert!(
        !page.to_string().contains(&b1),
        "incidente de outro tenant não pode aparecer: {page:?}"
    );
    assert_eq!(page["incidents"][0]["state"], "persisted_local");
    assert_eq!(page["incidents"][0]["ack_class"], "persisted_local");
    assert!(
        page["incidents"][0]["created_at_unix"]
            .as_i64()
            .unwrap_or(0)
            > 0
    );
    assert!(
        page["next_cursor"].is_null(),
        "página completa não emite cursor: {page:?}"
    );

    // O caminho inverso também isola.
    let (status, raw) = list(addr, &token_b, "limit=100");
    assert_eq!(status, 200, "{raw}");
    assert_eq!(ids(&json_body(&raw)), vec![b1]);
}

// --- paginação ---------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duas_paginas_com_cursor_nao_repetem_nem_perdem() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-list-page");
    declare_tenant(&pool, &tenant, "BR").await;
    let token = unique("grant-page");
    let addr = serve(
        pool.clone(),
        vec![(token.clone(), allow_grant(&tenant, "grant-page"))],
    )
    .await;

    let created: Vec<String> = (0..3).map(|i| unique(&format!("inc-page-{i}"))).collect();
    for incident_id in &created {
        let (status, raw) = create(addr, &token, incident_id);
        assert_eq!(status, 201, "{raw}");
    }
    let expected = vec![created[2].clone(), created[1].clone(), created[0].clone()];

    let (status, raw) = list(addr, &token, "limit=2");
    assert_eq!(status, 200, "{raw}");
    let first = json_body(&raw);
    assert_eq!(ids(&first), expected[..2].to_vec());
    let next = first["next_cursor"]
        .as_str()
        .expect("há próxima página: next_cursor presente")
        .to_owned();

    let (status, raw) = list(addr, &token, &format!("limit=2&cursor={next}"));
    assert_eq!(status, 200, "cursor próprio precisa ser aceito: {raw}");
    let second = json_body(&raw);
    assert_eq!(ids(&second), expected[2..].to_vec());
    assert!(
        second["next_cursor"].is_null(),
        "fim do intervalo não emite cursor: {second:?}"
    );

    let mut all = ids(&first);
    all.extend(ids(&second));
    assert_eq!(all, expected, "nenhuma linha repetida nem perdida");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cursor_de_outro_tenant_e_cursor_incomparable() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant_a = unique("tenant-list-cur-a");
    let tenant_b = unique("tenant-list-cur-b");
    declare_tenant(&pool, &tenant_a, "BR").await;
    declare_tenant(&pool, &tenant_b, "BR").await;
    let token_a = unique("grant-cur-a");
    let token_b = unique("grant-cur-b");
    let addr = serve(
        pool.clone(),
        vec![
            (token_a.clone(), allow_grant(&tenant_a, "grant-cur-a")),
            (token_b.clone(), allow_grant(&tenant_b, "grant-cur-b")),
        ],
    )
    .await;

    for i in 0..2 {
        assert_eq!(
            create(addr, &token_a, &unique(&format!("inc-cur-{i}"))).0,
            201
        );
    }

    let (status, raw) = list(addr, &token_a, "limit=1");
    assert_eq!(status, 200, "{raw}");
    let cursor = json_body(&raw)["next_cursor"]
        .as_str()
        .expect("duas linhas com limit=1 emitem cursor")
        .to_owned();

    let (status, raw) = list(addr, &token_b, &format!("limit=1&cursor={cursor}"));
    assert_eq!(
        status, 400,
        "cursor de outro tenant precisa ser recusado: {raw}"
    );
    assert!(
        raw.contains("cursor_incomparable"),
        "código precisa ser nomeado: {raw}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cursor_de_outro_filtro_e_cursor_incomparable() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-list-filter");
    declare_tenant(&pool, &tenant, "BR").await;
    let token = unique("grant-filter");
    let addr = serve(
        pool.clone(),
        vec![(token.clone(), allow_grant(&tenant, "grant-filter"))],
    )
    .await;

    for i in 0..2 {
        assert_eq!(
            create(addr, &token, &unique(&format!("inc-filter-{i}"))).0,
            201
        );
    }

    // Cursor emitido sem filtro de estado...
    let (status, raw) = list(addr, &token, "limit=1");
    assert_eq!(status, 200, "{raw}");
    let cursor = json_body(&raw)["next_cursor"]
        .as_str()
        .expect("cursor emitido")
        .to_owned();

    // ...reapresentado com filtro: é outra consulta.
    let (status, raw) = list(
        addr,
        &token,
        &format!("limit=1&state=persisted_local&cursor={cursor}"),
    );
    assert_eq!(
        status, 400,
        "cursor de outro filtro precisa ser recusado: {raw}"
    );
    assert!(
        raw.contains("cursor_incomparable"),
        "código precisa ser nomeado: {raw}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cursor_ilegivel_e_cursor_invalid() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-list-bad");
    declare_tenant(&pool, &tenant, "BR").await;
    let token = unique("grant-bad");
    let addr = serve(
        pool.clone(),
        vec![(token.clone(), allow_grant(&tenant, "grant-bad"))],
    )
    .await;

    for cursor in ["nao-e-cursor", "v1.!!!", "v9.eyJhIjoxfQ"] {
        let (status, raw) = list(addr, &token, &format!("limit=1&cursor={cursor}"));
        assert_eq!(status, 400, "cursor {cursor:?} precisa ser recusado: {raw}");
        assert!(
            raw.contains("cursor_invalid"),
            "código precisa ser nomeado para {cursor:?}: {raw}"
        );
    }
}

// --- limit -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn limit_acima_do_teto_e_recusado_e_nao_truncado() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-list-limit");
    declare_tenant(&pool, &tenant, "BR").await;
    let token = unique("grant-limit");
    let addr = serve(
        pool.clone(),
        vec![(token.clone(), allow_grant(&tenant, "grant-limit"))],
    )
    .await;
    assert_eq!(create(addr, &token, &unique("inc-limit")).0, 201);

    for (query, motivo) in [
        ("limit=101", "acima do teto"),
        ("limit=0", "zero"),
        ("", "ausente (sem default inventado)"),
    ] {
        let (status, raw) = list(addr, &token, query);
        assert_eq!(status, 422, "limit {motivo} precisa ser 422: {raw}");
        assert!(
            raw.contains("invalid_request"),
            "código precisa ser nomeado para limit {motivo}: {raw}"
        );
    }

    // O teto em si é aceito: recusar não é proibir o limite publicado.
    let (status, raw) = list(addr, &token, "limit=100");
    assert_eq!(status, 200, "limit=100 é o teto declarado: {raw}");
}

// --- filtro de estado --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filtro_de_estado_restrito_ao_conjunto_fechado() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-list-state");
    declare_tenant(&pool, &tenant, "BR").await;
    let token = unique("grant-state");
    let addr = serve(
        pool.clone(),
        vec![(token.clone(), allow_grant(&tenant, "grant-state"))],
    )
    .await;
    let incident_id = unique("inc-state");
    assert_eq!(create(addr, &token, &incident_id).0, 201);

    let (status, raw) = list(addr, &token, "limit=10&state=persisted_local");
    assert_eq!(status, 200, "{raw}");
    assert_eq!(ids(&json_body(&raw)), vec![incident_id.clone()]);

    let (status, raw) = list(addr, &token, "limit=10&state=closed");
    assert_eq!(status, 200, "{raw}");
    assert!(
        ids(&json_body(&raw)).is_empty(),
        "estado sem incidente: lista vazia"
    );

    for query in ["limit=10&state=inventado", "limit=10&state="] {
        let (status, raw) = list(addr, &token, query);
        assert_eq!(status, 422, "{query} precisa ser recusado: {raw}");
        assert!(raw.contains("invalid_request"), "{raw}");
    }

    // Tenancy não pode vir da query.
    let (status, raw) = list(addr, &token, "limit=10&tenant_id=outro-tenant");
    assert_eq!(
        status, 422,
        "tenant_id na query precisa ser recusado: {raw}"
    );
}
