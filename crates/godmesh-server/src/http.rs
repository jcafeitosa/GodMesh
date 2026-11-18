//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use godmesh_identity::grant::GrantDecision;
use serde_json::json;
use sqlx::PgPool;

use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};

/// Estado compartilhado.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

/// Rotas do servidor.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .route("/v1/incidents", post(post_incident))
        .with_state(AppState { pool })
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok", "ack_class": "transport_accepted" }))
}

async fn post_incident(
    State(state): State<AppState>,
    Json(payload): Json<IncidentPayload>,
) -> Result<(StatusCode, Json<IncidentResponse>), ApiErrorResponse> {
    match create_incident(&state.pool, payload).await {
        Ok(outcome) => Ok((created_status(outcome.created), Json(outcome.response))),
        Err(e) => Err(ApiErrorResponse::from(e)),
    }
}

/// 201 na primeira gravação, 200 no replay idempotente do mesmo tenant.
const fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

/// Erro HTTP com corpo estável e sem promessa de efeito.
pub struct ApiErrorResponse(ApiError);

impl From<ApiError> for ApiErrorResponse {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let (status, code, detail) = match self.0 {
            ApiError::GrantDenied(d) => (StatusCode::UNPROCESSABLE_ENTITY, "grant_denied", format!("{d:?}")),
            ApiError::RetentionPolicyMissing => (
                StatusCode::CONFLICT,
                "retention_policy_missing",
                "classe incidente_sos sem política aplicável à jurisdição do tenant".to_owned(),
            ),
            ApiError::RetentionScopeUnresolved => (
                StatusCode::CONFLICT,
                "retention_scope_unresolved",
                "tenant sem jurisdição declarada".to_owned(),
            ),
            ApiError::IncidentIdConflict => (
                StatusCode::CONFLICT,
                "incident_id_conflict",
                "incident_id já pertence a outro tenant".to_owned(),
            ),
            ApiError::Invariant(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invariant_violation",
                format!("contrato do incidente recusou a transição: {e:?}"),
            ),
            ApiError::Storage(_) => (StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable", "falha de persistência".to_owned()),
        };
        let _ = GrantDecision::Allow;
        (status, Json(json!({ "error": code, "detail": detail, "ack_class": "transport_accepted" }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::created_status;
    use axum::http::StatusCode;

    #[test]
    fn replay_is_200_and_first_write_is_201() {
        assert_eq!(created_status(false), StatusCode::OK);
        assert_eq!(created_status(true), StatusCode::CREATED);
    }
}
