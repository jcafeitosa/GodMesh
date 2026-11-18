//! Ciclo de vida do incidente no servidor — contrato HTTP ponta a ponta.
//!
//! Cobre o caminho completo feliz (`persisted_local` → `replicated_to_authority`
//! → `acknowledged_technical` → `acknowledged_human` → `closed`), transição fora
//! de ordem (409 com o estado atual), reconhecimento humano sem prova (422),
//! isolamento entre tenants (404, nunca 403), concorrência na mesma transição
//! (exatamente uma sucesso e uma 409) e a trilha `sos_ack` append-only com
//! `at_unix` real.
//!
//! Banco próprio (não o de dev compartilhado). O esquema é aplicado pelo runner
//! oficial (`tests/common/mod.rs`), não por psql manual:
//!   createdb godmesh_sc
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sc \
//!     cargo test -p godmesh-server --test lifecycle
//!
//! Sem DATABASE_URL os testes são ignorados (não fingem sucesso).

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

const TOKEN_A: &str = "grant-opaco-sc-tenant-a";
const TOKEN_B: &str = "grant-opaco-sc-tenant-b";
const TENANT_A: &str = "tenant-sc-a";
const TENANT_B: &str = "tenant-sc-b";

/// Serializa este binário: as asserções de contagem global só valem sem outro
/// teste inserindo no meio (o banco é dedicado, mas `cargo test` paraleliza).
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Sufixo único por execução: o banco pode ser reaproveitado entre rodadas.
static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_id(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("inc-sc-{tag}-{}-{n}-{nanos}", std::process::id())
}

fn allow_grant(tenant_id: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-sc".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::OpenSosIncident,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
        token_id: "test-grant".to_owned(),
        scopes: Scope::ALL.to_vec(),
        authority_epoch: 1,
    }
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_entries(vec![
        (TOKEN_A.to_owned(), allow_grant(TENANT_A)),
        (TOKEN_B.to_owned(), allow_grant(TENANT_B)),
    ])
}

/// Banco de teste migrado pelo runner oficial (`tests/common/mod.rs`).
async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(8).await
}

/// Declara jurisdição e política do tenant de teste (a API não cria nenhuma).
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

/// Cliente HTTP/1.1 cru: status e corpo passam pelo fio, sem intermediário que
/// pudesse normalizar o que o servidor realmente devolve.
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
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

fn json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("corpo não é JSON ({e}): {raw:?}"))
}

async fn create(pool: &PgPool, addr: SocketAddr, id: &str) -> Value {
    let _ = pool;
    let token = format!("Bearer {TOKEN_A}");
    let id_owned = id.to_owned();
    let body = format!("{{\"incident_id\":\"{id_owned}\"}}");
    let (status, raw) =
        tokio::task::spawn_blocking(move || http(addr, "POST", "/v1/incidents", Some(&token), Some(&body)))
            .await
            .expect("cliente HTTP");
    assert_eq!(status, 201, "criação precisa ser 201: {raw}");
    json(&raw)
}

async fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: &str,
    body: Option<String>,
) -> (u16, Value) {
    let token = format!("Bearer {token}");
    let path = path.to_owned();
    let method = method.to_owned();
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http(addr, &method, &path, Some(&token), body.as_deref())
    })
    .await
    .expect("cliente HTTP");
    (status, json(&raw))
}

fn ack_classes(view: &Value) -> Vec<String> {
    view["acks"]
        .as_array()
        .expect("acks é lista")
        .iter()
        .map(|a| a["ack_class"].as_str().expect("ack_class string").to_owned())
        .collect()
}

