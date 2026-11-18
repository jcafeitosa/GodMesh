//! `openapi.json` gerado **do registro de rotas** (`contract::RouteSpec`), não
//! de uma segunda lista.
//!
//! Justificativa de tooling (registrada na evidência da fatia):
//!  * `utoipa 6.0.0` fornece o modelo de dados OpenAPI 3.1 e o derive
//!    `ToSchema` dos tipos de request/response — a fonte dos schemas são as
//!    mesmas structs `serde` que os handlers serializam;
//!  * `utoipa-axum 0.3.0` **não** é usado: seu `Api<...>` exige macros
//!    `#[utoipa::path]` por handler e um router único por `Api`, o que não
//!    compõe com os portões de escopo por rota (`route_layer`) nem com o
//!    modelo "a rota nasce do registro". A compatibilidade declarada dele é
//!    `utoipa ^6` + `axum ^0.8.4` (funciona com axum 0.8.9), mas o encaixe
//!    estrutural falha — o documento é gerado por extrator próprio a partir
//!    do registro, a alternativa prevista na tarefa, e o teste de contrato
//!    (`tests/api_contract.rs`) sonda cada rota do documento contra o router
//!    real: se o documento mentir, o teste falha.
//!
//! Garantias por construção:
//!  * `securitySchemes` bearer global; operação protegida declara a exigência
//!    e o escopo na descrição (OpenAPI não associa escopo a esquema
//!    `http;bearer` — a fonte normativa de escopo é `GET /v1/meta`);
//!  * todo erro declarado responde com o schema `Error` e um exemplo real do
//!    catálogo;
//!  * rota destrutiva documenta `200 DryRunResponse` e o parâmetro `dry_run`;
//!  * código documentado fora do catálogo **pânico na composição** — melhor
//!    um servidor que não sobe do que um contrato que mente.

use std::collections::BTreeMap;

use utoipa::openapi::header::HeaderBuilder;
use utoipa::openapi::path::{
    HttpMethod, Operation, OperationBuilder, Parameter, ParameterBuilder, ParameterIn,
    PathItemBuilder,
};
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::schema::{ArrayBuilder, Object, ObjectBuilder, Type};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityRequirement, SecurityScheme};
use utoipa::openapi::{
    Components, ComponentsBuilder, Content, ContentBuilder, InfoBuilder, OpenApi, OpenApiBuilder,
    PathsBuilder, Ref, RefOr, Required as OpenRequired, Response, ResponseBuilder,
    ResponsesBuilder, Schema as SchemaValue, ServerBuilder,
};

use super::contract::{self, Access, RouteSpec, Schema};
use crate::admin::{
    GrantMetadata, IssueGrantPayload, IssuedGrantResponse, RevokeGrantPayload, RevocationResponse,
};
use crate::api::{IncidentPayload, IncidentResponse};
use crate::auth::Scope;
use crate::events::{EventAck, EventPage, EventPayload, EventRecord};
use crate::federation::JoinRequestView;
use crate::inbox::{
    InboxBatchPayload, InboxBatchResponse, InboxGap, InboxPositionPayload, InboxState,
};
use crate::incidents::{IncidentPage, IncidentSummary};
use crate::lifecycle::{AckView, AcknowledgePayload, IncidentView};

/// Nome do esquema de segurança bearer publicado no documento.
pub const BEARER_SCHEME: &str = "bearer";

/// Constrói o documento a partir do registro e devolve o JSON serializado
/// uma única vez (na composição do router).
pub(crate) fn document(specs: &[RouteSpec]) -> String {
    build(specs)
        .to_json()
        .expect("openapi.json deveria sempre serializar")
}

fn free_form() -> Object {
    // Objeto livre: corpo montado em runtime (meta, openapi). Declarar
    // propriedades falsas seria o documento mentir.
    ObjectBuilder::new().schema_type(Type::Object).build()
}

fn primitive(ty: &str) -> Object {
    ObjectBuilder::new()
        .schema_type(match ty {
            "integer" => Type::Integer,
            "boolean" => Type::Boolean,
            _ => Type::String,
        })
        .build()
}

fn schema_of(schema: Schema) -> Option<RefOr<SchemaValue>> {
    match schema {
        Schema::Ref(name) => Some(RefOr::Ref(Ref::from_schema_name(name))),
        Schema::Array(name) => Some(RefOr::T(SchemaValue::Array(
            ArrayBuilder::new()
                .items(Ref::from_schema_name(name))
                .build(),
        ))),
        Schema::Text(_) => None,
    }
}

