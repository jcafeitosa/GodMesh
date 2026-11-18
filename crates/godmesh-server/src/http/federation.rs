//! Rotas de federação: ingresso (público), registro de pares e diagnóstico de
//! enlace entre nós.
//!
//! `POST /v1/federation/join-requests` é **público** de propósito: um nó que
//! ainda não é par não tem credencial do cluster, e o pedido em si não autoriza
//! nada — a autoridade é a assinatura do manifesto mais a decisão administrativa
//! posterior. Todas as outras rotas exigem grant com escopo:
//!
//! * `federation:read` — listar pedidos e pares, e ler o estado do enlace de um
//!   par (`GET /v1/federation/peers/{node_id}/link`);
//! * `federation:admin` — aprovar e rejeitar.
//!
//! Auditoria registra `join_request_id`/`node_id` e quem agiu (`token_id`). O
//! corpo do manifesto nunca é logado e a chave privada não existe no servidor
//! nesta fatia (a identidade de nó é do runtime, não do route handler).

use std::collections::HashMap;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::{ApiErrorResponse, AppState, ScopeGate, created_status, require_scope};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::federation::{
    JoinRequestView, approve_join_request, list_join_requests, list_peers, reject_join_request,
    submit_join_request,
};
use godmesh_federation::SignedManifest;
use godmesh_federation::identity::is_node_id;
use godmesh_federation::link::LinkSnapshot;
use godmesh_federation::peer::PeerRecord;

/// Rotas públicas da federação. Montadas **fora** do portão `require_grant`.
pub(super) fn public_routes() -> Router<AppState> {
    Router::new().route("/v1/federation/join-requests", post(post_join_request))
}

/// Rotas autenticadas da federação.
pub(super) fn routes() -> Router<AppState> {
    let read = Router::new()
        .route("/v1/federation/join-requests", get(get_join_requests))
        .route("/v1/federation/peers", get(get_peers))
        .route(
            "/v1/federation/peers/{node_id}/link",
            get(get_peer_link),
        )
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::FederationRead),
            require_scope,
        ));

    let admin = Router::new()
        .route(
            "/v1/federation/join-requests/{join_request_id}/approve",
            post(post_join_request_approve),
        )
        .route(
            "/v1/federation/join-requests/{join_request_id}/reject",
            post(post_join_request_reject),
        )
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::FederationAdmin),
            require_scope,
        ));

    read.merge(admin)
}

/// Pede ingresso na federação. Público: a assinatura é a credencial.
///
/// 201 quando o pedido nasce, 200 quando a mesma geração (`node_id`, `epoch`) já
/// existia. O estado resultante é sempre `pending` ou o que já estava decidido —
/// esta rota nunca cria par autorizado.
async fn post_join_request(
    State(state): State<AppState>,
    payload: Result<Json<SignedManifest>, JsonRejection>,
) -> Result<(StatusCode, Json<JoinRequestView>), ApiErrorResponse> {
    let Json(manifest) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidManifest(rejection.body_text()))
    })?;
    let now = state.authorization.now_unix();
    match submit_join_request(&state.pool, state.join_window, &manifest, now).await {
        Ok(outcome) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_requested join_request_id={} node_id={} epoch={} state={}",
                outcome.request.join_request_id,
                outcome.request.node_id,
                outcome.request.epoch,
                outcome.request.state
            ));
            Ok((created_status(outcome.created), Json(outcome.request)))
        }
        Err(e) => {
            // Manifesto não verificado não gera identificador no log: um
            // `node_id` não provado não é dado confiável.
            state
                .authorization
                .audit_event("event=federation_join_denied reason=manifest_rejected");
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Lista pedidos de ingresso; `?state=pending|approved|rejected|expired` filtra.
async fn get_join_requests(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<JoinRequestView>>, ApiErrorResponse> {
    list_join_requests(&state.pool, query.get("state").map(String::as_str))
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

/// Aprova um pedido pendente e registra o par como `authorized`.
async fn post_join_request_approve(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(join_request_id): Path<String>,
) -> Result<Json<JoinRequestView>, ApiErrorResponse> {
    let now = state.authorization.now_unix();
    let approver = if grant.subject.principal.trim().is_empty() {
        grant.token_id.clone()
    } else {
        grant.subject.principal.clone()
    };
    match approve_join_request(&state.pool, &join_request_id, &approver, now).await {
        Ok((request, peer)) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_approved join_request_id={} node_id={} epoch={} peer_state={} by={}",
                request.join_request_id,
                request.node_id,
                request.epoch,
                peer.trust_state.as_str(),
                grant.token_id
            ));
            Ok(Json(request))
        }
        Err(e) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_approve_denied by={}",
                grant.token_id
            ));
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Rejeita um pedido pendente. Nenhum par é criado.
async fn post_join_request_reject(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(join_request_id): Path<String>,
) -> Result<Json<JoinRequestView>, ApiErrorResponse> {
    match reject_join_request(&state.pool, &join_request_id).await {
        Ok(request) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_rejected join_request_id={} node_id={} epoch={} by={}",
                request.join_request_id, request.node_id, request.epoch, grant.token_id
            ));
            Ok(Json(request))
        }
        Err(e) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_reject_denied by={}",
                grant.token_id
            ));
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Lista os pares registrados. `public_key` é dado público; **não** há campo de
/// chave privada em `PeerRecord`, então não existe caminho para vazá-la.
async fn get_peers(
    State(state): State<AppState>,
) -> Result<Json<Vec<PeerRecord>>, ApiErrorResponse> {
    list_peers(&state.pool)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

/// Estado real do enlace com um par — ADR-072 §10 (`unknown` é estado de primeira
/// classe) e ADR-071 §(3) (heartbeat por par).
///
/// Sem registro do par — inclusive quando este processo não tem runtime de
/// federação — a resposta é `200` com `state: unknown` e os demais campos `null`.
/// É a diferença entre "não sei" e "está tudo bem", e a resposta escolhe
/// "não sei".
///
/// `node_id` fora da forma canônica (`gmn_` + 64 hex) é pedido inválido, não
/// valor a normalizar: dois nós com identificador "quase igual" seriam o mesmo
/// nó se aqui se aceitasse aproximação.
async fn get_peer_link(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<LinkSnapshot>, ApiErrorResponse> {
    if !is_node_id(&node_id) {
        return Err(ApiErrorResponse::from(ApiError::InvalidRequest(
            "node_id deve ser gmn_ seguido de 64 hex minúsculos (sha256 da chave pública Ed25519)"
                .to_owned(),
        )));
    }
    let snapshot = match &state.federation {
        Some(links) => links.snapshot_or_never_seen(&node_id),
        None => LinkSnapshot::never_seen(),
    };
    Ok(Json(snapshot))
}
