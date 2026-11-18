//! Camada HTTP: traduz o caso de uso para axum, sem regra de domínio aqui.
//!
//! Autenticação é aplicada por `route_layer` no grupo protegido: rota nova sob
//! esse router nasce protegida, em vez de depender de alguém lembrar do
//! middleware. O handler só recebe o grant pela extensão preenchida pelo
//! middleware, logo não existe caminho de código que chame o caso de uso sem
//! credencial.
//!
//! **Onde uma rota nasce:** `contract::Registry`. Os módulos de domínio
//! (incidents, events, sync, admin, federation) *registran* handlers no
//! registro; o roteador, `GET /v1/meta` e `openapi.json` são três leituras do
//! mesmo registro. Não existe `.route()` fora de `contract.rs` — o teste de
//! contrato (`tests/api_contract.rs`) vigia essa regra e sonda o router real
//! contra o documento: documento que mente derruba o build do teste.
//!
//! Este módulo mantém a composição, os portões de autenticação/escopo, o
//! portão de abuso (`gate`) e o formato de erro (código/status/detail via
//! `contract::error_parts`, campos acionáveis via catálogo).
//!
//! Ordem de execução em uma rota `/v1` protegida, de fora para dentro:
//! `observe → http_gate → require_grant → grant_gate → portão de escopo →
//! portão semântico (idempotência/dry_run) → handler`. Mudar essa ordem muda
//! o que é contado e o que é recusado antes de qualquer efeito; é o que os
//! testes de `tests/limits.rs` verificam.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::admission::{Admission, AdmissionConfig};
use crate::api::ApiError;
use crate::auth::{bearer_token, Authorization, GrantRegistry, Scope, VerifiedGrant};
use crate::federation::JoinWindow;
use crate::metrics::{Gauges, Metrics};
use crate::observability::{observe, RequestContext, RequestLog, StderrRequestLog};
use godmesh_federation::link::PeerLinks;

mod admin;
mod contract;
mod events;
mod federation;
mod gate;
mod incidents;
mod openapi;
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
    /// honesto, em vez de fingir saúde. `GET /v1/meta →
    /// capabilities.federation_enabled` publica exatamente isto.
    pub federation: Option<Arc<PeerLinks>>,
    /// Portão de abuso: cotas, tetos de payload e admissão por classe. Vive em
    /// `Arc` porque o estado é clonado por requisição; os buckets são **por
    /// processo** (ver docs de `crate::admission`). Os limites efetivos saem
    /// daqui para `GET /v1/meta → routes[].limits.rate`.
    pub admission: Arc<Admission>,
    /// O **publicador** do outbox roda neste processo? Estado efetivo, não
    /// intenção: `GET /v1/meta → capabilities.outbox_enabled` publica isto.
    /// Os construtores de teste não rodam publicador, logo `false`; o
    /// binário chama [`router_runtime`] com a flag real.
    pub outbox_enabled: bool,
    /// A tabela que construiu este router. `GET /v1/meta` publica daqui.
    route_specs: Arc<Vec<contract::RouteSpec>>,
    /// `openapi.json` serializado uma vez na composição (mesmo registro).
    openapi_json: Arc<String>,
    /// Derivado do registro: `/metrics` foi montado neste processo.
    host_metrics_available: bool,
}

/// Rotas do servidor com a composição de produção (relógio do sistema,
/// revogação em Postgres). Publicador **não** está anexado e as cotas são as
/// padrão documentadas: capacidades do contrato refletem o processo real
/// deste router (ADR-074).
pub fn router(pool: PgPool, grants: GrantRegistry) -> Router {
    let authorization = Authorization::new(grants, pool.clone());
    router_with(pool, authorization)
}

/// Rotas do servidor com configuração de cotas explícita — é o construtor que
/// os testes de portão usam para baixar cotas sem tocar no ambiente global.
/// O binário usa [`router_runtime`], que além das cotas anexa o runtime de
/// malha e declara o estado efetivo do publicador.
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
        false,
    )
}