fn json_content(schema: Option<RefOr<SchemaValue>>, example: Option<serde_json::Value>) -> Content {
    let mut content = ContentBuilder::new();
    if let Some(s) = schema {
        content = content.schema(Some(s));
    }
    if let Some(e) = example {
        content = content.example(Some(e));
    }
    content.build()
}

fn error_response(code: &str) -> Response {
    let spec = contract::error_by_code(code)
        .unwrap_or_else(|| panic!("rota documenta código fora do catálogo: {code}"));
    let mut response = ResponseBuilder::new()
        .description(format!("`{code}` — {}", spec.when))
        .content(
        "application/json",
        json_content(
            Some(RefOr::Ref(Ref::from_schema_name("Error"))),
            contract::error_example(code),
        ),
    );
    if spec.http_status == 401 {
        response = response.header(
            "WWW-Authenticate",
            HeaderBuilder::new()
                .schema(Some(primitive("string")))
                .description(Some("Sempre `Bearer`"))
                .build(),
        );
    }
    response.build()
}

/// Códigos que o portão pode devolver **antes** de o handler rodar.
fn structural_errors(spec: &RouteSpec) -> &'static [&'static str] {
    match spec.access {
        Access::Public => &[],
        Access::Scope(_) => &[
            "unauthenticated",
            "invalid_credential",
            "credential_expired",
            "credential_not_yet_valid",
            "credential_revoked",
            "insufficient_scope",
            "revocation_unavailable",
            "issuance_unavailable",
        ],
    }
}

fn operation(spec: &RouteSpec) -> OperationBuilder {
    let doc = spec.doc;
    let mut op = OperationBuilder::new()
        .summary(Some(doc.summary))
        .operation_id(Some(format!(
            "{}_{}",
            spec.method.to_lowercase(),
            spec.path.trim_start_matches('/').replace(['/', '{', '}'], "_")
        )));

    // Descrição: escopo, mutação, destrutividade e limites — o que um agente
    // precisa para decidir sem adivinhar.
    let mut notes: Vec<String> = Vec::new();
    match spec.access {
        Access::Public => notes.push("Pública: nenhuma credencial é exigida.".to_owned()),
        Access::Scope(scope) => notes.push(format!("Escopo exigido: `{}`.", scope.as_str())),
    }
    if spec.mutating {
        notes.push(format!(
            "Mutante: aceita cabeçalho `{}` (1..={} bytes ASCII imprimíveis; presente e \
             inválido devolve 422). A deduplicação do efeito por chave é a próxima fatia.",
            contract::IDEMPOTENCY_KEY_HEADER,
            contract::MAX_IDEMPOTENCY_KEY_LEN
        ));
    }
    if spec.destructive {
        notes.push(
            "Destrutiva: `?dry_run=true` devolve 200 `{would_do, applied: false}` sem executar \
             nada."
                .to_owned(),
        );
    }
    if let Some(max) = spec.max_body_bytes {
        notes.push(format!("Corpo máximo: {max} bytes."));
    }
    op = op.description(Some(notes.join(" ")));

    let mut params: Vec<Parameter> = doc
        .params
        .iter()
        .map(|p| {
            ParameterBuilder::new()
                .name(p.name)
                .parameter_in(if p.in_path {
                    ParameterIn::Path
                } else {
                    ParameterIn::Query
                })
                .required(if p.required {
                    OpenRequired::True
                } else {
                    OpenRequired::False
                })
                .schema(Some(primitive(p.ty)))
                .description(Some(p.desc))
                .build()
        })
        .collect();
    if spec.destructive {
        params.push(
            ParameterBuilder::new()
                .name("dry_run")
                .parameter_in(ParameterIn::Query)
                .required(OpenRequired::False)
                .schema(Some(primitive("boolean")))
                .description(Some("true = valide e devolva o que faria, sem nenhum efeito"))
                .build(),
        );
    }
    if !params.is_empty() {
        op = op.parameters(Some(params));
    }

    if let Some(name) = doc.request {
        op = op.request_body(Some(
            RequestBodyBuilder::new()
                .content(
                    "application/json",
                    json_content(
                        Some(RefOr::Ref(Ref::from_schema_name(name))),
                        None,
                    ),
                )
                .required(Some(OpenRequired::True))
                .build(),
        ));
    }

    let mut responses = ResponsesBuilder::new();
    for (status, schema) in doc.success {
        let media = match schema {
            Schema::Text(m) => *m,
            _ => "application/json",
        };
        // Exemplo real do `/healthz` (o bloco `capacity` é parte do corpo e o
        // teste de contrato compara o corpo real com as propriedades deste
        // schema; exemplo ausente aqui seria documento mudo sobre o campo).
        let example = match schema {
            Schema::Ref("HealthResponse") => Some(health_example(*status)),
            _ => None,
        };
        responses = responses.response(
            status.to_string(),
            ResponseBuilder::new()
                .description(format!("HTTP {status} esperado por contrato"))
                .content(media, json_content(schema_of(*schema), example))
                .build(),
        );
    }
    if spec.destructive {
        responses = responses.response(
            "200",
            ResponseBuilder::new()
                .description("dry_run=true: o que seria feito, nada aplicado")
                .content(
                    "application/json",
                    json_content(
                        Some(RefOr::Ref(Ref::from_schema_name("DryRunResponse"))),
                        Some(serde_json::json!({
                            "would_do": [format!("POST {}", spec.path)],
                            "applied": false,
                            "dry_run": true,
                        })),
                    ),
                )
                .build(),
        );
    }
    let mut codes: Vec<&str> = doc.errors.to_vec();
    for code in structural_errors(spec) {
        if !codes.contains(code) {
            codes.push(code);
        }
    }
    for code in codes.iter() {
        let status = contract::error_by_code(code)
            .unwrap_or_else(|| panic!("código documentado fora do catálogo: {code}"))
            .http_status
            .to_string();
        responses = responses.response(status, error_response(code));
    }
    op = op.responses(responses.build());

    if let Access::Scope(_scope) = spec.access {
        op = op.securities(Some([SecurityRequirement::new(
            BEARER_SCHEME,
            Vec::<String>::new(),
        )]));
        // O escopo está na descrição acima e em GET /v1/meta → routes[].scope;
        // OpenAPI não tem campo de escopo para `http;bearer` e fingir OAuth2
        // seria um contrato falso.
    }
    op
}

