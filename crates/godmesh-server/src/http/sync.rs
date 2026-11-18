//! Rotas de sincronização: Inbox de posições e checkpoint do prefixo contíguo.
//!
//! Escopos separados porque ler o prefixo não é aceitar posição.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Query, State};
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::{require_scope, AppState, ApiErrorResponse, ScopeGate};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::inbox::{
    read_state, receive_batch, InboxBatchPayload, InboxBatchResponse, InboxState, InboxStateQuery,
};

pub(super) fn routes() -> Router<AppState> {
    let write = Router::new()
        .route("/v1/sync/inbox", post(post_sync_inbox))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::SyncWrite),
            require_scope,
        ));

    let read = Router::new()
        .route("/v1/sync/state", get(get_sync_state))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::SyncRead),
            require_scope,
        ));

    write.merge(read)
}

/// `MAX_INBOX_BATCH`) é recusado na fronteira, antes de qualquer efeito. O teto é
/// limite desta fatia, declarado e recusado — nunca truncado em silêncio.
///
/// 200 com `{accepted, duplicates, contiguous_prefix, ack_class}` quando nada
/// falta; 409 `sequence_gap` com `{expected, received, accepted, duplicates,
/// contiguous_prefix}` quando o lote salta posição — o trecho contíguo antes da
/// lacuna é persistido e o resto não é aceito.
pub(super) async fn post_sync_inbox(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    payload: Result<Json<InboxBatchPayload>, JsonRejection>,
) -> Result<Json<InboxBatchResponse>, ApiErrorResponse> {
    let Json(payload) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    receive_batch(&state.pool, &grant, payload)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

/// Estado do Inbox: prefixo contíguo, instante do último avanço e lacunas.
///
/// Stream desconhecida devolve prefixo 0, `last_recorded_at` nulo e `gaps` vazio:
/// ausência de evento não é erro, e distinguir "stream inexistente" de "stream
/// sem evento" exigiria uma lista de streams que este servidor não tem.
pub(super) async fn get_sync_state(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    query: Result<Query<InboxStateQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<InboxState>, ApiErrorResponse> {
    let Query(query) = query.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    read_state(&state.pool, &grant, query)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}
