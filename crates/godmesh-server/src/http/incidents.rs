//! Rotas de incidente: escrita (criação e transições) e leitura (item e lista).
//!
//! Registradas no `Registry`: a assinatura exige escopo, semântica e
//! documentação — não existe rota registrada sem contrato (ADR-074).

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use super::contract::{Access, ParamSpec, Registry, RouteDoc, Schema};
use super::{created_status, AppState, ApiErrorResponse};
use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};
use crate::auth::{Scope, VerifiedGrant};
use crate::incidents::{list_incidents, IncidentListQuery, IncidentPage};
use crate::lifecycle::{load_incident, transition, AcknowledgePayload, IncidentView, Transition};

pub(super) fn register(reg: &mut Registry) {
    let write = Access::Scope(Scope::IncidentWrite);
    let read = Access::Scope(Scope::IncidentRead);
    const ID_PARAM: ParamSpec = ParamSpec {
        name: "id",
        in_path: true,
        required: true,
        ty: "string",
        desc: "identificador do incidente do tenant autenticado",
    };

    reg.post(
        "/v1/incidents",
        write,
        RouteDoc {
            summary: "Cria o incidente do tenant; repetição idempotente devolve 200 sem reescrever a trilha",
            request: Some("IncidentPayload"),
            success: &[
                (201, Schema::Ref("IncidentResponse")),
                (200, Schema::Ref("IncidentResponse")),
            ],
            errors: &[
                "invalid_request",
                "grant_denied",
                "retention_scope_unresolved",
                "retention_policy_missing",
                "incident_id_conflict",
                "invariant_violation",
                "storage_unavailable",
            ],
            params: &[],
        },
        post_incident,
    );
    reg.get(
        "/v1/incidents",
        read,
        RouteDoc {
            summary: "Lista incidentes do tenant, paginada por cursor opaco",
            request: None,
            success: &[(200, Schema::Ref("IncidentPage"))],
            errors: &[
                "invalid_request",
                "cursor_invalid",
                "cursor_incomparable",
                "grant_denied",
                "storage_unavailable",
            ],
            params: &[
                ParamSpec {
                    name: "limit",
                    in_path: false,
                    required: true,
                    ty: "integer",
                    desc: "1..=100, obrigatório: paginação sem default publicado seria parâmetro inventado",
                },
                ParamSpec {
                    name: "state",
                    in_path: false,
                    required: false,
                    ty: "string",
                    desc: "filtro pelo conjunto fechado de estados (see IncidentView.state)",
                },
                ParamSpec {
                    name: "cursor",
                    in_path: false,
                    required: false,
                    ty: "string",
                    desc: "next_cursor da página anterior; de outra partição/filtro é recusado",
                },
            ],
        },
        get_incidents,
    );
    reg.get(
        "/v1/incidents/{id}",
        read,
        RouteDoc {
            summary: "Lê o incidente com a trilha de ACK; inexistente e alheio devolvem o mesmo 404",
            request: None,
            success: &[(200, Schema::Ref("IncidentView"))],
            errors: &[
                "incident_not_found",
                "grant_denied",
                "storage_unavailable",
            ],
            params: &[ID_PARAM],
        },
        get_incident,
    );

    const TRANSITIONS: &[(&str, &str, bool)] = &[
        ("/v1/incidents/{id}/replicate", "Replica o incidente para a autoridade (persistido → replicado)", false),
        ("/v1/incidents/{id}/ack-technical", "Reconhecimento técnico (replicado → acknowledged_technical)", false),
        ("/v1/incidents/{id}/acknowledge", "Reconhecimento humano; exige prova de pessoa no corpo", false),
        ("/v1/incidents/{id}/close", "Encerra o incidente; transição terminal, por isso aceita dry_run", true),
    ];
    // Um registro por transição: o handler específico vive na tabela, e a
    // semântica (destructive) também — ninguém esquece o dry_run do `close`.
    for &(path, summary, destructive) in TRANSITIONS {
        let doc = RouteDoc {
            summary,
            request: if path.ends_with("acknowledge") {
                Some("AcknowledgePayload")
            } else {
                None
            },
            success: &[(200, Schema::Ref("IncidentView"))],
            errors: &[
                "incident_not_found",
                "illegal_transition",
                "missing_human_proof",
                "grant_denied",
                "invariant_violation",
                "storage_unavailable",
            ],
            params: &[ID_PARAM],
        };
        match (path, destructive) {
            ("/v1/incidents/{id}/replicate", _) => reg.post(path, write, doc, post_replicate),
            ("/v1/incidents/{id}/ack-technical", _) => reg.post(path, write, doc, post_ack_technical),
            ("/v1/incidents/{id}/acknowledge", _) => reg.post(path, write, doc, post_acknowledge),
            ("/v1/incidents/{id}/close", true) => reg.post_destructive(path, write, doc, post_close),
            _ => unreachable!("transição fora da tabela"),
        }
    }
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
