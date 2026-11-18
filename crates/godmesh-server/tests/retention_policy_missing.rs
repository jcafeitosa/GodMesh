//! Contrato da retenção no banco para o caso **negativo**: jurisdição sem
//! política efetiva bloqueia a persistência (ADR-062).
//!
//! É o único teste que altera o registro de política, e altera/restaura em volta
//! da chamada — por isso roda sozinho, com `--ignored`:
//!   DATABASE_URL=… cargo test -p godmesh-server --test retention_policy_missing -- --ignored --nocapture
//!
//! Exige as migrações 0001 e 0002 aplicadas.

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::api::{create_incident, ApiError, IncidentPayload};
use godmesh_server::auth::{Subject, VerifiedGrant};
use sqlx::PgPool;

const TENANT_CO: &str = "tenant-co-negative";

/// Corpo do pedido: só o identificador. A tenancy vem do grant verificado.
fn payload(incident_id: &str) -> IncidentPayload {
    IncidentPayload {
        incident_id: incident_id.to_owned(),
    }
}

/// Grant de teste do tenant CO: sujeito e predicado completos, para que a recusa
/// medida seja a de **retenção** e não a de autorização.
fn co_grant() -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: TENANT_CO.to_owned(),
            principal: "principal-42".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::OpenSosIncident,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
    }
}

fn fresh_id(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("inc-{label}-{}-{nanos}", std::process::id())
}

/// Remove a **cópia primária autorizada** de `incidente_sos` para CO: a classe
/// continua existindo, mas a política deixa de autorizar persistência, porque a
/// ADR-062 exige classe, finalidade, cópia, jurisdição, prazo e gatilho.
///
/// A linha de `retention_class` não é apagada de propósito: incidentes de CO já
/// gravados a referenciam e a FK `sos_incident_retention_class_fkey` os deixaria
/// órfãos — exatamente o que a migração 0002 passou a impedir.
async fn drop_co_authorized_copy(pool: &PgPool) {
    sqlx::query("DELETE FROM retention_copy WHERE class = 'incidente_sos' AND jurisdiction = 'CO'")
        .execute(pool)
        .await
        .expect("cópia CO removida");
}

/// Repõe exatamente a semente da migração 0002.
async fn restore_co_authorized_copy(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ('incidente_sos','CO','primary') ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("cópia CO reposta");
}

#[tokio::test]
#[ignore = "altera o registro de política de CO; roda isolado com --ignored"]
async fn colombian_tenant_without_policy_is_refused_with_retention_policy_missing() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL obrigatório com --ignored");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("conexão");

    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'CO') ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = 'CO'")
        .bind(TENANT_CO)
        .execute(&pool)
        .await
        .expect("jurisdição CO do tenant de teste");

    drop_co_authorized_copy(&pool).await;
    let refused = create_incident(&pool, &co_grant(), payload(&fresh_id("co-no-policy"))).await;
    restore_co_authorized_copy(&pool).await;

    assert!(
        matches!(refused, Err(ApiError::RetentionPolicyMissing)),
        "CO sem política precisa falhar fechado com retention_policy_missing: {refused:?}"
    );

    // Controle positivo depois da reposição: a política voltou a autorizar.
    let accepted = create_incident(&pool, &co_grant(), payload(&fresh_id("co-restored"))).await;
    assert!(accepted.is_ok(), "semente de CO precisa voltar a autorizar: {accepted:?}");
}