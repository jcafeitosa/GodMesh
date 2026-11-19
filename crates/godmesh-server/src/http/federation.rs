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
//! Desde a emenda à ADR-071 (fatia M1b) `federation:admin` **não basta**: as
//! duas rotas de decisão exigem também um `ApprovalCertificate` no corpo,
//! assinado por um nó `authorized`. O escopo humano autoriza a operação; a
//! assinatura de nó é o que prova a autoridade de ingresso. As duas recusam
//! antes de qualquer transição de estado.
//!
//! Auditoria registra `join_request_id`/`node_id`, quem agiu (`token_id`) e o
//! `node_id` do aprovador. O corpo do manifesto nunca é logado, e a chave
//! privada do aprovador nunca chega aqui: quem assina é o nó aprovador, e o
//! servidor só verifica contra a chave pública registrada.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{ApiErrorResponse, AppState, created_status};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};
use crate::federation::{
    ApprovalDecision, JoinRequestView, approve_join_request, list_join_requests, list_peers,
    reject_join_request, submit_join_request,
};
use godmesh_federation::SignedManifest;
use godmesh_federation::identity::is_node_id;
use godmesh_federation::link::LinkSnapshot;
use godmesh_federation::peer::PeerRecord;

/// Todas as rotas de federação, no registro. O `Access` é o portão: a rota
/// pública fica fora do `require_grant` **por declaração explícita no
/// registro**, não por omissão de montagem.
pub(super) fn register(reg: &mut Registry) {
    reg.post(
        "/v1/federation/join-requests",
        Access::Public,
        RouteDoc {
            summary: "Pede ingresso na federação; a credencial é a assinatura do manifesto, não um token",
            request: Some("SignedManifest"),
            success: &[
                (201, Schema::Ref("JoinRequestView")),
                (200, Schema::Ref("JoinRequestView")),
            ],
            errors: &[
                "invalid_manifest",
                "invalid_manifest_signature",
                "manifest_expired",
                "manifest_not_yet_valid",
                "storage_unavailable",
            ],
            params: &[],
        },
        post_join_request,
    );

    let read = Access::Scope(Scope::FederationRead);
    reg.get(
        "/v1/federation/join-requests",
        read,
        RouteDoc {
            summary: "Lista pedidos de ingresso; ?state=pending|approved|rejected|expired filtra",
            request: None,
            success: &[(200, Schema::Array("JoinRequestView"))],
            errors: &["invalid_request", "storage_unavailable"],
            params: &[ParamSpec {
                name: "state",
                in_path: false,
                required: false,
                ty: "string",
                desc: "filtro opcional pelo estado do pedido",
            }],
        },
        get_join_requests,
    );
    reg.get(
        "/v1/federation/peers",
        read,
        RouteDoc {
            summary: "Lista pares registrados; só dados públicos (chave pública em hex)",
            request: None,
            success: &[(200, Schema::Array("PeerRecord"))],
            errors: &["storage_unavailable"],
            params: &[],
        },
        get_peers,
    );
    reg.get(
        "/v1/federation/peers/{node_id}/link",
        read,
        RouteDoc {
            summary: "Estado do enlace com um par; unknown é estado de primeira classe (ADR-072)",
            request: None,
            success: &[(200, Schema::Ref("LinkSnapshot"))],
            errors: &["invalid_request"],
            params: &[ParamSpec {
                name: "node_id",
                in_path: true,
                required: true,
                ty: "string",
                desc: "gmn_ + 64 hex minúsculos (sha256 da chave pública Ed25519); fora da forma é 422",
            }],
        },
        get_peer_link,
    );

    let admin = Access::Scope(Scope::FederationAdmin);
    const JR_PARAM: ParamSpec = ParamSpec {
        name: "join_request_id",
        in_path: true,
        required: true,
        ty: "string",
        desc: "identificador público do pedido (fjr_ + 32 hex)",
    };
    reg.post_destructive(
        "/v1/federation/join-requests/{join_request_id}/approve",
        admin,
        RouteDoc {
            summary: "Aprova ingresso ou confirma decisão existente; exige prova no domínio godmesh.federation.approval.v1",
            request: Some("ApprovalDecision"),
            success: &[(200, Schema::Ref("JoinRequestView"))],
            errors: &[
                "invalid_request",
                "approval_signature_invalid",
                "approver_not_authorized",
                "approval_expired",
                "approval_not_yet_valid",
                "approval_epoch_regressed",
                "no_authorized_approver",
                "federation_governance_disabled",
                "join_request_not_found",
                "join_request_state_conflict",
                "illegal_trust_transition",
                "federation_invariant_violation",
                "storage_unavailable",
            ],
            params: &[JR_PARAM],
        },
        post_join_request_approve,
    );
    reg.post_destructive(
        "/v1/federation/join-requests/{join_request_id}/reject",
        admin,
        RouteDoc {
            summary: "Rejeita ingresso; exige prova no domínio godmesh.federation.rejection.v1",
            request: Some("ApprovalDecision"),
            success: &[(200, Schema::Ref("JoinRequestView"))],
            errors: &[
                "invalid_request",
                "approval_signature_invalid",
                "approver_not_authorized",
                "approval_expired",
                "approval_not_yet_valid",
                "approval_epoch_regressed",
                "no_authorized_approver",
                "federation_governance_disabled",
                "join_request_not_found",
                "join_request_state_conflict",
                "federation_invariant_violation",
                "storage_unavailable",
            ],
            params: &[JR_PARAM],
        },
        post_join_request_reject,
    );
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
///
/// O corpo é a segunda autoridade: sem `approval`, a operação morre no
/// extrator com `invalid_request` e nenhum estado muda.
async fn require_legacy_cluster(state: &AppState, id: &str) -> Result<(), ApiErrorResponse> {
    if let Some(portable) = &state.portable {
        let belongs: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM federation_join_request
              WHERE join_request_id = $1 AND manifest->>'cluster' = $2)",
        ).bind(id).bind(portable.cluster()).fetch_one(&state.pool).await.map_err(ApiError::from)?;
        if belongs { return Err(ApiError::FederationGovernanceDisabled.into()); }
    }
    Ok(())
}

