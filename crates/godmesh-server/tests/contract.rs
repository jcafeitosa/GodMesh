//! Contrato do endpoint de incidente: a resposta não pode prometer mais do que
//! a persistência local prova (ADR-058). Exige um Postgres com a migração aplicada:
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh
//!
//! Sem DATABASE_URL os testes são ignorados (não fingem sucesso).

use godmesh_identity::grant::{FreshnessSource, OfflineOp, RevocationState};
use godmesh_server::api::{create_incident, IncidentPayload, ApiError};

fn payload(operation: OfflineOp, freshness: FreshnessSource) -> IncidentPayload {
    IncidentPayload {
        incident_id: format!("inc-{}", std::process::id()),
        tenant_id: "tenant-1".to_owned(),
        principal: "principal-42".to_owned(),
        operation,
        freshness,
        revocation: RevocationState::Absent,
        epoch_comparable: true,
        action_enumerated: true,
        // A classe incidente_sos tem política aplicável na migração de dev; sem
        // isso o predicado nega e o caminho feliz nem seria exercitado.
        retention_class_known: true,
    }
}

async fn pool() -> Option<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    sqlx::postgres::PgPoolOptions::new().max_connections(2).connect(&url).await.ok()
}

#[tokio::test]
async fn denies_incident_without_trusted_freshness_source() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let r = create_incident(&pool, payload(OfflineOp::OpenSosIncident, FreshnessSource::None)).await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "sem fonte de confiança precisa negar: {r:?}");
}

#[tokio::test]
async fn persists_incident_and_never_claims_more_than_persisted_local() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let r = create_incident(&pool, payload(OfflineOp::OpenSosIncident, FreshnessSource::F3VerifiedOnline))
        .await
        .expect("incidente aceito com grant válido");

    assert_eq!(r.ack_class, "persisted_local");
    assert_eq!(r.state, "persisted_local");
    assert_ne!(r.ack_class, "delivered_to_device");
    assert_ne!(r.ack_class, "acknowledged_human");
}

#[tokio::test]
async fn refuses_precise_location_offline() {
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    let r = create_incident(&pool, payload(OfflineOp::SharePreciseLocation, FreshnessSource::F3VerifiedOnline)).await;
    assert!(matches!(r, Err(ApiError::GrantDenied(_))), "posição precisa offline é sempre negada: {r:?}");
}
