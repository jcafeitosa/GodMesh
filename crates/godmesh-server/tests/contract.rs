//! Contrato do endpoint de incidente.
//!
//! Duas autoridades compõem esta fatia e nenhuma das duas é negociável:
//!  * **auth** — a resposta não pode prometer mais do que a persistência local
//!    prova (ADR-058) e o pedido não pode conceder a si mesmo tenancy nem
//!    autorização: `tenant_id`, `principal` e o `GrantPredicate` vêm do grant
//!    verificado, nunca do corpo;
//!  * **retenção** — a política vem da jurisdição declarada do tenant
//!    (ADR-062/066) e o replay é idempotente por tenant.
//!
//! Exige um Postgres com as migrações 0001 e 0002 aplicadas:
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh
//!
//! Sem DATABASE_URL os testes são ignorados (não fingem sucesso).

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantDecision, GrantPredicate, OfflineOp, RevocationState,
};
use godmesh_server::api::{create_incident, ApiError, IncidentPayload};
use godmesh_server::auth::{GrantRegistry, Subject, VerifiedGrant};
use sqlx::PgPool;

const TENANT_BR: &str = "tenant-1";
const TENANT_CO: &str = "tenant-co";

/// Token opaco do grant de teste. Não é segredo real: vive só no processo de teste.
const TOKEN_TENANT_A: &str = "grant-opaco-tenant-a";
const TOKEN_EMPTY_TENANT: &str = "grant-opaco-sem-tenant";

/// Sufixo único por chamada: o banco de dev é compartilhado entre execuções.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// `count(*)` global só é evidência se nenhum outro teste inserir no meio.
/// Os testes que tocam o banco seguram este cadeado: serializa este binário
/// (não protege contra outro processo usando o mesmo banco). Mutex async porque
/// o cadeado vive através de awaits de banco e de HTTP.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn db_guard() -> tokio::sync::MutexGuard<'static, ()> {
    DB_LOCK.lock().await
}

fn unique_id(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("inc-{tag}-{}-{n}", std::process::id())
}

/// Corpo do pedido: só o identificador. Tenancy e grant são do `Authorization`.
fn payload(incident_id: &str) -> IncidentPayload {
    IncidentPayload {
        incident_id: incident_id.to_owned(),
    }
}

fn allow_predicate(operation: OfflineOp) -> GrantPredicate {
    GrantPredicate {
        operation,
        epoch_comparable: true,
        revocation: RevocationState::Absent,
        freshness: FreshnessSource::F3VerifiedOnline,
        // A classe incidente_sos tem política aplicável na migração de dev; sem
        // isso o predicado nega e o caminho feliz nem seria exercitado.
        retention_class_known: true,
        action_enumerated: true,
    }
}

fn grant(tenant_id: &str, predicate: GrantPredicate) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-42".to_owned(),
        },
        predicate,
    }
}

fn allow_grant(tenant_id: &str) -> VerifiedGrant {
    grant(tenant_id, allow_predicate(OfflineOp::OpenSosIncident))
}

async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .ok()
}

/// Registra a jurisdição declarada do tenant (ADR-066) e garante a semente da
/// política de `incidente_sos` para essa jurisdição.
///
/// Não é fixture de produto: a API não cria tenant nem política, ela resolve o que
/// já está declarado. O `INSERT ... ON CONFLICT DO NOTHING` é idempotente com a
/// semente das migrações e deixa cada teste independente do estado deixado por
/// execuções anteriores.
///
/// Sem esta linha a persistência falha fechado (`retention_scope_unresolved`),
/// que é exatamente o que ADR-066 exige; um teste que quer o caminho feliz
/// precisa declarar a jurisdição do sujeito que o grant identifica.
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

async fn incident_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sos_incident")
        .fetch_one(pool)
        .await
        .expect("count(*) de sos_incident")
}

async fn rows_for(pool: &PgPool, incident_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(incident_id)
        .fetch_one(pool)
        .await
        .expect("count(*) por id")
}

// --- transporte ---------------------------------------------------------------

