//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.
//!
//! Autenticação é aplicada por `route_layer` em `/v1/*`: rota nova sob esse
//! router nasce protegida, em vez de depender de alguém lembrar do middleware.
//! O handler só recebe o grant pela extensão preenchida pelo middleware, logo
//! não existe caminho de código que chame o caso de uso sem credencial.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};
use crate::auth::{bearer_token, GrantRegistry, VerifiedGrant};
use crate::lifecycle::{
    load_incident, transition, AcknowledgePayload, IncidentView, Transition,
};

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
        .route("/v1/incidents/{id}", get(get_incident))
        .route("/v1/incidents/{id}/replicate", post(post_replicate))
        .route("/v1/incidents/{id}/ack-technical", post(post_ack_technical))
        .route("/v1/incidents/{id}/acknowledge", post(post_acknowledge))
        .route("/v1/incidents/{id}/close", post(post_close))
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

/// Leitura do incidente do tenant autenticado.
///
/// Incidente de outro tenant e incidente inexistente devolvem o **mesmo** 404:
/// distinguir os dois confirmaria a existência do identificador alheio.
async fn get_incident(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    load_incident(&state.pool, &grant, &id)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

async fn post_replicate(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Replicate, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

async fn post_ack_technical(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::AcknowledgeTechnical, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

/// Reconhecimento humano exige corpo com `responder`.
///
/// Corpo ausente, vazio ou ilegível é tratado como **ausência de prova**
/// (`422 missing_human_proof`), não como erro genérico: sem pessoa identificada a
/// transição não existe, e o código de erro diz exatamente isso.
async fn post_acknowledge(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
    payload: Result<Json<AcknowledgePayload>, JsonRejection>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    let responder = match payload {
        Ok(Json(payload)) => payload.responder,
        Err(_) => None,
    };
    transition(
        &state.pool,
        &grant,
        &id,
        Transition::AcknowledgeHuman,
        responder.as_deref(),
    )
    .await
    .map(Json)
    .map_err(ApiErrorResponse::from)
}

async fn post_close(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Close, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
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
            // 404, nunca 403: um 403 confirmaria que o identificador existe em
            // outro tenant, e existência de incidente alheio não é dado público.
            ApiError::IncidentNotFound => (
                StatusCode::NOT_FOUND,
                "incident_not_found",
                "incidente não encontrado".to_owned(),
            ),
            ApiError::IllegalTransition { current } => (
                StatusCode::CONFLICT,
                "illegal_transition",
                format!("transição ilegal a partir de {current}"),
            ),
            ApiError::MissingHumanProof => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "missing_human_proof",
                "reconhecimento humano exige responder (prova de pessoa)".to_owned(),
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

impl ApiErrorResponse {
    /// Campo extra do corpo de erro, quando o cliente precisa dele para agir.
    /// Hoje só `illegal_transition` devolve o estado atual.
    fn extra(&self) -> Option<(&'static str, Value)> {
        match &self.0 {
            ApiError::IllegalTransition { current } => {
                Some(("state", Value::String(current.clone())))
            }
            _ => None,
        }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let (status, code, detail) = self.status_and_code();
        let mut body = Map::new();
        body.insert("error".to_owned(), Value::String(code.to_owned()));
        body.insert("detail".to_owned(), Value::String(detail));
        // `ack_class` do erro é o piso honesto: nada foi confirmado.
        body.insert(
            "ack_class".to_owned(),
            Value::String("transport_accepted".to_owned()),
        );
        if let Some((key, value)) = self.extra() {
            body.insert(key.to_owned(), value);
        }
        let mut response = (status, Json(Value::Object(body))).into_response();
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
