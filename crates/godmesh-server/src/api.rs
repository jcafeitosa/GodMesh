//! Caso de uso do incidente, independente do transporte.
//!
//! Separado do HTTP de propósito: o contrato é testável sem subir servidor.

use godmesh_emergency::incident::{Incident, IncidentError, IncidentState};
use godmesh_identity::grant::{
    FreshnessSource, GrantDecision, GrantPredicate, OfflineOp, RevocationState,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Corpo do pedido de incidente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentPayload {
    pub incident_id: String,
    pub tenant_id: String,
    pub principal: String,
    pub operation: OfflineOp,
    pub freshness: FreshnessSource,
    pub revocation: RevocationState,
    pub epoch_comparable: bool,
    pub action_enumerated: bool,
    #[serde(default)]
    pub retention_class_known: bool,
}

/// Resposta do endpoint. `ack_class` é a única alegação permitida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentResponse {
    pub incident_id: String,
    pub state: String,
    pub ack_class: String,
    pub ui_label: String,
}

/// Erros de API.
#[derive(Debug)]
pub enum ApiError {
    GrantDenied(GrantDecision),
    RetentionPolicyMissing,
    /// O contrato do incidente recusou a transição; nunca deveria sair do servidor.
    Invariant(IncidentError),
    Storage(sqlx::Error),
}

/// Nome persistido do estado: espelha o CHECK de `sos_incident.state` em
/// `migrations/0001_core.sql`. É adaptador de armazenamento, não regra de domínio.
const fn state_name(state: IncidentState) -> &'static str {
    match state {
        IncidentState::TriggeredLocally => "triggered_locally",
        IncidentState::PersistedLocal => "persisted_local",
        IncidentState::ReplicatedToAuthority => "replicated_to_authority",
        IncidentState::AcknowledgedTechnical => "acknowledged_technical",
        IncidentState::AcknowledgedHuman => "acknowledged_human",
        IncidentState::OutcomeUnknown => "outcome_unknown",
        IncidentState::NotPersisted => "not_persisted",
        IncidentState::Closed => "closed",
    }
}

/// Cria o incidente: valida grant, exige política de retenção e grava a trilha.
///
/// # Errors
/// `GrantDenied` quando o predicado nega; `RetentionPolicyMissing` sem política da
/// classe; `Invariant` quando o contrato do incidente recusa a transição; `Storage`
/// para falha de banco.
pub async fn create_incident(pool: &PgPool, payload: IncidentPayload) -> Result<IncidentResponse, ApiError> {
    let predicate = GrantPredicate {
        operation: payload.operation,
        epoch_comparable: payload.epoch_comparable,
        revocation: payload.revocation,
        freshness: payload.freshness,
        retention_class_known: payload.retention_class_known,
        action_enumerated: payload.action_enumerated,
    };
    if let denied @ (GrantDecision::DenyNeverOffline
    | GrantDecision::DenyNotEnumerated
    | GrantDecision::DenyDeniedNoTrustedTime
    | GrantDecision::DenyEpochIncomparable
    | GrantDecision::DenyRevocationUnknown
    | GrantDecision::DenyRetentionPolicyMissing) = predicate.evaluate()
    {
        return Err(ApiError::GrantDenied(denied));
    }

    let policy_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM retention_class WHERE class = 'incidente_sos' AND jurisdiction = 'BR')",
    )
    .fetch_one(pool)
    .await
    .map_err(ApiError::Storage)?;
    if !policy_exists {
        return Err(ApiError::RetentionPolicyMissing);
    }

    // A classe de ACK vem do domínio, não de string local: `persist` é a transição
    // que autoriza alegar `persisted_local` (ADR-058).
    // O instante do ACK é real (relógio do host) e nunca zero: `sos_ack.at_unix`
    // é `NOT NULL` e um ACK sem instante não é evidência.
    let at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::Invariant(IncidentError::MissingTimestamp))?
        .as_secs();
    let mut incident = Incident::triggered(&payload.incident_id, &payload.tenant_id, &payload.principal);
    incident
        .persist(
            &format!("inbox:{}", payload.tenant_id),
            incident.retention_class(),
            at_unix,
        )
        .map_err(ApiError::Invariant)?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;
    sqlx::query(
        "INSERT INTO sos_incident (id, tenant_id, principal, state) VALUES ($1, $2, $3, $4)",
    )
    .bind(&payload.incident_id)
    .bind(&payload.tenant_id)
    .bind(&payload.principal)
    .bind(state_name(incident.state()))
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    for ack in incident.acks() {
        sqlx::query(
            "INSERT INTO sos_ack (incident_id, ack_class, source, at_unix) VALUES ($1, $2, $3, $4)",
        )
        .bind(&payload.incident_id)
        .bind(ack.class)
        .bind(&ack.source)
        .bind(i64::try_from(ack.at_unix).unwrap_or(i64::MAX))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Storage)?;
    }
    tx.commit().await.map_err(ApiError::Storage)?;

    Ok(IncidentResponse {
        incident_id: payload.incident_id,
        state: state_name(incident.state()).to_owned(),
        ack_class: incident.ack_class().to_owned(),
        ui_label: incident.ui_label().to_owned(),
    })
}