/// Rotas do servidor com destino de log injetado. Existe para o teste provar o
/// que foi (e o que não foi) registrado; produção usa [`router_runtime`].
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
        false,
    )
}

/// Rotas do servidor com autorização injetada (relógio e fonte de revogação).
///
/// Cada rota declara o escopo que exige **no registro**; rota nova nasce com
/// escopo explícito, em vez de herdá-lo por omissão. Os construtores públicos
/// abaixo usam [`AdmissionConfig::default`] (tetos altos, documentados): os
/// valores de ambiente só valem na subida do binário, via [`router_runtime`],
/// que é onde a config inválida derruba o processo com EX_CONFIG — ler
/// ambiente silenciosamente dentro de um teste seria um acoplamento invisível.
pub fn router_with(pool: PgPool, authorization: Authorization) -> Router {
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        None,
        AdmissionConfig::default(),
        false,
    )
}

/// Rotas com o runtime de federação anexado: habilita o diagnóstico de enlace
/// com estado real. Sem isto, `/v1/federation/**` responde `unknown` e
/// `capabilities.federation_enabled` é `false`.
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
        false,
    )
}

/// Rotas com os dois anexos de produção: o runtime de malha (diagnóstico de
/// enlace) e as cotas validadas no boot. Sem publicador anexado
/// (`capabilities.outbox_enabled = false`) — a porta completa do binário é
/// [`router_runtime`].
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
        false,
    )
}

/// Composição completa do binário: federação, cotas e publicador com seus
/// estados efetivos. É a única porta que declara `outbox_enabled = true`, e só
/// o `main` a chama quando o publicador roda de fato.
///
/// `federation` é `Option` porque a malha é desligável por ambiente
/// (`GODMESH_FEDERATION_BIND`): desligada, o diagnóstico responde `unknown` e
/// nenhuma rota muda de comportamento. A cota nunca é opcional — sem config
/// explícita vale [`AdmissionConfig::default`], e o binário é que valida o
/// ambiente antes de chegar aqui.
pub fn router_runtime(
    pool: PgPool,
    grants: GrantRegistry,
    federation: Option<Arc<PeerLinks>>,
    admission: AdmissionConfig,
    outbox_enabled: bool,
) -> Router {
    let authorization = Authorization::new(grants, pool.clone());
    build_router(
        pool,
        authorization,
        Arc::new(StderrRequestLog),
        federation,
        admission,
        outbox_enabled,
    )
}

