//! Rotas de incidente: escrita (criação e transições) e leitura (item e lista).

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::{created_status, require_scope, AppState, ApiErrorResponse, ScopeGate};
use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};
use crate::auth::{Scope, VerifiedGrant};
use crate::incidents::{list_incidents, IncidentListQuery, IncidentPage};
use crate::lifecycle::{load_incident, transition, AcknowledgePayload, IncidentView, Transition};

pub(super) fn routes() -> Router<AppState> {
    let write = Router::new()
        .route("/v1/incidents", post(post_incident))
        .route("/v1/incidents/{id}/replicate", post(post_replicate))
        .route("/v1/incidents/{id}/ack-technical", post(post_ack_technical))
        .route("/v1/incidents/{id}/acknowledge", post(post_acknowledge))
        .route("/v1/incidents/{id}/close", post(post_close))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::IncidentWrite),
            require_scope,
        ));

    let read = Router::new()
        .route("/v1/incidents", get(get_incidents))
        .route("/v1/incidents/{id}", get(get_incident))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::IncidentRead),
            require_scope,
        ));

    write.merge(read)
}

pub(super) async fn post_incident(
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

/// Lista os incidentes do tenant autenticado, paginado por cursor.
///
/// `limit` ausente, `0` ou acima de 100 → 422 `invalid_request`; `state`
/// desconhecido → 422; cursor de outro tenant/filtro → 400 `cursor_incomparable`;
/// cursor ilegível → 400 `cursor_invalid`.
pub(super) async fn get_incidents(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    query: Result<Query<IncidentListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<IncidentPage>, ApiErrorResponse> {
    let Query(query) = query.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    list_incidents(&state.pool, &grant, query)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}


/// Leitura do incidente do tenant autenticado.
///
/// Incidente de outro tenant e incidente inexistente devolvem o **mesmo** 404:
/// distinguir os dois confirmaria a existência do identificador alheio.
pub(super) async fn get_incident(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    load_incident(&state.pool, &grant, &id)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

pub(super) async fn post_replicate(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Replicate, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

pub(super) async fn post_ack_technical(
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
pub(super) async fn post_acknowledge(
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

pub(super) async fn post_close(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Close, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}