async fn post_join_request_approve(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(join_request_id): Path<String>,
    payload: Result<Json<ApprovalDecision>, JsonRejection>,
) -> Result<Json<JoinRequestView>, ApiErrorResponse> {
    require_legacy_cluster(&state, &join_request_id).await?;
    let Json(decision) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(format!(
            "aprovar exige o certificado de aprovação assinado por nó autorizado no corpo \
             (campo approval): {}",
            rejection.body_text()
        )))
    })?;
    let approver = if grant.subject.principal.trim().is_empty() {
        grant.token_id.clone()
    } else {
        grant.subject.principal.clone()
    };
    let result = approve_join_request(
        &state.pool,
        &state.decisions,
        &join_request_id,
        &approver,
        &decision.approval,
        || state.authorization.now_unix(),
    )
    .await;
    match result {
        Ok((request, peer)) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_approved join_request_id={} node_id={} epoch={} peer_state={} by={} approved_by_node={} approval_epoch={}",
                request.join_request_id,
                request.node_id,
                request.epoch,
                peer.trust_state.as_str(),
                grant.token_id,
                decision.approval.node_id,
                decision.approval.epoch
            ));
            Ok(Json(request))
        }
        Err(e) => {
            // Nenhum estado mudou; a auditoria nomeia o aprovador **alegado**,
            // porque é o dado que um operador precisa para distinguir chave
            // errada de nó não autorizado. O `node_id` vem de um certificado
            // que não se verificou, logo é alegação — e o log diz isso.
            state.authorization.audit_event(&format!(
                "event=federation_join_approve_denied by={} claimed_approver={} reason={}",
                grant.token_id,
                claimed_approver(&decision.approval.node_id),
                approval_code(&e)
            ));
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Rejeita um pedido pendente. Nenhum par é criado.
///
/// A assinatura usa o domínio exclusivo de rejeição. A autoria da rejeição
/// fica na auditoria.
async fn post_join_request_reject(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(join_request_id): Path<String>,
    payload: Result<Json<ApprovalDecision>, JsonRejection>,
) -> Result<Json<JoinRequestView>, ApiErrorResponse> {
    require_legacy_cluster(&state, &join_request_id).await?;
    let Json(decision) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(format!(
            "rejeitar exige o certificado de rejeição assinado por nó autorizado no corpo \
             (campo approval): {}",
            rejection.body_text()
        )))
    })?;
    let result = reject_join_request(
        &state.pool,
        &state.decisions,
        &join_request_id,
        &decision.approval,
        || state.authorization.now_unix(),
    )
    .await;
    match result {
        Ok(request) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_rejected join_request_id={} node_id={} epoch={} by={} rejected_by_node={}",
                request.join_request_id,
                request.node_id,
                request.epoch,
                grant.token_id,
                decision.approval.node_id
            ));
            Ok(Json(request))
        }
        Err(e) => {
            state.authorization.audit_event(&format!(
                "event=federation_join_reject_denied by={} claimed_approver={} reason={}",
                grant.token_id,
                claimed_approver(&decision.approval.node_id),
                approval_code(&e)
            ));
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Só o código estável do erro: a linha de auditoria não pode carregar texto
/// controlado por quem chamou.
fn approval_code(e: &ApiError) -> &'static str {
    super::contract::error_parts(e).1
}

/// `node_id` **alegado** por um certificado que ainda não se verificou.
///
/// Auditoria é linha única (`event=… campo=valor`): um identificador com
/// quebra de linha forjaria eventos que não aconteceram. O valor só é
/// transcrito se tiver a forma canônica; fora dela, vira um rótulo fixo — o
/// motivo exato já está no `reason=`, e ele é um código do catálogo.
fn claimed_approver(node_id: &str) -> &str {
    if is_node_id(node_id) {
        node_id
    } else {
        "<fora-do-formato>"
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
