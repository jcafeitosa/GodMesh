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
        Ok(resp) => Ok((StatusCode::CREATED, Json(resp))),
        Err(e) => Err(ApiErrorResponse::from(e)),
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
                "classe incidente_sos sem política aplicável".to_owned(),
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
