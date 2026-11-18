//! Rotas de evento: aceitação de posição e leitura da stream do tenant.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::{created_status, require_scope, AppState, ApiErrorResponse, ScopeGate};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::events::{create_event, list_events, EventAck, EventPage, EventPayload, EventQuery};

pub(super) fn routes() -> Router<AppState> {
    let write = Router::new()
        .route("/v1/events", post(post_event))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::EventWrite),
            require_scope,
        ));

    let read = Router::new()
        .route("/v1/events", get(get_events))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::EventRead),
            require_scope,
        ));

    write.merge(read)
}

pub(super) async fn post_event(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    payload: Result<Json<EventPayload>, JsonRejection>,
) -> Result<(StatusCode, Json<EventAck>), ApiErrorResponse> {
    let Json(payload) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    // 201 na posição nova, 200 na repetição idempotente da mesma posição. O ACK
    // nunca passa de `persisted_local` (ADR-058).
    match create_event(&state.pool, &grant, payload).await {
        Ok(outcome) => Ok((created_status(outcome.created), Json(outcome.response))),
        Err(e) => Err(ApiErrorResponse::from(e)),
    }
}

pub(super) async fn get_events(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    query: Result<Query<EventQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<EventPage>, ApiErrorResponse> {
    let Query(query) = query.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    list_events(&state.pool, &grant, query)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

