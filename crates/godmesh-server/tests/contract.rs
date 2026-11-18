//! Contrato do endpoint de incidente: a resposta não pode prometer mais do que
//! a persistência local prova (ADR-058), a política de retenção vem da jurisdição
//! declarada do tenant (ADR-062/066) e o replay é idempotente por tenant.
//!
//! Exige um Postgres com as migrações 0001 e 0002 aplicadas:
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh
//!
//! Sem DATABASE_URL os testes são ignorados (não fingem sucesso).

use godmesh_identity::grant::{FreshnessSource, OfflineOp, RevocationState};
use godmesh_server::api::{create_incident, ApiError, IncidentPayload};
use sqlx::PgPool;

const TENANT_BR: &str = "tenant-1";
const TENANT_CO: &str = "tenant-co";

fn payload(incident_id: &str, tenant: &str, operation: OfflineOp, freshness: FreshnessSource) -> IncidentPayload {
    IncidentPayload {
        incident_id: incident_id.to_owned(),
        tenant_id: tenant.to_owned(),
        principal: "principal-42".to_owned(),
        operation,
        freshness,
        revocation: RevocationState::Absent,
        epoch_comparable: true,
        action_enumerated: true,
        // A classe incidente_sos tem política aplicável na migração; sem isso o
        // predicado nega e o caminho feliz nem seria exercitado.
        retention_class_known: true,
    }
}

/// Aceita o incidente com grant válido, para os testes que precisam de um já gravado.
fn happy(incident_id: &str, tenant: &str) -> IncidentPayload {
    payload(incident_id, tenant, OfflineOp::OpenSosIncident, FreshnessSource::F3VerifiedOnline)
}

async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .ok()
}

/// Identificador único por execução: os testes compartilham o mesmo banco.
fn fresh_id(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("inc-{label}-{}-{nanos}", std::process::id())
}

/// Registra a jurisdição declarada do tenant (ADR-066) e garante a semente da
/// política de `incidente_sos` para essa jurisdição.
///
/// Não é fixture de produto: a API não cria tenant nem política, ela resolve o que
/// já está declarado. O `INSERT ... ON CONFLICT DO NOTHING` é idempotente com a
/// semente das migrações e deixa cada teste independente do estado deixado por
/// execuções anteriores.
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

#[tokio::test]
async fn denies_incident_without_trusted_freshness_source() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let r = create_incident(
        &pool,
        payload(&fresh_id("no-freshness"), TENANT_BR, OfflineOp::OpenSosIncident, FreshnessSource::None),
    )
    .await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "sem fonte de confiança precisa negar: {r:?}");
}

#[tokio::test]
async fn persists_incident_and_never_claims_more_than_persisted_local() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let outcome = create_incident(&pool, happy(&fresh_id("persist"), TENANT_BR))
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
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let r = create_incident(
        &pool,
        payload(&fresh_id("precise"), TENANT_BR, OfflineOp::SharePreciseLocation, FreshnessSource::F3VerifiedOnline),
    )
    .await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "posição precisa offline é sempre negada: {r:?}");
}

#[tokio::test]
async fn colombian_tenant_uses_colombian_policy() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_CO, "CO").await;
    let id = fresh_id("co");
    let outcome = create_incident(&pool, happy(&id, TENANT_CO))
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
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    let id = fresh_id("retry");

    let first = create_incident(&pool, happy(&id, TENANT_BR)).await.expect("primeira gravação");
    assert!(first.created);

    let replay = create_incident(&pool, happy(&id, TENANT_BR))
        .await
        .expect("replay idêntico do mesmo tenant não pode falhar (não é 503)");
    assert!(!replay.created, "replay não cria nada novo");
    assert_eq!(replay.response, first.response, "replay repete o ack_class original");

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE tenant_id = $1 AND id = $2")
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
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_BR, "BR").await;
    declare_tenant(&pool, TENANT_CO, "CO").await;
    let id = fresh_id("squat");

    let owner = create_incident(&pool, happy(&id, TENANT_BR)).await.expect("dono do id grava");
    assert!(owner.created);

    let squatter = create_incident(&pool, happy(&id, TENANT_CO)).await;
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
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let tenant = fresh_id("no-region");
    let r = create_incident(&pool, happy(&fresh_id("no-region-inc"), &tenant)).await;
    assert!(
        matches!(r, Err(ApiError::RetentionScopeUnresolved)),
        "sem jurisdição declarada a persistência é bloqueada (ADR-062/066): {r:?}"
    );
}