/// Sobe o router real em porta efêmera. O teste fala HTTP/1.1 cru de propósito:
/// `401` é status de transporte, não de caso de uso, e precisa ser provado no fio.
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

fn http_post(addr: SocketAddr, authorization: Option<&str>, body: &str) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de escrita");
    let mut request = format!(
        "POST /v1/incidents HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
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

fn registry(entries: Vec<(String, VerifiedGrant)>) -> GrantRegistry {
    GrantRegistry::from_entries(entries)
}

fn tenant_a_registry() -> GrantRegistry {
    registry(vec![(TOKEN_TENANT_A.to_owned(), allow_grant("tenant-a"))])
}

/// Corpo cru, para exercitar campos que o schema não aceita mais.
fn raw_body(incident_id: &str, extra: &str) -> String {
    format!("{{\"incident_id\":\"{incident_id}\"{extra}}}")
}

// --- autenticação -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_without_credential_is_401_and_inserts_nothing() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let addr = serve(pool.clone(), tenant_a_registry()).await;
    let before = incident_count(&pool).await;
    let incident_id = unique_id("sem-credencial");

    let (status, raw) =
        tokio::task::spawn_blocking(move || http_post(addr, None, &raw_body(&incident_id, "")))
            .await
            .expect("cliente HTTP");

    assert_eq!(status, 401, "sem credencial precisa ser 401: {raw}");
    assert!(raw.contains("unauthenticated"), "corpo precisa nomear a recusa: {raw}");
    assert!(
        raw.to_ascii_lowercase().contains("www-authenticate: bearer"),
        "401 precisa anunciar o esquema: {raw}"
    );
    assert_eq!(incident_count(&pool).await, before, "401 não pode inserir");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_credential_is_401_and_inserts_nothing() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let addr = serve(pool.clone(), tenant_a_registry()).await;
    let before = incident_count(&pool).await;
    let incident_id = unique_id("credencial-desconhecida");

    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(
            addr,
            Some("Bearer grant-que-nao-existe"),
            &raw_body(&incident_id, ""),
        )
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401, "token desconhecido precisa ser 401: {raw}");
    assert_eq!(incident_count(&pool).await, before, "401 não pode inserir");
}