// --- caminho completo feliz---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_happy_path_derives_every_label_from_the_state() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("feliz");

    let created = create(&pool, addr, &id).await;
    assert_eq!(created["state"], "persisted_local");
    assert_eq!(created["ack_class"], "persisted_local");

    let (status, got) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200, "GET do próprio incidente: {got}");
    assert_eq!(got["incident_id"], id);
    assert_eq!(ack_classes(&got), vec!["persisted_local".to_owned()]);

    let (status, replicated) =
        call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(status, 200, "replicate do estado persistido: {replicated}");
    assert_eq!(replicated["state"], "replicated_to_authority");
    assert_eq!(replicated["ack_class"], "replicated_to_authority");
    assert_eq!(replicated["ui_label"], "replicado à autoridade, sem confirmação");

    let (status, technical) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/ack-technical"),
        TOKEN_A,
        None,
    )
    .await;
    assert_eq!(status, 200, "ack-technical: {technical}");
    assert_eq!(technical["state"], "acknowledged_technical");
    assert_eq!(technical["ack_class"], "materialized");

    let (status, human) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/acknowledge"),
        TOKEN_A,
        Some("{\"responder\":\"maria-plantonista\"}".to_owned()),
    )
    .await;
    assert_eq!(status, 200, "acknowledge com pessoa: {human}");
    assert_eq!(human["state"], "acknowledged_human");
    assert_eq!(human["ack_class"], "acknowledged_human");
    assert!(
        !human["ui_label"].as_str().unwrap_or_default().contains("entrega"),
        "rótulo não promete entrega: {}",
        human["ui_label"]
    );

    let (status, closed) =
        call(addr, "POST", &format!("/v1/incidents/{id}/close"), TOKEN_A, None).await;
    assert_eq!(status, 200, "close a partir do ACK humano: {closed}");
    assert_eq!(closed["state"], "closed");
    // O ACK final permanece: encerrar não apaga nem reescreve evidência.
    assert_eq!(closed["ack_class"], "acknowledged_human");

    let (status, final_view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        ack_classes(&final_view),
        vec![
            "persisted_local".to_owned(),
            "replicated_to_authority".to_owned(),
            "materialized".to_owned(),
            "acknowledged_human".to_owned(),
        ],
        "trilha append-only na ordem das transições"
    );
    for ack in final_view["acks"].as_array().expect("acks") {
        let at = ack["at_unix"].as_i64().expect("at_unix inteiro");
        assert!(at > 0, "ACK sem instante real não é evidência: {ack}");
    }
    assert_eq!(
        final_view["acks"][3]["source"], "maria-plantonista",
        "ACK humano registra a pessoa que reconheceu"
    );
    assert_eq!(final_view["acks"][1]["source"], "authority:godmesh-server");
}

// --- transição fora de ordem ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transition_out_of_order_is_409_with_the_current_state() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("fora-de-ordem");
    create(&pool, addr, &id).await;

    // Pular direto para o reconhecimento humano a partir de `persisted_local`.
    let (status, body) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/acknowledge"),
        TOKEN_A,
        Some("{\"responder\":\"maria-plantonista\"}".to_owned()),
    )
    .await;
    assert_eq!(status, 409, "transição ilegal precisa ser 409: {body}");
    assert_eq!(body["error"], "illegal_transition");
    assert_eq!(body["state"], "persisted_local", "corpo carrega o estado atual");
    assert_eq!(body["ack_class"], "transport_accepted", "erro não alega ACK");

    // Nada mudou: o estado e a trilha continuam os de antes da recusa.
    let (_, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(view["state"], "persisted_local");
    assert_eq!(ack_classes(&view), vec!["persisted_local".to_owned()]);
}

// --- prova de pessoa obrigatória -------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledge_without_responder_is_422_missing_human_proof() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("sem-prova");
    create(&pool, addr, &id).await;
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(s, 200);
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/ack-technical"), TOKEN_A, None).await;
    assert_eq!(s, 200);

    for body in [
        Some("{}".to_owned()),
        Some("{\"responder\":\"   \"}".to_owned()),
        None,
    ] {
        let (status, refused) = call(
            addr,
            "POST",
            &format!("/v1/incidents/{id}/acknowledge"),
            TOKEN_A,
            body.clone(),
        )
        .await;
        assert_eq!(status, 422, "sem prova de pessoa precisa ser 422 ({body:?}): {refused}");
        assert_eq!(refused["error"], "missing_human_proof");
    }

    let (_, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(view["state"], "acknowledged_technical", "recusa não transiciona");
    assert_eq!(
        ack_classes(&view),
        vec![
            "persisted_local".to_owned(),
            "replicated_to_authority".to_owned(),
            "materialized".to_owned(),
        ],
        "recusa não grava ACK humano"
    );
}

