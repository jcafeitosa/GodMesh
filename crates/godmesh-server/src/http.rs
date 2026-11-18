//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.
//!
//! Autenticação é aplicada por `route_layer` em `/v1/*`: rota nova sob esse
//! router nasce protegida, em vez de depender de alguém lembrar do middleware.
//! O handler só recebe o grant pela extensão preenchida pelo middleware, logo
//! não existe caminho de código que chame o caso de uso sem credencial.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::api::{create_incident, ApiError, IncidentPayload, IncidentResponse};
use crate::auth::{bearer_token, Authorization, GrantRegistry, Scope, VerifiedGrant};
use crate::events::{create_event, list_events, EventAck, EventPage, EventPayload, EventQuery};
use crate::inbox::{
    read_state, receive_batch, InboxBatchPayload, InboxBatchResponse, InboxState, InboxStateQuery,
};
use crate::lifecycle::{load_incident, transition, AcknowledgePayload, IncidentView, Transition};

/// Estado compartilhado.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub authorization: Arc<Authorization>,
}

/// Rotas do servidor com a composição de produção (relógio do sistema,
/// revogação em Postgres).
///
/// Toda rota sob `/v1` passa por `require_grant` **e** pelo portão de escopo da
/// própria rota. `/healthz` é público de propósito: não expõe dado de tenant e
/// não tem efeito.
pub fn router(pool: PgPool, grants: GrantRegistry) -> Router {
    let authorization = Authorization::new(grants, pool.clone());
    router_with(pool, authorization)
}

/// Rotas do servidor com autorização injetada (relógio e fonte de revogação).
///
/// Cada grupo de rotas declara o escopo que exige na própria construção do
/// router: rota nova nasce com escopo explícito, em vez de herdá-lo por omissão.
pub fn router_with(pool: PgPool, authorization: Authorization) -> Router {
    let state = AppState {
        pool,
        authorization: Arc::new(authorization),
    };

    let incident_write = Router::new()
        .route("/v1/incidents", post(post_incident))
        .route("/v1/incidents/{id}/replicate", post(post_replicate))
        .route("/v1/incidents/{id}/ack-technical", post(post_ack_technical))
        .route("/v1/incidents/{id}/acknowledge", post(post_acknowledge))
        .route("/v1/incidents/{id}/close", post(post_close))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::IncidentWrite),
            require_scope,
        ));

    let incident_read = Router::new()
        .route("/v1/incidents/{id}", get(get_incident))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::IncidentRead),
            require_scope,
        ));

    let event_write = Router::new()
        .route("/v1/events", post(post_event))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::EventWrite),
            require_scope,
        ));

    let event_read = Router::new()
        .route("/v1/events", get(get_events))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::EventRead),
            require_scope,
        ));

    // Inbox de sync: lote de posições (escrita) e leitura do checkpoint do
    // prefixo contíguo (leitura). Escopos separados porque ler o prefixo não é
    // aceitar posição.
    let sync_write = Router::new()
        .route("/v1/sync/inbox", post(post_sync_inbox))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::SyncWrite),
            require_scope,
        ));

    let sync_read = Router::new()
        .route("/v1/sync/state", get(get_sync_state))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::SyncRead),
            require_scope,
        ));

    // O portão de autenticação é aplicado por último: ele roda antes dos portões
    // de escopo, que leem o `VerifiedGrant` já injetado na extensão.
    let protected = incident_write
        .merge(incident_read)
        .merge(event_write)
        .merge(event_read)
        .merge(sync_write)
        .merge(sync_read)
        .route_layer(middleware::from_fn_with_state(state.clone(), require_grant));

    Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .merge(protected)
        .with_state(state)
}

/// Escopo que uma rota exige, carregado pelo middleware de escopo.
#[derive(Clone, Copy, Debug)]
struct ScopeGate(Scope);

async fn healthz(State(state): State<AppState>) -> Response {
    // Dependência real: 200 só com o pool respondendo. Um healthz estático fica
    // `healthy` com o banco morto e esconde indisponibilidade.
    match tokio::time::timeout(
        std::time::Duration::from_millis(500),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool),
    )
    .await
    {
        Ok(Ok(_)) => Json(json!({ "status": "ok", "ack_class": "transport_accepted" })).into_response(),
        Ok(Err(_)) | Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unavailable",
                "dependency": "postgres",
                "ack_class": "transport_accepted"
            })),
        )
            .into_response(),
    }
}