fn build(specs: &[RouteSpec]) -> OpenApi {
    let mut by_path: BTreeMap<&str, Vec<(HttpMethod, Operation)>> =
        BTreeMap::new();
    for spec in specs {
        let method = match spec.method {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            other => panic!("verbo não previsto no registro: {other}"),
        };
        by_path
            .entry(spec.path)
            .or_default()
            .push((method, operation(spec).build()));
    }
    let mut paths = PathsBuilder::new();
    for (path, ops) in by_path {
        let mut item = PathItemBuilder::new();
        for (method, op) in ops {
            item = item.operation(method, op);
        }
        paths = paths.path(path, item.build());
    }

    OpenApiBuilder::default()
        .info(
            InfoBuilder::new()
                .title(format!("GodMesh API {}", contract::API_VERSION))
                .description(Some(
                    "Contrato gerado do registro de rotas do servidor (utoipa 6.0.0 + extrator \
                     próprio a partir do registro). Fonte normativa de escopos, limites e erros: \
                     GET /v1/meta."
                ))
                .version(env!("CARGO_PKG_VERSION"))
                .build(),
        )
        .servers(Some(vec![ServerBuilder::new().url("/").build()]))
        .paths(paths.build())
        .components(Some(components()))
        .build()
}

fn components() -> Components {
    ComponentsBuilder::default()
        // Tipos de domínio: as mesmas structs serde que os handlers
        // serializam — sem segunda definição de schema para divergir.
        .schema_from::<IncidentPayload>()
        .schema_from::<IncidentResponse>()
        .schema_from::<IncidentView>()
        .schema_from::<AckView>()
        .schema_from::<AcknowledgePayload>()
        .schema_from::<IncidentPage>()
        .schema_from::<IncidentSummary>()
        .schema_from::<EventPayload>()
        .schema_from::<EventAck>()
        .schema_from::<EventRecord>()
        .schema_from::<EventPage>()
        .schema_from::<InboxPositionPayload>()
        .schema_from::<InboxBatchPayload>()
        .schema_from::<InboxBatchResponse>()
        .schema_from::<InboxState>()
        .schema_from::<InboxGap>()
        .schema_from::<IssueGrantPayload>()
        .schema_from::<IssuedGrantResponse>()
        .schema_from::<GrantMetadata>()
        .schema_from::<RevokeGrantPayload>()
        .schema_from::<RevocationResponse>()
        .schema_from::<JoinRequestView>()
        .schema_from::<Scope>()
        .schema_from::<godmesh_federation::manifest::SignedManifest>()
        .schema_from::<godmesh_federation::peer::PeerRecord>()
        .schema_from::<godmesh_federation::link::LinkSnapshot>()
        .schema_from::<godmesh_federation::peer::TrustState>()
        .schema_from::<godmesh_federation::link::LinkState>()
        .schema("Error", error_schema())
        .schema("DryRunResponse", dry_run_schema())
        .schema(contract::META_SCHEMA_COMPONENT, free_form())
        .schema("OpenApiDocument", free_form())
        .schema(
            "HealthResponse",
            ObjectBuilder::new()
                .schema_type(Type::Object)
                .property("status", primitive("string"))
                .property("ack_class", primitive("string"))
                .property("dependency", primitive("string"))
                .property("capacity", health_capacity_schema())
                .required("status")
                .description(Some(
                    "`dependency` só aparece no 503: nomeia o que não respondeu. `capacity` é \
                     dado de host (ADR-073 camada 1), publicado também no 503, e não julga \
                     saúde: pressão alta continua 200 por decisão desta fatia.",
                ))
                .build(),
        )
        .security_scheme(
            BEARER_SCHEME,
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "Token opaco emitido por POST /v1/admin/grants. O escopo exigido por \
                         operação está em GET /v1/meta → routes[].scope e na descrição da \
                         operação.",
                    ))
                    .build(),
            ),
        )
        .build()
}

