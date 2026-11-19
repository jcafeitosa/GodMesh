//! Rotas de evento: aceitação de posição e leitura da stream do tenant.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::Json;

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{created_status, AppState, ApiErrorResponse};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::config_bundle::BundleService;
use crate::events::{create_event_with_fence, list_events, EventAck, EventPage, EventQuery};
use godmesh_federation::SignedEvent;
use godmesh_federation::now_unix;

pub(super) fn register(reg: &mut Registry) {
    reg.post(
        "/v1/events",
        Access::Scope(Scope::EventWrite),
        RouteDoc {
            summary: "Aceita um evento assinado pelo nó de origem; replay da mesma posição devolve 200 sem reescrever (S-sec-2: sem assinatura válida de nó autorizado, nenhum INSERT)",
            request: Some("SignedEvent"),
            success: &[
                (201, Schema::Ref("EventAck")),
                (200, Schema::Ref("EventAck")),
            ],
            errors: &[
                "invalid_request",
                "grant_denied",
                "event_signature_invalid",
                "event_node_unauthorized",
                "config_fence_blocked",
                "event_scope_denied",
                "event_epoch_stale",
                "event_not_fresh",
                "retention_scope_unresolved",
                "retention_policy_missing",
                "sequence_gap",
                "storage_unavailable",
            ],
            params: &[],
        },
        post_event,
    );
    reg.get(
        "/v1/events",
        Access::Scope(Scope::EventRead),
        RouteDoc {
            summary: "Lê a stream do tenant paginada por cursor opaco",
            request: None,
            success: &[(200, Schema::Ref("EventPage"))],
            errors: &[
                "invalid_request",
                "cursor_invalid",
                "cursor_incomparable",
                "grant_denied",
                "storage_unavailable",
            ],
            params: &[
                ParamSpec { name: "stream_id", in_path: false, required: true, ty: "string", desc: "identificador da stream" },
                ParamSpec { name: "partition", in_path: false, required: true, ty: "integer", desc: "partição da posição canônica (ADR-057)" },
                ParamSpec { name: "authority_epoch", in_path: false, required: true, ty: "integer", desc: "epoch da autoridade; cursor de outro epoch não é comparável" },
                ParamSpec { name: "limit", in_path: false, required: true, ty: "integer", desc: "1..=100, obrigatório" },
                ParamSpec { name: "origin_node", in_path: false, required: false, ty: "string", desc: "filtro opcional por nó de origem" },
                ParamSpec { name: "cursor", in_path: false, required: false, ty: "string", desc: "next_cursor da página anterior" },
            ],
        },
        get_events,
    );
}

pub(super) async fn post_event(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    config: Option<Extension<Arc<BundleService>>>,
    payload: Result<Json<SignedEvent>, JsonRejection>,
) -> Result<(StatusCode, Json<EventAck>), ApiErrorResponse> {
    let Json(event) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    // 201 na posição nova, 200 na repetição idempotente da mesma posição. O ACK
    // nunca passa de `persisted_local` (ADR-058). A janela de frescor usa o
    // relógio do processo; o caso de uso aceita `now` injetável para teste.
    match create_event_with_fence(
        &state.pool,
        &grant,
        event,
        &state.event_window,
        now_unix(),
        state.portable.as_deref(),
        config.as_ref().map(|Extension(service)| service.as_ref()),
    )
    .await
    {
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

