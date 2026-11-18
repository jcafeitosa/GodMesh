//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.
//!
//! Autenticação é aplicada por `route_layer` em `/v1/*`: rota nova sob esse
//! router nasce protegida, em vez de depender de alguém lembrar do middleware.
//! O handler só recebe o grant pela extensão preenchida pelo middleware, logo
//! não existe caminho de código que chame o caso de uso sem credencial.
//!
//! O roteador é dividido por domínio (`incidents`, `events`, `sync`, `admin`,
//! `federation`); cada módulo declara as rotas e o escopo que exige. Este módulo
//! mantém só a composição, os portões (abuso, autenticação, escopo), `healthz`,
//! `/metrics` e o formato de erro.
//!
//! Ordem de execução em uma rota `/v1`, de fora para dentro:
//! `observe → http_gate → require_grant → grant_gate → portão de escopo →
//! handler`. Mudar essa ordem muda o que é contado e o que é recusado antes de
//! qualquer efeito; está escrita em `build_router` e é o que os testes de
//! `tests/limits.rs` verificam.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::api::ApiError;
use crate::admission::{Admission, AdmissionConfig};
use crate::auth::{bearer_token, Authorization, GrantRegistry, Scope, VerifiedGrant};
use crate::federation::JoinWindow;
use crate::metrics::{Gauges, Metrics};
use crate::observability::{observe, RequestContext, RequestLog, StderrRequestLog};
use godmesh_federation::link::PeerLinks;

mod admin;
mod events;
mod federation;
mod gate;
mod incidents;
mod sync;

/// Estado compartilhado.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub authorization: Arc<Authorization>,
    pub metrics: Metrics,
    pub log: Arc<dyn RequestLog>,
    /// Janela de frescor aceita no `issued_at` de manifesto de ingresso.
    /// Lida do ambiente na composição do router (padrão: 5 minutos).
    pub join_window: JoinWindow,
    /// Enlaces de federação observados neste processo. `None` quando o runtime
    /// de malha não está anexado: o diagnóstico responde `unknown`, que é o piso
    /// honesto, em vez de fingir saúde.
    pub federation: Option<Arc<PeerLinks>>,
    /// Portão de abuso: cotas, tetos de payload e admissão por classe. Vive em
    /// `Arc` porque o estado é clonado por requisição; os buckets são **por
    /// processo** (ver docs de `crate::admission`).
    pub admission: Arc<Admission>,
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

/// Rotas do servidor com configuração de cotas explícita — é o construtor que
/// os testes de portão usam para baixar cotas sem tocar no ambiente global.
/// O binário usa [`router_with_federation_and_admission`], que além das cotas
/// anexa o runtime de malha.
pub fn router_with_admission(
    pool: PgPool,
    grants: GrantRegistry,
    admission: AdmissionConfig,
) -> Router {
    let authorization = Authorization::new(grants, pool.clone());
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        None,
        admission,
    )
}

/// Rotas do servidor com destino de log injetado. Existe para o teste provar o
/// que foi (e o que não foi) registrado; produção usa [`router`].
pub fn router_with_log(
    pool: PgPool,
    authorization: Authorization,
    log: Arc<dyn RequestLog>,
) -> Router {
    build_router(
        pool,
        authorization,
        log,
        None,
        AdmissionConfig::default(),
    )
}

/// Rotas do servidor com autorização injetada (relógio e fonte de revogação).
///
/// Cada grupo de rotas declara o escopo que exige na própria construção do
/// router: rota nova nasce com escopo explícito, em vez de herdá-lo por omissão.
/// Os construtores públicos abaixo usam [`AdmissionConfig::default`] (tetos
/// altos, documentados): os valores de ambiente só valem na subida do binário,
/// via [`router_with_federation_and_admission`] (ou [`router_with_admission`],
/// sem malha), que é onde a config inválida derruba o processo com EX_CONFIG —
/// ler ambiente silenciosamente dentro de um teste seria um acoplamento
/// invisível.
pub fn router_with(pool: PgPool, authorization: Authorization) -> Router {
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        None,
        AdmissionConfig::default(),
    )
}

/// Rotas com o runtime de federação anexado: habilita o diagnóstico de enlace
/// com estado real. Sem isto, `/v1/federation/**` responde `unknown`.
///
/// As cotas ficam no [`AdmissionConfig::default`]; existe também a variante com
/// cotas explícitas, [`router_with_federation_and_admission`].
pub fn router_with_federation(
    pool: PgPool,
    authorization: Authorization,
    links: Arc<PeerLinks>,
) -> Router {
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        Some(links),
        AdmissionConfig::default(),
    )
}

/// Rotas com os dois anexos de produção: o runtime de malha (diagnóstico de
/// enlace) e as cotas validadas no boot. É **o** construtor do binário.
///
/// `federation` é `Option` porque a malha é desligável por ambiente
/// (`GODMESH_FEDERATION_BIND`): desligada, o diagnóstico responde `unknown` e
/// nenhuma rota muda de comportamento. A cota nunca é opcional — sem config
/// explícita vale [`AdmissionConfig::default`], e o binário é que valida o
/// ambiente antes de chegar aqui.
pub fn router_with_federation_and_admission(
    pool: PgPool,
    grants: GrantRegistry,
    federation: Option<Arc<PeerLinks>>,
    admission: AdmissionConfig,
) -> Router {
    let authorization = Authorization::new(grants, pool.clone());
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        federation,
        admission,
    )
}

