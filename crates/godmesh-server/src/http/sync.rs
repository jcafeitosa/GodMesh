//! Rotas de sincronização: Inbox de posições e checkpoint do prefixo contíguo.
//!
//! Escopos separados porque ler o prefixo não é aceitar posição.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Query, State};
use axum::Json;

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{AppState, ApiErrorResponse};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::inbox::{
    read_state, receive_batch, InboxBatchPayload, InboxBatchResponse, InboxState, InboxStateQuery,
};

pub(super) fn register(reg: &mut Registry) {
    reg.post(
        "/v1/sync/inbox",
        Access::Scope(Scope::SyncWrite),
        RouteDoc {
            summary: "Recebe um lote de até 100 posições e avança o prefixo contíguo do namespace",
            request: Some("InboxBatchPayload"),
            success: &[(200, Schema::Ref("InboxBatchResponse"))],
            errors: &[
                "invalid_request",
                "grant_denied",
                "retention_scope_unresolved",
                "retention_policy_missing",
                "sequence_gap",
                "event_namespace_reserved",
                "storage_unavailable",
            ],
            params: &[],
        },
        post_sync_inbox,
    );
    reg.get(
        "/v1/sync/state",
        Access::Scope(Scope::SyncRead),
        RouteDoc {
            summary: "Estado do Inbox: prefixo contíguo, último avanço e lacunas do namespace",
            request: None,
            success: &[(200, Schema::Ref("InboxState"))],
            errors: &[
                "invalid_request",
                "grant_denied",
                "storage_unavailable",
            ],
            params: &[
                ParamSpec { name: "stream_id", in_path: false, required: true, ty: "string", desc: "identificador da stream" },
                ParamSpec { name: "partition", in_path: false, required: true, ty: "integer", desc: "partição da posição canônica" },
                ParamSpec { name: "authority_epoch", in_path: false, required: true, ty: "integer", desc: "epoch da autoridade" },
                ParamSpec { name: "origin_node", in_path: false, required: false, ty: "string", desc: "sem ele, agregado conservador entre origens" },
            ],
        },
        get_sync_state,
    );
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