// --- tenancy ------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grant_of_tenant_a_declaring_tenant_b_is_refused() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let addr = serve(pool.clone(), tenant_a_registry()).await;
    let before = incident_count(&pool).await;
    let incident_id = unique_id("tenant-divergente");
    let body = raw_body(
        &incident_id,
        ",\"tenant_id\":\"tenant-b\",\"principal\":\"intruso\"",
    );

    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(
            addr,
            Some(&format!("Bearer {TOKEN_TENANT_A}")),
            &body,
        )
    })
    .await
    .expect("cliente HTTP");

    assert!(
        (400..500).contains(&status),
        "corpo que declara tenancy precisa ser 4xx: {status} {raw}"
    );
    assert!(status != 401, "credencial era válida; a recusa é do corpo: {raw}");
    assert!(
        raw.contains("invalid_request"),
        "recusa precisa ser estável e nomeada: {raw}"
    );
    assert_eq!(rows_for(&pool, &incident_id).await, 0, "nada pode ser gravado");
    assert_eq!(incident_count(&pool).await, before, "contagem inalterada");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_with_empty_tenant_is_refused_before_insert() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let addr = serve(pool.clone(), tenant_a_registry()).await;
    let before = incident_count(&pool).await;
    let incident_id = unique_id("tenant-vazio");
    let body = raw_body(&incident_id, ",\"tenant_id\":\"\"");

    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(
            addr,
            Some(&format!("Bearer {TOKEN_TENANT_A}")),
            &body,
        )
    })
    .await
    .expect("cliente HTTP");

    assert!(
        (400..500).contains(&status),
        "tenant vazio no corpo precisa ser 4xx: {status} {raw}"
    );
    assert_eq!(rows_for(&pool, &incident_id).await, 0, "nada pode ser gravado");
    assert_eq!(incident_count(&pool).await, before, "contagem inalterada");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credential_without_authenticated_tenant_is_refused_before_insert() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let grants = registry(vec![(TOKEN_EMPTY_TENANT.to_owned(), allow_grant(""))]);
    let addr = serve(pool.clone(), grants).await;
    let before = incident_count(&pool).await;
    let incident_id = unique_id("grant-sem-tenant");

    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(
            addr,
            Some(&format!("Bearer {TOKEN_EMPTY_TENANT}")),
            &raw_body(&incident_id, ""),
        )
    })
    .await
    .expect("cliente HTTP");

    assert!(
        (400..500).contains(&status),
        "sujeito sem tenant precisa ser 4xx: {status} {raw}"
    );
    assert!(raw.contains("invalid_credential"), "recusa precisa ser nomeada: {raw}");
    assert_eq!(incident_count(&pool).await, before, "nada pode ser gravado");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tenant_and_principal_come_from_the_verified_grant() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    // A jurisdição é dado do servidor (ADR-066): sem ela a persistência falha
    // fechado, então o caminho feliz deste teste precisa declará-la.
    declare_tenant(&pool, "tenant-a", "BR").await;
    let addr = serve(pool.clone(), tenant_a_registry()).await;
    let incident_id = unique_id("tenant-do-grant");
    let expected_id = incident_id.clone();

    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(
            addr,
            Some(&format!("Bearer {TOKEN_TENANT_A}")),
            &raw_body(&incident_id, ""),
        )
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 201, "grant válido precisa ser aceito: {raw}");
    assert!(raw.contains("persisted_local"), "ACK precisa ser o persistido: {raw}");

    let (tenant_id, principal, jurisdiction): (String, String, String) = sqlx::query_as(
        "SELECT tenant_id, principal, retention_jurisdiction FROM sos_incident WHERE id = $1",
    )
    .bind(&expected_id)
    .fetch_one(&pool)
    .await
    .expect("linha do incidente");
    assert_eq!(tenant_id, "tenant-a", "tenant deriva do grant, não do corpo");
    assert_eq!(principal, "principal-42", "principal deriva do grant");
    assert_eq!(
        jurisdiction, "BR",
        "jurisdição deriva de tenant_region do tenant autenticado"
    );
}

// --- exaustividade do predicado ----------------------------------------------

/// Guarda de exaustividade: `match` **sem curinga**. Quando `GrantDecision`
/// ganhar uma variante nova, este teste deixa de compilar e obriga revisão antes
/// de a nova decisão poder autorizar qualquer coisa.
fn only_allow_authorizes(decision: GrantDecision) -> bool {
    match decision {
        GrantDecision::Allow => true,
        GrantDecision::DenyNeverOffline
        | GrantDecision::DenyNotEnumerated
        | GrantDecision::DenyDeniedNoTrustedTime
        | GrantDecision::DenyEpochIncomparable
        | GrantDecision::DenyRevocationUnknown
        | GrantDecision::DenyRetentionPolicyMissing => false,
    }
}

#[test]
fn only_allow_is_an_acceptance_path() {
    let decisions = [
        GrantDecision::Allow,
        GrantDecision::DenyNeverOffline,
        GrantDecision::DenyNotEnumerated,
        GrantDecision::DenyDeniedNoTrustedTime,
        GrantDecision::DenyEpochIncomparable,
        GrantDecision::DenyRevocationUnknown,
        GrantDecision::DenyRetentionPolicyMissing,
    ];
    for decision in decisions {
        assert_eq!(
            only_allow_authorizes(decision),
            decision == GrantDecision::Allow,
            "apenas Allow aceita; {decision:?} precisa negar"
        );
    }
}