/// Portão único de autenticação das rotas `/v1`.
///
/// Sem header, com esquema errado, com token desconhecido → 401 e o handler
/// nunca roda (portanto nenhum INSERT). Credencial expirada, ainda não válida
/// ou revogada → 401 com o motivo. Falha ao consultar revogação → 503 e o
/// handler também não roda (falha fechada).
async fn require_grant(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let Some(token) = bearer_token(request.headers()) else {
        return ApiErrorResponse::from(ApiError::Unauthenticated).into_response();
    };
    // O escopo exigido é declarado pela rota; este portão roda antes e por isso
    // autentica sem julgar escopo. `require_scope` aplica o escopo da rota.
    match state.authorization.authenticate(token).await {
        Ok(grant) => {
            request.extensions_mut().insert(grant);
            next.run(request).await
        }
        Err(e) => ApiErrorResponse::from(ApiError::from(e)).into_response(),
    }
}

/// Portão de escopo da rota: 403 `insufficient_scope` quando o grant verificado
/// não contém o escopo exigido.
async fn require_scope(
    State(gate): State<ScopeGate>,
    request: Request,
    next: Next,
) -> Response {
    let Some(grant) = request.extensions().get::<VerifiedGrant>() else {
        // Defesa em profundidade: sem grant verificado nenhuma rota de escopo roda.
        return ApiErrorResponse::from(ApiError::Unauthenticated).into_response();
    };
    if !grant.has_scope(gate.0) {
        return ApiErrorResponse::from(ApiError::InsufficientScope).into_response();
    }
    next.run(request).await
}