// --- isolamento entre tenants ----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incident_of_another_tenant_is_404_not_403() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    declare_tenant(&pool, TENANT_B, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("alheio");
    create(&pool, addr, &id).await;

    let (status, body) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_B, None).await;
    assert_eq!(status, 404, "incidente de outro tenant não vaza existência: {body}");
    assert_eq!(body["error"], "incident_not_found");
    assert_ne!(status, 403, "403 confirmaria a existência do incidente alheio");

    let (status, body) =
        call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_B, None).await;
    assert_eq!(status, 404, "transição de outro tenant também é 404: {body}");

    // Inexistente responde exatamente o mesmo.
    let missing = unique_id("inexistente");
    let (status, body) = call(addr, "GET", &format!("/v1/incidents/{missing}"), TOKEN_A, None).await;
    assert_eq!(status, 404, "inexistente é 404: {body}");
    assert_eq!(body["error"], "incident_not_found");

    // O dono segue intacto.
    let (status, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200);
    assert_eq!(view["state"], "persisted_local", "nenhuma tentativa alheia transiciona");
}

// --- concorrência -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_transition_yields_exactly_one_success_and_one_409() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("corrida");
    create(&pool, addr, &id).await;

    let path = format!("/v1/incidents/{id}/replicate");
    let one = {
        let path = path.clone();
        let token = format!("Bearer {TOKEN_A}");
        tokio::task::spawn_blocking(move || http(addr, "POST", &path, Some(&token), None))
    };
    let two = {
        let path = path.clone();
        let token = format!("Bearer {TOKEN_A}");
        tokio::task::spawn_blocking(move || http(addr, "POST", &path, Some(&token), None))
    };
    let (first, second) = tokio::join!(one, two);
    let (first, second) = (first.expect("1ª chamada"), second.expect("2ª chamada"));
    let mut statuses = vec![first.0, second.0];
    statuses.sort_unstable();
    assert_eq!(
        statuses,
        vec![200, 409],
        "exatamente uma sucesso e uma 409: {first:?} / {second:?}"
    );
    let conflict = if first.0 == 409 { &first.1 } else { &second.1 };
    assert!(
        conflict.contains("illegal_transition"),
        "conflito precisa ser nomeado: {conflict}"
    );

    // A trilha tem exatamente uma linha `replicated_to_authority`: o ACK não
    // duplica sob concorrência.
    let acks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sos_ack WHERE incident_id = $1 AND ack_class = 'replicated_to_authority'",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("contagem de ACK");
    assert_eq!(acks, 1, "ACK duplicado sob concorrência");

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_ack WHERE incident_id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .expect("contagem de ACK");
    assert_eq!(total, 2, "persisted_local + replicated_to_authority, nada mais");
}

// --- trilha append-only ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ack_trail_is_append_only_and_records_real_instants() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("append-only");
    create(&pool, addr, &id).await;
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(s, 200);

    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT ack_class, at_unix FROM sos_ack WHERE incident_id = $1 ORDER BY recorded_at, at_unix, ack_class")
            .bind(&id)
            .fetch_all(&pool)
            .await
            .expect("trilha");
    assert_eq!(rows.len(), 2, "duas fronteiras atravessadas, duas linhas");
    assert_eq!(rows[0].0, "persisted_local");
    assert_eq!(rows[1].0, "replicated_to_authority");
    for (class, at) in &rows {
        assert!(*at > 0, "at_unix real obrigatório em {class}");
    }
    assert!(
        rows[0].1 <= rows[1].1,
        "instantes não podem retroceder: {rows:?}"
    );

    // O banco recusa reescrever a trilha (trigger `sos_ack_append_only`).
    let update = sqlx::query("UPDATE sos_ack SET at_unix = at_unix + 1 WHERE incident_id = $1")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(update.is_err(), "UPDATE em sos_ack precisa ser recusado pelo banco");
    let delete = sqlx::query("DELETE FROM sos_ack WHERE incident_id = $1")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "DELETE em sos_ack precisa ser recusado pelo banco");
}