#[tokio::test]
async fn every_denial_variant_denies_without_inserting() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let mut unknown_revocation = allow_predicate(OfflineOp::OpenSosIncident);
    unknown_revocation.revocation = RevocationState::Unknown;
    let mut known_revocation = allow_predicate(OfflineOp::OpenSosIncident);
    known_revocation.revocation = RevocationState::Known;
    let mut no_enumeration = allow_predicate(OfflineOp::OpenSosIncident);
    no_enumeration.action_enumerated = false;
    let mut no_freshness = allow_predicate(OfflineOp::OpenSosIncident);
    no_freshness.freshness = FreshnessSource::None;
    let mut no_epoch = allow_predicate(OfflineOp::OpenSosIncident);
    no_epoch.epoch_comparable = false;
    let mut no_retention = allow_predicate(OfflineOp::OpenSosIncident);
    no_retention.retention_class_known = false;

    let cases = [
        (
            GrantDecision::DenyNeverOffline,
            allow_predicate(OfflineOp::SharePreciseLocation),
        ),
        (GrantDecision::DenyNeverOffline, allow_predicate(OfflineOp::FederatedMutation)),
        (GrantDecision::DenyNeverOffline, allow_predicate(OfflineOp::CreateOrEditGrant)),
        (GrantDecision::DenyNotEnumerated, no_enumeration),
        (GrantDecision::DenyDeniedNoTrustedTime, no_freshness),
        (GrantDecision::DenyEpochIncomparable, no_epoch),
        (GrantDecision::DenyRevocationUnknown, unknown_revocation),
        (GrantDecision::DenyRevocationUnknown, known_revocation),
        (GrantDecision::DenyRetentionPolicyMissing, no_retention),
    ];

    let before = incident_count(&pool).await;
    for (expected, predicate) in cases {
        let incident_id = unique_id("negacao");
        let result = create_incident(&pool, &grant(TENANT_BR, predicate), payload(&incident_id)).await;
        match result {
            Err(ApiError::GrantDenied(actual)) => assert_eq!(
                actual, expected,
                "negação precisa ser a do predicado para {expected:?}"
            ),
            other => panic!("esperava GrantDenied({expected:?}), veio {other:?}"),
        }
        assert_eq!(rows_for(&pool, &incident_id).await, 0, "negação não insere");
    }
    assert_eq!(incident_count(&pool).await, before, "nenhuma negação pode inserir");
}

// --- comportamento herdado do núcleo -----------------------------------------

#[tokio::test]
async fn denies_incident_without_trusted_freshness_source() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let mut predicate = allow_predicate(OfflineOp::OpenSosIncident);
    predicate.freshness = FreshnessSource::None;
    let incident_id = unique_id("sem-frescor");
    let r = create_incident(&pool, &grant(TENANT_BR, predicate), payload(&incident_id)).await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "sem fonte de confiança precisa negar: {r:?}");
    assert_eq!(rows_for(&pool, &incident_id).await, 0);
}

#[tokio::test]
async fn persists_incident_and_never_claims_more_than_persisted_local() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let incident_id = unique_id("persistido");
    let outcome = create_incident(&pool, &allow_grant(TENANT_BR), payload(&incident_id))
        .await
        .expect("incidente aceito com grant válido");

    assert!(outcome.created, "primeira gravação precisa ser criação");
    let r = outcome.response;
    assert_eq!(r.ack_class, "persisted_local");
    assert_eq!(r.state, "persisted_local");
    assert_ne!(r.ack_class, "delivered_to_device");
    assert_ne!(r.ack_class, "acknowledged_human");
}

#[tokio::test]
async fn refuses_precise_location_offline() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let incident_id = unique_id("posicao-precisa");
    let r = create_incident(
        &pool,
        &grant(TENANT_BR, allow_predicate(OfflineOp::SharePreciseLocation)),
        payload(&incident_id),
    )
    .await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "posição precisa offline é sempre negada: {r:?}");
    assert_eq!(rows_for(&pool, &incident_id).await, 0);
}

// --- retenção por jurisdição do tenant (ADR-062/066) -------------------------

#[tokio::test]
async fn colombian_tenant_uses_colombian_policy() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_CO, "CO").await;
    let id = unique_id("co");
    let outcome = create_incident(&pool, &allow_grant(TENANT_CO), payload(&id))
        .await
        .expect("CO tem política e cópia primária de incidente_sos (ADR-065/066)");
    assert_eq!(outcome.response.ack_class, "persisted_local");

    let (class, jurisdiction): (String, String) = sqlx::query_as(
        "SELECT retention_class, retention_jurisdiction FROM sos_incident WHERE tenant_id = $1 AND id = $2",
    )
    .bind(TENANT_CO)
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("incidente CO gravado");
    assert_eq!((class.as_str(), jurisdiction.as_str()), ("incidente_sos", "CO"));
}

