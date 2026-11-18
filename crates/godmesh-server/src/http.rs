//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.
//!
//! Autenticação é aplicada por `route_layer` em `/v1/*`: rota nova sob esse
//! router nasce protegida, em vez de depender de alguém lembrar do middleware.
//! O handler só recebe o grant pela extensão preenchida pelo middleware, logo
//! não existe caminho de código que chame o caso de uso sem credencial.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use sqlx::PgPool;

use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};
use crate::auth::{bearer_token, GrantRegistry, VerifiedGrant};

/// Estado compartilhado.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub grants: GrantRegistry,
}

/// Rotas do servidor.
///
/// Toda rota sob `/v1` passa por `require_grant`. `/healthz` é público de
/// propósito: não expõe dado de tenant e não tem efeito.
pub fn router(pool: PgPool, grants: GrantRegistry) -> Router {
    let state = AppState { pool, grants };
    let protected = Router::new()
        .route("/v1/incidents", post(post_incident))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_grant));

    Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .merge(protected)
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok", "ack_class": "transport_accepted" }))
}

/// Portão único de autenticação das rotas `/v1`.
///
/// Sem header, com esquema errado, com token desconhecido → 401 e o handler
/// nunca roda (portanto nenhum INSERT). Token conhecido mas sem sujeito
/// utilizável → 422 antes do handler.
async fn require_grant(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let Some(token) = bearer_token(request.headers()) else {
        return ApiErrorResponse::from(ApiError::Unauthenticated).into_response();
    };
    let Some(grant) = state.grants.verify(token) else {
        return ApiErrorResponse::from(ApiError::Unauthenticated).into_response();
    };
    if let Err(e) = grant.validate() {
        return ApiErrorResponse::from(ApiError::InvalidCredential(e)).into_response();
    }
    request.extensions_mut().insert(grant.clone());
    next.run(request).await
}

async fn post_incident(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    payload: Result<Json<IncidentPayload>, JsonRejection>,
) -> Result<(StatusCode, Json<IncidentResponse>), ApiErrorResponse> {
    // Corpo inválido é recusado na fronteira, antes de qualquer efeito.
    let Json(payload) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    // O grant vem da extensão preenchida por `require_grant`; 201 na criação e
    // 200 no replay idempotente do mesmo tenant (ADR-058).
    match create_incident(&state.pool, &grant, payload).await {
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

impl ApiErrorResponse {
    fn status_and_code(&self) -> (StatusCode, &'static str, String) {
        match &self.0 {
            // 401 com `WWW-Authenticate` é o formato esperado por cliente HTTP.
            ApiError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "credencial ausente ou desconhecida".to_owned(),
            ),
            ApiError::InvalidCredential(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_credential",
                e.to_string(),
            ),
            ApiError::InvalidRequest(detail) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                detail.clone(),
            ),
            ApiError::GrantDenied(d) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "grant_denied",
                format!("{d:?}"),
            ),
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
            ApiError::Storage(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "falha de persistência".to_owned(),
            ),
        }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let (status, code, detail) = self.status_and_code();
        let mut response = (
            status,
            Json(json!({ "error": code, "detail": detail, "ack_class": "transport_accepted" })),
        )
            .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
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