fn build_router(
    pool: PgPool,
    authorization: Authorization,
    log: Arc<dyn RequestLog>,
    federation: Option<Arc<PeerLinks>>,
    admission: AdmissionConfig,
) -> Router {
    let state = AppState {
        pool,
        authorization: Arc::new(authorization),
        metrics: Metrics::new(),
        log,
        join_window: JoinWindow::from_env(),
        federation,
        admission: Arc::new(Admission::new(admission)),
    };

    // Métricas operacionais: escopo próprio (`admin:metrics`). Ver a nota em
    // `get_metrics` sobre por que não é só loopback.
    let metrics_read = Router::new()
        .route("/metrics", get(get_metrics))
        .route_layer(middleware::from_fn_with_state(
            ScopeGate(Scope::AdminMetrics),
            require_scope,
        ));

    // O portão de autenticação é aplicado por último (camada mais externa do
    // grupo protegido): ele roda antes do portão de cotas, que lê o
    // `VerifiedGrant` já injetado na extensão, e antes do portão de escopo.
    // ordem de execução por rota: `require_grant → grant_gate → escopo → handler`.
    let protected = incidents::routes()
        .merge(events::routes())
        .merge(sync::routes())
        .merge(admin::routes())
        .merge(federation::routes())
        .merge(metrics_read)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            gate::grant_gate,
        ))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_grant));

    // O observador é a camada mais externa: sem isso, 401/403/404 e rota não
    // casada ficariam fora da contagem — exatamente o que se quer enxergar.
    // Logo abaixo dele está o portão de abuso (payload/IP/timeout), que também
    // enxerga as rotas públicas de federação.
    Router::new()
        .route("/healthz", axum::routing::get(healthz))
        // Federação: o pedido de ingresso é público **por rota**, não por
        // omissão do middleware. Ele fica fora do router protegido, e as demais
        // rotas de federação entram em `protected` com escopo explícito.
        .merge(federation::public_routes())
        .merge(protected)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            gate::http_gate,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), observe))
        .with_state(state)
}

/// Escopo que uma rota exige, carregado pelo middleware de escopo.
#[derive(Clone, Copy, Debug)]
pub(super) struct ScopeGate(pub(super) Scope);

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
            // O log de requisição só recebe `token_id` e tenant, nunca o token.
            if let Some(context) = request.extensions().get::<Arc<RequestContext>>() {
                context.set_identity(&grant.token_id, &grant.subject.tenant_id);
            }
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

/// Métricas no formato de texto do Prometheus.
///
/// Escopo `admin:metrics` em vez de "só loopback" porque a proteção precisa ser
/// verificável e independente da topologia: o processo pode escutar em `0.0.0.0`
/// atrás de proxy, onde todo cliente parece loopback, e um `/metrics` aberto
/// publicaria volume de tráfego e backlog de outbox para qualquer um que alcance
/// a porta. Escopo é negado por padrão (falha fechada) e testado no fio.
///
/// Gauge de outbox desconhecido (banco fora) **não** vira zero: a série não sai
/// e `godmesh_metrics_scrape_dependency_up` marca 0.
async fn get_metrics(State(state): State<AppState>) -> Response {
    let outbox_pending: Option<i64> = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_entry WHERE confirmed_at IS NULL",
    )
    .fetch_one(&state.pool)
    .await
    .ok();

    let gauges = Gauges {
        pool_size: state.pool.size(),
        pool_idle: state.pool.num_idle(),
        outbox_pending,
    };
    let body = state.metrics.render(&gauges);
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

pub(super) const fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

/// Erro HTTP com corpo estável e sem promessa de efeito.
pub(super) struct ApiErrorResponse(ApiError);

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
            // Mesma regra para o registro de grants emitidos: falha fechada.
            ApiError::IssuanceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "issuance_unavailable",
                "não foi possível consultar grants emitidos; acesso negado".to_owned(),
            ),
            // 404 também aqui: token_id desconhecido não confirma nada além de si.
            ApiError::GrantNotFound => (
                StatusCode::NOT_FOUND,
                "grant_not_found",
                "grant não encontrado".to_owned(),
            ),
            ApiError::RandomnessUnavailable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "randomness_unavailable",
                "RNG do sistema indisponível; nenhuma credencial foi emitida".to_owned(),
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
            // 401: o manifesto não prova quem diz ser. Nada foi gravado.
            ApiError::InvalidManifestSignature => (
                StatusCode::UNAUTHORIZED,
                "invalid_manifest_signature",
                "assinatura do manifesto não confere".to_owned(),
            ),
            // 422: assinatura válida, manifesto velho demais para ingresso.
            ApiError::ManifestExpired => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "manifest_expired",
                "manifesto fora da janela de frescor".to_owned(),
            ),
            ApiError::ManifestNotYetValid => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "manifest_not_yet_valid",
                "manifesto emitido no futuro além da tolerância".to_owned(),
            ),
            ApiError::InvalidManifest(detail) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_manifest",
                detail.clone(),
            ),
            ApiError::JoinRequestNotFound => (
                StatusCode::NOT_FOUND,
                "join_request_not_found",
                "pedido de ingresso não encontrado".to_owned(),
            ),
            // 409: o pedido existe e já foi decidido de outro jeito.
            ApiError::JoinRequestStateConflict { state } => (
                StatusCode::CONFLICT,
                "join_request_state_conflict",
                format!("pedido de ingresso em estado {state}"),
            ),
            // 409: a máquina de estados de confiança recusou (ex.: revogado não
            // volta direto a autorizado).
            ApiError::TrustTransition { current, next } => (
                StatusCode::CONFLICT,
                "illegal_trust_transition",
                format!("transição de confiança ilegal: {current} → {next}"),
            ),
            ApiError::FederationInvariant(detail) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "federation_invariant_violation",
                detail.clone(),
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
            ApiError::JoinRequestStateConflict { state } => {
                vec![("state", Value::String(state.clone()))]
            }
            ApiError::TrustTransition { current, next } => vec![
                ("current", Value::String(current.clone())),
                ("next", Value::String(next.clone())),
            ],
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