async fn post_incident(
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

/// Leitura do incidente do tenant autenticado.
///
/// Incidente de outro tenant e incidente inexistente devolvem o **mesmo** 404:
/// distinguir os dois confirmaria a existência do identificador alheio.
async fn get_incident(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    load_incident(&state.pool, &grant, &id)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

async fn post_replicate(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Replicate, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

async fn post_ack_technical(
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
async fn post_acknowledge(
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

async fn post_close(
    State(state): State<AppState>,
    Extension(grant): Extension<VerifiedGrant>,
    Path(id): Path<String>,
) -> Result<Json<IncidentView>, ApiErrorResponse> {
    transition(&state.pool, &grant, &id, Transition::Close, None)
        .await
        .map(Json)
        .map_err(ApiErrorResponse::from)
}

async fn post_event(
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

async fn get_events(
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

/// Recebe um lote de posições do Inbox do tenant autenticado.
///
/// Corpo inválido (JSON ilegível, campo desconhecido, lote vazio ou acima de
/// `MAX_INBOX_BATCH`) é recusado na fronteira, antes de qualquer efeito. O teto é
/// limite desta fatia, declarado e recusado — nunca truncado em silêncio.
///
/// 200 com `{accepted, duplicates, contiguous_prefix, ack_class}` quando nada
/// falta; 409 `sequence_gap` com `{expected, received, accepted, duplicates,
/// contiguous_prefix}` quando o lote salta posição — o trecho contíguo antes da
/// lacuna é persistido e o resto não é aceito.
async fn post_sync_inbox(
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
async fn get_sync_state(
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

/// 201 na primeira gravação, 200 no replay idempotente do mesmo tenant.
const fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

/// Erro HTTP com corpo estável e sem promessa de efeito.
pub struct ApiErrorResponse(ApiError);

impl From<ApiError> for ApiErrorResponse {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}

impl ApiErrorResponse {
    fn status_and_code(&self) -> (StatusCode, &'static str, String) {
        match &self.0 {
            // 401 com `WWW-Authenticate` é o formato esperado por cliente HTTP.
            ApiError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "credencial ausente ou desconhecida".to_owned(),
            ),
            ApiError::InvalidCredential(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_credential",
                e.to_string(),
            ),
            ApiError::CredentialExpired => (
                StatusCode::UNAUTHORIZED,
                "credential_expired",
                "credencial expirada".to_owned(),
            ),
            ApiError::CredentialNotYetValid => (
                StatusCode::UNAUTHORIZED,
                "credential_not_yet_valid",
                "credencial ainda não válida".to_owned(),
            ),
            ApiError::CredentialRevoked => (
                StatusCode::UNAUTHORIZED,
                "credential_revoked",
                "credencial revogada".to_owned(),
            ),
            // 403: a credencial é válida; o que falta é autorização para esta rota.
            ApiError::InsufficientScope => (
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "grant sem o escopo exigido por esta rota".to_owned(),
            ),
            // Falha fechada: revogação ilegível não vira aceitação.
            ApiError::RevocationUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "revocation_unavailable",
                "não foi possível consultar a revogação; acesso negado".to_owned(),
            ),
            ApiError::InvalidRequest(detail) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                detail.clone(),
            ),
            ApiError::GrantDenied(d) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "grant_denied",
                format!("{d:?}"),
            ),
            ApiError::RetentionPolicyMissing => (
                StatusCode::CONFLICT,
                "retention_policy_missing",
                "classe de retenção sem política aplicável à jurisdição do tenant".to_owned(),
            ),
            // Lacuna não é aceita nem completada: a resposta nomeia a posição que
            // a stream espera, e nada é gravado.
            ApiError::SequenceGap { expected, got } => (
                StatusCode::CONFLICT,
                "sequence_gap",
                format!("posição {got} salta a lacuna: a stream espera {expected}"),
            ),
            // Mesmo código de lacuna no lote de Inbox: o prefixo aceito antes da
            // lacuna ficou durável, e a resposta diz onde reenviar e o que já foi
            // confirmado.
            ApiError::InboxGap {
                expected,
                received,
                accepted,
                ..
            } => (
                StatusCode::CONFLICT,
                "sequence_gap",
                format!(
                    "posição {received} salta a lacuna: a stream espera {expected}; \
                     {accepted} posição(ões) antes da lacuna foram persistidas"
                ),
            ),
            // Cursor ilegível/expirado e cursor vazio são recusados de forma
            // distinta de cursor ausente (que começa a leitura do início).
            ApiError::CursorInvalid(detail) => (
                StatusCode::BAD_REQUEST,
                "cursor_invalid",
                detail.clone(),
            ),
            ApiError::CursorIncomparable(detail) => (
                StatusCode::BAD_REQUEST,
                "cursor_incomparable",
                detail.clone(),
            ),
            ApiError::RetentionScopeUnresolved => (
                StatusCode::CONFLICT,
                "retention_scope_unresolved",
                "tenant sem jurisdição declarada".to_owned(),
            ),
            ApiError::IncidentIdConflict => (
                StatusCode::CONFLICT,
                "incident_id_conflict",
                "incident_id já pertence a outro tenant".to_owned(),
            ),
            // 404, nunca 403: um 403 confirmaria que o identificador existe em
            // outro tenant, e existência de incidente alheio não é dado público.
            ApiError::IncidentNotFound => (
                StatusCode::NOT_FOUND,
                "incident_not_found",
                "incidente não encontrado".to_owned(),
            ),
            ApiError::IllegalTransition { current } => (
                StatusCode::CONFLICT,
                "illegal_transition",
                format!("transição ilegal a partir de {current}"),
            ),
            ApiError::MissingHumanProof => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "missing_human_proof",
                "reconhecimento humano exige responder (prova de pessoa)".to_owned(),
            ),
            ApiError::Invariant(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invariant_violation",
                format!("contrato do incidente recusou a transição: {e:?}"),
            ),
            ApiError::Storage(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "falha de persistência".to_owned(),
            ),
        }
    }
}

impl ApiErrorResponse {
    /// Campos extras do corpo de erro, quando o cliente precisa deles para agir.
    /// `illegal_transition` devolve o estado atual; lacuna de Inbox devolve a
    /// posição esperada, a recebida e o que já ficou durável.
    fn extra(&self) -> Vec<(&'static str, Value)> {
        match &self.0 {
            ApiError::IllegalTransition { current } => {
                vec![("state", Value::String(current.clone()))]
            }
            ApiError::InboxGap {
                expected,
                received,
                accepted,
                duplicates,
                contiguous_prefix,
            } => vec![
                ("expected", json!(expected)),
                ("received", json!(received)),
                ("accepted", json!(accepted)),
                ("duplicates", json!(duplicates)),
                ("contiguous_prefix", json!(contiguous_prefix)),
            ],
            _ => Vec::new(),
        }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let (status, code, detail) = self.status_and_code();
        let mut body = Map::new();
        body.insert("error".to_owned(), Value::String(code.to_owned()));
        body.insert("detail".to_owned(), Value::String(detail));
        // `ack_class` do erro é o piso honesto: nada foi confirmado.
        body.insert(
            "ack_class".to_owned(),
            Value::String("transport_accepted".to_owned()),
        );
        for (key, value) in self.extra() {
            body.insert(key.to_owned(), value);
        }
        let mut response = (status, Json(Value::Object(body))).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::created_status;
    use axum::http::StatusCode;

    #[test]
    fn replay_is_200_and_first_write_is_201() {
        assert_eq!(created_status(false), StatusCode::OK);
        assert_eq!(created_status(true), StatusCode::CREATED);
    }
}
