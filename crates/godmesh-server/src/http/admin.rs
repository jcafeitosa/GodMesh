//! Rotas de administração de grants: emitir, consultar e revogar credenciais.
//!
//! O escopo é o da rota (`admin:grants`), não um "admin" genérico. Revogar é
//! destrutiva por definição (irreversível): o registro impõe `dry_run`.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::Json;

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{AppState, ApiErrorResponse};
use crate::admin::{
    grant_metadata, issue_grant, revoke_grant, GrantMetadata, IssueGrantPayload,
    IssuedGrantResponse, RevokeGrantPayload, RevocationResponse,
};
use crate::api::ApiError;
use crate::auth::{Scope, VerifiedGrant};

pub(super) fn register(reg: &mut Registry) {
    let access = Access::Scope(Scope::AdminGrants);
    const TOKEN_PARAM: ParamSpec = ParamSpec {
        name: "token_id",
        in_path: true,
        required: true,
        ty: "string",
        desc: "identificador público do grant emitido pela API",
    };

    reg.post(
        "/v1/admin/grants",
        access,
        RouteDoc {
            summary: "Emite um grant; o token em claro volta uma única vez e nunca é logado",
            request: Some("IssueGrantPayload"),
            success: &[(201, Schema::Ref("IssuedGrantResponse"))],
            errors: &[
                "invalid_request",
                "randomness_unavailable",
                "storage_unavailable",
            ],
            params: &[],
        },
        post_admin_grant,
    );
    reg.get(
        "/v1/admin/grants/{token_id}",
        access,
        RouteDoc {
            summary: "Metadados de um grant: sem token e sem hash, por construção",
            request: None,
            success: &[(200, Schema::Ref("GrantMetadata"))],
            errors: &["grant_not_found", "storage_unavailable"],
            params: &[TOKEN_PARAM],
        },
        get_admin_grant,
    );
    reg.post_destructive(
        "/v1/admin/grants/{token_id}/revoke",
        access,
        RouteDoc {
            summary: "Revoga um grant; vale no mesmo processo e é idempotente no instante",
            request: Some("RevokeGrantPayload"),
            success: &[(200, Schema::Ref("RevocationResponse"))],
            errors: &["invalid_request", "grant_not_found", "storage_unavailable"],
            params: &[TOKEN_PARAM],
        },
        post_admin_grant_revoke,
    );
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