fn build_router(
    pool: PgPool,
    authorization: Authorization,
    log: Arc<dyn RequestLog>,
    federation: Option<Arc<PeerLinks>>,
    admission: AdmissionConfig,
    outbox_enabled: bool,
) -> Router {
    let join_window = JoinWindow::from_env();

    // 1. O registro é a única fonte: ele constrói o router E é publicado.
    let mut reg = contract::Registry::new();
    incidents::register(&mut reg);
    events::register(&mut reg);
    sync::register(&mut reg);
    admin::register(&mut reg);
    federation::register(&mut reg);

    // Métricas operacionais: escopo próprio (`admin:metrics`). Ver a nota em
    // `get_metrics` sobre por que não é só loopback.
    reg.get(
        "/metrics",
        contract::Access::Scope(Scope::AdminMetrics),
        contract::RouteDoc {
            summary: "Métricas Prometheus agregadas do nó (sem dado de tenant)",
            request: None,
            success: &[(200, contract::Schema::Text("text/plain; version=0.0.4; charset=utf-8"))],
            errors: &["storage_unavailable"],
            params: &[],
        },
        get_metrics,
    );

    // Endpoints de contrato e liveness, públicos por rota (não expõem dado de
    // tenant e não têm efeito).
    reg.get(
        "/healthz",
        contract::Access::Public,
        contract::RouteDoc {
            summary: "Liveness com dependência real: 200 só com Postgres respondendo",
            request: None,
            success: &[
                (200, contract::Schema::Ref("HealthResponse")),
                (503, contract::Schema::Ref("HealthResponse")),
            ],
            errors: &[],
            params: &[],
        },
        healthz,
    );
    reg.get(
        "/v1/meta",
        contract::Access::Public,
        contract::RouteDoc {
            summary: "Contrato legível por máquina: versões, rotas, erros, capacidades",
            request: None,
            success: &[(200, contract::Schema::Ref(contract::META_SCHEMA_COMPONENT))],
            errors: &[],
            params: &[],
        },
        contract::get_meta,
    );
    reg.get(
        "/openapi.json",
        contract::Access::Public,
        contract::RouteDoc {
            summary: "Documento OpenAPI 3.1 gerado do registro de rotas",
            request: None,
            success: &[(200, contract::Schema::Ref("OpenApiDocument"))],
            errors: &[],
            params: &[],
        },
        get_openapi_json,
    );

    let out = reg.build();
    // Os limites efetivos do processo (mesma `AdmissionConfig` que os portões
    // aplicam) entram no registro **antes** de ele virar meta/openapi: uma
    // única fonte, três leituras honestas.
    let admission = Arc::new(Admission::new(admission));
    let mut specs = out.specs;
    contract::apply_effective_limits(&mut specs, admission.config());
    let route_specs = Arc::new(specs);
    // O documento OpenAPI nasce do MESMO registro que montou o router; se um
    // dia divergir, o teste de contrato sonda rota por rota e falha.
    let openapi_json = Arc::new(openapi::document(&route_specs));
    let host_metrics_available = route_specs.iter().any(|spec| spec.path == "/metrics");

    let state = AppState {
        pool,
        authorization: Arc::new(authorization),
        metrics: Metrics::new(),
        log,
        join_window,
        federation,
        admission,
        outbox_enabled,
        route_specs,
        openapi_json,
        host_metrics_available,
    };

    // 2. O portão de autenticação é a camada mais externa do grupo protegido
    // (roda por último ao adicionar); abaixo dele está o portão de abuso
    // pós-autenticação, que conhece o `VerifiedGrant` injetado na extensão.
    // ordem por rota protegida: `require_grant → grant_gate → escopo →
    // portão semântico → handler`.
    let protected = out
        .protected
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            gate::grant_gate,
        ))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_grant));

    // 3. O observador é a camada mais externa: sem isso, 401/403/404 e rota
    // não casada ficariam fora da contagem — exatamente o que se quer
    // enxergar. Logo abaixo dele está o portão de abuso (payload/IP/timeout),
    // que também enxerga as rotas públicas do registro.
    Router::new()
        .merge(out.public)
        .merge(protected)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            gate::http_gate,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), observe))
        .with_state(state)
}

/// Escopo exigido por uma rota, carregado pelo middleware de escopo.
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

/// `GET /openapi.json`: o documento serializado na composição. Não há
/// segunda fonte — a string é o registro renderizado.
async fn get_openapi_json(State(state): State<AppState>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        state.openapi_json.as_str().to_owned(),
    )
        .into_response()
}

/// Portão único de autenticação das rotas protegidas.
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

#[must_use]
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
    /// Corpo padronizado e acionável (ADR-074):
    /// `{error, detail, hint, docs_url, retryable, ack_class, ...extras}`.
    /// `error`/`detail` preservam os códigos testados; `hint` diz o próximo
    /// passo concreto; `docs_url` aponta para o catálogo público.
    fn into_response(self) -> Response {
        let (status, code, detail) = contract::error_parts(&self.0);
        let mut body = Map::new();
        body.insert("error".to_owned(), Value::String(code.to_owned()));
        body.insert("detail".to_owned(), Value::String(detail));
        // `ack_class` do erro é o piso honesto: nada foi confirmado.
        body.insert(
            "ack_class".to_owned(),
            Value::String("transport_accepted".to_owned()),
        );
        if let Some(spec) = contract::error_by_code(code) {
            body.insert("hint".to_owned(), Value::String(spec.hint.to_owned()));
            body.insert("retryable".to_owned(), json!(spec.retryable));
            body.insert("docs_url".to_owned(), Value::String("/v1/meta".to_owned()));
        }
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