/// Bloco `capacity` do `/healthz` (ADR-073 camada 1): o resumo que o handler
/// publica em `HostTelemetry::capacity_json`. `null` é "não medido" — os tipos
/// abaixo descrevem o valor medido; a ausência é declarada na descrição e
/// provada pelo teste de contrato (corpo real ↔ propriedades do schema).
/// Exemplo do corpo de `/healthz` por status: mesmo formato do handler, com
/// `capacity` medido no 200 e `null` no 503 (dependência fora não apaga o
/// painel de disponibilidade do host).
fn health_example(status: u16) -> serde_json::Value {
    let capacity = serde_json::json!({
        "cpu_pressure": 1.5,
        "memory_pressure": 0.75,
        "disk_free_ratio": 0.42,
        "source": "proc",
        "container": false,
        "snapshot_age_seconds": 2.0,
        "pressure_is_not_a_health_signal": true,
    });
    if status == 200 {
        serde_json::json!({
            "status": "ok",
            "ack_class": "transport_accepted",
            "capacity": capacity,
        })
    } else {
        serde_json::json!({
            "status": "unavailable",
            "dependency": "postgres",
            "ack_class": "transport_accepted",
            "capacity": {
                "cpu_pressure": null,
                "memory_pressure": null,
                "disk_free_ratio": null,
                "source": "none",
                "container": false,
                "snapshot_age_seconds": null,
                "pressure_is_not_a_health_signal": true,
            },
        })
    }
}

fn health_capacity_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property("cpu_pressure", primitive("number"))
        .property("memory_pressure", primitive("number"))
        .property("disk_free_ratio", primitive("number"))
        .property("source", primitive("string"))
        .property("container", primitive("boolean"))
        .property("snapshot_age_seconds", primitive("number"))
        .property("pressure_is_not_a_health_signal", primitive("boolean"))
        .required("source")
        .required("container")
        .required("pressure_is_not_a_health_signal")
        .description(Some(
            "Pressão de CPU/memória (% de tempo stall, janela do PSI) e fração livre do \
             volume raiz. `cpu_pressure`, `memory_pressure`, `disk_free_ratio` e \
             `snapshot_age_seconds` são `null` quando não medidos — série ausente não é \
             zero. `source` ∈ `proc` | `cgroup` | `statfs` | `none`; `container` diz se a \
             leitura é do namespace. Pressão **não** é sinal de saúde nesta fatia.",
        ))
        .build()
}

fn error_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property("error", primitive("string"))
        .property("detail", primitive("string"))
        .property("hint", primitive("string"))
        .property("docs_url", primitive("string"))
        .property("retryable", primitive("boolean"))
        .property("ack_class", primitive("string"))
        .required("error")
        .required("detail")
        .required("hint")
        .required("retryable")
        .description(Some(
            "Corpo de erro estável e acionável (ADR-074): `error` é o código de máquina; \
             `hint` diz o próximo passo concreto.",
        ))
        .build()
}

fn dry_run_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property(
            "would_do",
            ArrayBuilder::new()
                .items(primitive("string"))
                .build(),
        )
        .property("applied", primitive("boolean"))
        .property("dry_run", primitive("boolean"))
        .required("would_do")
        .required("applied")
        .description(Some("Resposta de `?dry_run=true`: descreve, não faz."))
        .build()
}