#[tokio::test]
async fn identical_retry_is_idempotent_and_returns_the_original_ack() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let id = unique_id("retry");
    let grant = allow_grant(TENANT_BR);

    let first = create_incident(&pool, &grant, payload(&id))
        .await
        .expect("primeira gravação");
    assert!(first.created);

    let replay = create_incident(&pool, &grant, payload(&id))
        .await
        .expect("replay idêntico do mesmo tenant não pode falhar (não é 503)");
    assert!(!replay.created, "replay não cria nada novo");
    assert_eq!(replay.response, first.response, "replay repete o ack_class original");

    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE tenant_id = $1 AND id = $2")
            .bind(TENANT_BR)
            .bind(&id)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(rows, 1, "replay não duplica o incidente");

    let acks: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_ack WHERE incident_id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .expect("contagem de ACK");
    assert_eq!(acks, 1, "replay não reescreve a trilha append-only");
}

#[tokio::test]
async fn incident_id_of_another_tenant_is_a_conflict_not_a_second_incident() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    declare_tenant(&pool, TENANT_CO, "CO").await;
    let id = unique_id("squat");

    let owner = create_incident(&pool, &allow_grant(TENANT_BR), payload(&id))
        .await
        .expect("dono do id grava");
    assert!(owner.created);

    let squatter = create_incident(&pool, &allow_grant(TENANT_CO), payload(&id)).await;
    assert!(
        matches!(squatter, Err(ApiError::IncidentIdConflict)),
        "id de outro tenant precisa ser 409 incident_id_conflict, não 503 nem segundo incidente: {squatter:?}"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(rows, 1, "o tenant vizinho não cria um segundo incidente com o mesmo id");
}

#[tokio::test]
async fn tenant_without_declared_jurisdiction_fails_closed() {
    let _guard = db_guard().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let tenant = unique_id("no-region");
    let r = create_incident(&pool, &allow_grant(&tenant), payload(&unique_id("no-region-inc"))).await;
    assert!(
        matches!(r, Err(ApiError::RetentionScopeUnresolved)),
        "sem jurisdição declarada a persistência é bloqueada (ADR-062/066): {r:?}"
    );
}

// --- configuração do registro de grants --------------------------------------

#[test]
fn grant_registry_config_rejects_unknown_fields_and_duplicates() {
    let valid = r#"{"grants":[{"token":"t1","tenant_id":"tenant-a","principal":"p1",
        "operation":"open_sos_incident","freshness":"f3_verified_online",
        "revocation":"absent","epoch_comparable":true,"action_enumerated":true,
        "retention_class_known":true}]}"#;
    assert_eq!(GrantRegistry::from_json(valid).expect("config válida").len(), 1);

    let unknown_field = r#"{"grants":[{"token":"t1","tenant_id":"tenant-a","principal":"p1",
        "operation":"open_sos_incident","freshness":"f3_verified_online",
        "revocation":"absent","epoch_comparable":true,"action_enumerated":true,
        "retention_class_known":true,"tenant":"outro"}]}"#;
    assert!(GrantRegistry::from_json(unknown_field).is_err());

    let duplicate = r#"{"grants":[
        {"token":"t1","tenant_id":"a","principal":"p","operation":"open_sos_incident",
         "freshness":"f3_verified_online","revocation":"absent","epoch_comparable":true,
         "action_enumerated":true,"retention_class_known":true},
        {"token":"t1","tenant_id":"b","principal":"p","operation":"open_sos_incident",
         "freshness":"f3_verified_online","revocation":"absent","epoch_comparable":true,
         "action_enumerated":true,"retention_class_known":true}]}"#;
    assert!(GrantRegistry::from_json(duplicate).is_err());
}

#[test]
fn empty_registry_verifies_nothing() {
    let grants = GrantRegistry::default();
    assert!(grants.is_empty());
    assert!(grants.verify("qualquer-token").is_none());
}