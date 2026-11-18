//! Rotas de administração de grants: emitir, consultar e revogar credenciais.
//!
//! O escopo é o da rota (`admin:grants`), não um "admin" genérico.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::{require_scope, AppState, ApiErrorResponse, ScopeGate};
use crate::admin::{
    grant_metadata, issue_grant, revoke_grant, GrantMetadata, IssueGrantPayload,
    IssuedGrantResponse, RevokeGrantPayload, RevocationResponse,
};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/admin/grants", post(post_admin_grant))
        .route("/v1/admin/grants/{token_id}", get(get_admin_grant))
        .route(
            "/v1/admin/grants/{token_id}/revoke",
            post(post_admin_grant_revoke),
        )
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::AdminGrants),
            require_scope,
        ))
}

/// Emite um grant. O token volta **uma vez** e nunca é logado.
///
/// O corpo traz `tenant_id` e `principal` porque esta é a rota de administração:
/// aqui os dois são o objeto da emissão, não proveniência de chamada. Quem
/// chama já foi autenticado com `admin:grants` pelo portão.
pub(super) async fn post_admin_grant(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    payload: Result<Json<IssueGrantPayload>, JsonRejection>,
) -> Result<(StatusCode, Json<IssuedGrantResponse>), ApiErrorResponse> {
    let Json(payload) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    let now = state.authorization.now_unix();
    match issue_grant(&state.pool, &payload, now).await {
        Ok(issued) => {
            // Auditoria identifica o grant e o sujeito; o token não entra aqui.
            state.authorization.audit_event(&format!(
                "event=grant_issued token_id={} tenant_id={} principal={} scopes={} by={}",
                issued.token_id,
                issued.tenant_id,
                issued.principal,
                issued
                    .scopes
                    .iter()
                    .map(|scope| scope.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                grant.token_id
            ));
            Ok((StatusCode::CREATED, Json(issued)))
        }
        Err(e) => {
            state
                .authorization
                .audit_event(&format!("event=grant_issue_denied by={}", grant.token_id));
            Err(ApiErrorResponse::from(e))
        }
    }
}

/// Metadados de um grant emitido: sem token e sem hash.
pub(super) async fn get_admin_grant(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<Json<GrantMetadata>, ApiErrorResponse> {
    grant_metadata(&state.pool, &token_id)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

/// Revoga um grant emitido; o efeito vale no mesmo processo, sem reinício.
///
/// Depois do commit, o cache de revogação daquele `token_id` é invalidado: a
/// próxima requisição relê o banco em vez de esperar o TTL.
pub(super) async fn post_admin_grant_revoke(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(token_id): Path<String>,
    payload: Result<Json<RevokeGrantPayload>, JsonRejection>,
) -> Result<Json<RevocationResponse>, ApiErrorResponse> {
    let Json(payload) = payload.map_err(|rejection| {
        ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
    })?;
    match revoke_grant(&state.pool, &token_id, &payload).await {
        Ok(revoked) => {
            // Invalidação explícita: revogar não espera o TTL de 5 s.
            state.authorization.invalidate_revocation(&token_id).await;
            state.authorization.audit_event(&format!(
                "event=grant_revoked token_id={} by={}",
                revoked.token_id, grant.token_id
            ));
            Ok(Json(revoked))
        }
        Err(e) => {
            // O `token_id` recusado vem do caminho e não foi validado contra o
            // banco: fica fora do log para não permitir forjar linha de auditoria.
            state.authorization.audit_event(&format!(
                "event=grant_revoke_denied by={}",
                grant.token_id
            ));
            Err(ApiErrorResponse::from(e))
        }
    }
}

