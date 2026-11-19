//! Registro de rotas: o **único** lugar onde uma rota passa a existir.
//!
//! Por que um registro e não `Router::route()` espalhado pelos módulos: o
//! contrato (`GET /v1/meta`, `openapi.json` e o teste de contrato) precisa da
//! mesma fonte que o roteador real usa. Se a lista de rotas fosse escrita à
//! mão ao lado do router, ela mentiria no dia em que alguém registrasse uma
//! rota e esquecesse a lista. Aqui é o contrário: o router **é construído a
//! partir do registro**, e o documento também. Registrar sem documentar não é
//! possível — a assinatura de registro exige os campos do documento.
//!
//! O registro também carrega a semântica segura para agente (ADR-074):
//!  * `mutating`: a rota pode ter `Idempotency-Key`; o cabeçalho presente e
//!    inválido é recusado **antes** do handler (422). A aceitação plena da
//!    chave (deduplicação de efeito) fica para a próxima fatia — declarar e
//!    validar é o que vive aqui.
//!  * `destructive`: a rota aceita `?dry_run=true`, que devolve `200
//!    {would_do, applied: false}` sem executar o handler — portanto sem
//!    escrever nada. Provar "sem efeito" com SQL antes/depois é papel do teste
//!    de contrato (`tests/api_contract.rs`).
//!  * erros: catálogo único derivado do `match` **exaustivo** de [`advice`].
//!    Variante nova de `ApiError` sem entrada de catálogo não compila.

use std::sync::OnceLock;

use axum::extract::{Request, State};
use axum::handler::Handler;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use axum::{Json, Router};
use serde_json::{json, Value};

use super::{require_scope, AppState, ApiErrorResponse, ScopeGate};
use crate::admission::{route_class, AdmissionConfig, BodyKind};
use crate::api::ApiError;
use crate::auth::Scope;

/// Versão publicada da API HTTP. O perfil versionado de compatibilidade é o
/// `v1`; `supported_versions` segue a política N/N-1 da ADR-056: com apenas um
/// perfil publicado, não existe N-1 a anunciar — e anunciar v0 inventado seria
/// pior que admitir o intervalo vazio.
pub const API_VERSION: &str = "v1";
pub const SUPPORTED_VERSIONS: &[&str] = &["v1"];
pub const COMPATIBILITY_POLICY: &str = "N/N-1 (ADR-056)";

/// Nome do cabeçalho de idempotência (IETF `Idempotency-Key`) e seus limites
/// declarados no contrato. A **validação** vive aqui; a deduplicação de
/// efeitos é a próxima fatia.
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
pub const MAX_IDEMPOTENCY_KEY_LEN: usize = 128;

/// Como uma rota é alcançada.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Access {
    /// Pública por rota: sem `require_grant`, sem escopo. Só existe onde a
    /// ausência de credencial é deliberada (pedido de ingresso, healthz,
    /// contrato).
    Public,
    /// Exige grant verificado contendo o escopo declarado.
    Scope(Scope),
}

/// Referência a um schema do documento OpenAPI.
#[derive(Clone, Copy)]
pub(crate) enum Schema {
    /// Componente nomeado (`components.schemas.<N>`).
    Ref(&'static str),
    /// Array do componente nomeado.
    Array(&'static str),
    /// Texto puro com o media type dado (métricas Prometheus).
    Text(&'static str),
}

/// Parâmetro de path/query documentado.
#[derive(Clone, Copy)]
pub(crate) struct ParamSpec {
    pub name: &'static str,
    pub in_path: bool,
    pub required: bool,
    /// `Type` do OpenAPI: "string" | "integer".
    pub ty: &'static str,
    pub desc: &'static str,
}

/// O que uma rota devolve, aceita e pode recusar. Tudo `&'static`: o registro
/// é composição, não estado por requisição.
#[derive(Clone, Copy)]
pub(crate) struct RouteDoc {
    pub summary: &'static str,
    pub request: Option<&'static str>,
    /// Sucessos possíveis: (status, schema). Rotas destrutivas ganham o
    /// `200 DryRunResponse` do registro automaticamente; não repetir aqui.
    pub success: &'static [(u16, Schema)],
    /// Códigos de erro **específicos** da rota. Os de autenticação/escopo são
    /// anexados automaticamente pelo contrato conforme o `Access`.
    pub errors: &'static [&'static str],
    pub params: &'static [ParamSpec],
}

/// Fato registrado sobre uma rota. `GET /v1/meta` publica isto, `openapi.json`
/// deriva disto, e o router foi construído a partir disto.
#[derive(Clone)]
pub(crate) struct RouteSpec {
    pub method: &'static str,
    pub path: &'static str,
    pub access: Access,
    pub mutating: bool,
    pub destructive: bool,
    /// Teto de corpo **efetivo** deste processo, resolvido do `AdmissionConfig`
    /// na composição (`apply_effective_limits`) — não é um número paralelo.
    pub max_body_bytes: Option<u64>,
    pub doc: RouteDoc,
}

impl RouteSpec {
    /// Escopo exigido como nome estável, ou `null` para rota pública.
    pub(crate) fn scope_name(&self) -> Option<&'static str> {
        match self.access {
            Access::Public => None,
            Access::Scope(s) => Some(s.as_str()),
        }
    }
}

/// Erro do catálogo: `{error, http_status, when, retryable, hint}`.
#[derive(Clone, Copy)]
pub(crate) struct ErrorSpec {
    pub error: &'static str,
    pub http_status: u16,
    pub when: &'static str,
    pub retryable: bool,
    pub hint: &'static str,
}

/// Constrói o router a partir do registro. `mod.rs` escolhe se o grupo vai
/// para o lado público ou protegido; o portão de `require_grant` continua
/// sendo aplicado por fora, como antes.
#[derive(Default)]
pub(crate) struct Registry {
    specs: Vec<RouteSpec>,
    public: Vec<Router<AppState>>,
    protected: Vec<Router<AppState>>,
}

pub(crate) struct RegistryOut {
    pub public: Router<AppState>,
    pub protected: Router<AppState>,
    pub specs: Vec<RouteSpec>,
}

impl Registry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn add_route(
        &mut self,
        verb: (&'static str, bool, bool),
        path: &'static str,
        access: Access,
        mr: MethodRouter<AppState>,
        doc: RouteDoc,
    ) {
        let (method, mutating, destructive) = verb;
        debug_assert!(
            method == "POST" || method == "GET",
            "registro só conhece os verbos usados pelo contrato"
        );
        // A rota existe **uma** vez: o mesmo `router` alimentado ao merge e o
        // mesmo `spec` alimentado a /v1/meta e ao openapi.json.
        let router = Router::new().route(path, mr);
        // Camada interna primeiro: o portão semântico (idempotência/dry_run)
        // só roda depois de autenticação+escopo, que ficam por fora.
        let router = if mutating || destructive {
            router.route_layer(middleware::from_fn_with_state(
                Gate { destructive },
                gate,
            ))
        } else {
            router
        };
        let router = match access {
            Access::Scope(scope) => router.route_layer(middleware::from_fn_with_state(
                ScopeGate(scope),
                require_scope,
            )),
            Access::Public => router,
        };
        self.specs.push(RouteSpec {
            method,
            path,
            access,
            mutating,
            destructive,
            // Preenchido na composição por `apply_effective_limits`: o registro
            // sozinho não conhece a configuração de cotas do processo.
            max_body_bytes: None,
            doc,
        });
        match access {
            Access::Public => self.public.push(router),
            Access::Scope(_) => self.protected.push(router),
        }
    }

    pub(crate) fn get<H, T>(&mut self, path: &'static str, access: Access, doc: RouteDoc, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.add_route(("GET", false, false), path, access, MethodRouter::new().get(handler), doc);
    }

    pub(crate) fn post<H, T>(&mut self, path: &'static str, access: Access, doc: RouteDoc, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.add_route(
            ("POST", true, false),
            path,
            access,
            MethodRouter::new().post(handler),
            doc,
        );
    }

    /// POST que **não pode ser desfeito**: o registro impõe a semântica de
    /// `dry_run` e a marca no contrato, em vez de depender de lembrança.
    pub(crate) fn post_destructive<H, T>(
        &mut self,
        path: &'static str,
        access: Access,
        doc: RouteDoc,
        handler: H,
    ) where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.add_route(
            ("POST", true, true),
            path,
            access,
            MethodRouter::new().post(handler),
            doc,
        );
    }

    pub(crate) fn build(self) -> RegistryOut {
        let merge_all = |routers: Vec<Router<AppState>>| {
            routers
                .into_iter()
                .reduce(|acc, r| acc.merge(r))
                .unwrap_or_default()
        };
        RegistryOut {
            public: merge_all(self.public),
            protected: merge_all(self.protected),
            specs: self.specs,
        }
    }
}

// ---------------------------------------------------------------------------
// Portão semântico: Idempotency-Key e dry_run.
// ---------------------------------------------------------------------------

/// Estado do portão por rota.
#[derive(Clone, Copy)]
pub(crate) struct Gate {
    pub destructive: bool,
}

/// `true`/`false`/ausente/malformado sem panic e sem aceitar sinonimo
/// silencioso: `1`, `yes`, `TRUE` são **malformados**, porque aceitar meia
/// dúzia de interpretações de um parâmetro é como `?admin=1` virar `?admin=yes`.
enum Flag {
    Absent,
    True,
    False,
    Malformed,
}

fn query_flag(query: &str, key: &str) -> Flag {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            return match v {
                "true" => Flag::True,
                "false" => Flag::False,
                _ => Flag::Malformed,
            };
        }
    }
    Flag::Absent
}

/// Cabeçalho presente é validado; ausente continua aceito (compatibilidade:
/// nenhum cliente atual quebra com esta fatia). Charset: ASCII imprimível
/// 0x21..=0x7E, 1..=128 bytes — o conjunto conservador do rascunho
/// `Idempotency-Key`.
fn idempotency_key_is_valid(value: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDEMPOTENCY_KEY_LEN
        && value.iter().all(|b| (0x21..=0x7e).contains(b))
}

fn invalid_request(detail: &str) -> Response {
    ApiErrorResponse::from(ApiError::InvalidRequest(detail.to_owned())).into_response()
}

async fn gate(State(gate): State<Gate>, request: Request, next: Next) -> Response {
    if let Some(value) = request.headers().get(IDEMPOTENCY_KEY_HEADER)
        && !idempotency_key_is_valid(value.as_bytes())
    {
        return invalid_request(
            "Idempotency-Key deve ter 1 a 128 bytes ASCII imprimíveis (0x21..=0x7E)",
        );
    }
    match query_flag(request.uri().query().unwrap_or(""), "dry_run") {
        Flag::Absent | Flag::False => next.run(request).await,
        Flag::Malformed => invalid_request("dry_run aceita apenas 'true' ou 'false'"),
        Flag::True if gate.destructive => {
            // O handler nunca roda: nenhuma escrita acontece aqui, e o teste
            // de contrato prova isso por contagem SQL antes/depois.
            let would = format!(
                "{} {}",
                request.method().as_str(),
                request.uri().path()
            );
            Json(json!({
                "would_do": [would],
                "applied": false,
                "dry_run": true,
            }))
            .into_response()
        }
        Flag::True => invalid_request(
            "dry_run=true só é aceito em rotas destrutivas; a lista está em GET /v1/meta",
        ),
    }
}

// ---------------------------------------------------------------------------
// Catálogo de erros: uma entrada por variante de ApiError, garantida pelo
// compilador (match exaustivo, sem wildcard).
// ---------------------------------------------------------------------------

use crate::portable_join::PortableJoinError as Join;

fn portable_error(error: &Join) -> (StatusCode, &'static str, &'static str) {
    let invalid = StatusCode::UNPROCESSABLE_ENTITY;
    let conflict = StatusCode::CONFLICT;
    match error {
        Join::Proof(_) => (invalid, "join_proof_invalid", "a prova portátil foi recusada"),
        Join::Issuer => (invalid, "join_issuer_not_authorized", "o nó não tem autoridade portátil para esta operação"),
        Join::Validity => (invalid, "join_validity_invalid", "a vigência apresentada ou configurada não se aplica à prova"),
        Join::Revoked => (invalid, "join_proof_revoked", "a cadeia contém um elo revogado"),
        Join::Revocation => (invalid, "join_revocation_invalid", "a declaração ou o recibo de revogação foi recusado"),
        Join::Capacity => (invalid, "join_envelope_capacity_exceeded", "um envelope de uso da cadeia excede o teto local de corpo"),
        Join::Replacement => (conflict, "join_chain_replacement_conflict", "a origem ou a prova vigente conflita com a importação"),
        Join::RevocationConflict => (conflict, "join_revocation_conflict", "o elo já tem outra declaração de revogação"),
        Join::NotFound => (StatusCode::NOT_FOUND, "join_proof_not_found", "nenhuma prova confirmada nesta raiz para o pedido"),
        Join::Configuration(_) | Join::Encoding(_) | Join::Storage(_) =>
            (StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable", "estado portátil indisponível"),
        Join::Disabled => (invalid, "federation_governance_disabled", "governança portátil desabilitada"),
        Join::Epoch { .. } => (invalid, "approval_epoch_regressed", "epoch inferior ao piso registrado"),
        Join::State(_) => (conflict, "join_request_state_conflict", "o pedido já foi decidido"),
    }
}

/// `(when, retryable, hint)` por variante.
fn advice(e: &ApiError) -> (&'static str, bool, &'static str) {
    match e {
        ApiError::PortableJoin(error) => (
            portable_error(error).2,
            false,
            "confira a prova, a política e o estado do pedido na raiz",
        ),
        ApiError::Unauthenticated => (
            "nenhuma credencial apresentada, esquema errado ou token desconhecido",
            false,
            "autentique com Authorization: Bearer <token>; token é emitido por POST /v1/admin/grants com escopo admin:grants",
        ),
        ApiError::InvalidCredential(_) => (
            "credencial reconhecida mas com sujeito inutilizável (tenant ou principal vazio)",
            false,
            "peça um grant com tenant_id e principal válidos em POST /v1/admin/grants",
        ),
        ApiError::CredentialExpired => (
            "o grant tem expires_at no passado",
            false,
            "emita um novo grant em POST /v1/admin/grants; o token atual não será revalidado",
        ),
        ApiError::CredentialNotYetValid => (
            "o grant tem not_before no futuro",
            true,
            "aguarde not_before e repita; relógio do cliente desalinhado é a causa comum",
        ),
        ApiError::CredentialRevoked => (
            "o grant foi revogado (configuração ou POST /v1/admin/grants/{token_id}/revoke)",
            false,
            "solicite um novo grant ao operador; a revogação não é reversível",
        ),
        ApiError::InsufficientScope => (
            "a credencial é válida, mas não contém o escopo exigido pela rota",
            false,
            "peça o escopo exigido (veja GET /v1/meta → routes[].scope) ao operador com admin:grants",
        ),
        ApiError::RevocationUnavailable => (
            "o banco de revogação não respondeu; o acesso é negado (falha fechada)",
            true,
            "repita com backoff; se persistir, verifique a dependência Postgres do nó",
        ),
        ApiError::IssuanceUnavailable => (
            "o registro de grants emitidos não respondeu; o acesso é negado (falha fechada)",
            true,
            "repita com backoff; se persistir, verifique a dependência Postgres do nó",
        ),
        ApiError::InvalidRequest(_) => (
            "o pedido foi recusado na fronteira (corpo, query ou cabeçalho inválido)",
            false,
            "corrija o pedido conforme detail; contratos aceitos estão em GET /v1/meta",
        ),
        ApiError::GrantDenied(_) => (
            "o predicado do grant negou a operação (não é Allow)",
            false,
            "a decisão está no grant, não na rota: peça um grant cujo predicado cubra o recurso",
        ),
        ApiError::RetentionPolicyMissing => (
            "a classe de retenção não tem política com cópia primária na jurisdição do tenant",
            false,
            "registre a política para a jurisdição antes de reenviar; classes válidas em GET /v1/meta → capabilities.retention_classes",
        ),
        ApiError::EventNamespaceReserved => (
            "o namespace está reservado para replicação de metadados em todos os epochs",
            false,
            "use um namespace não reservado para escritas por esta rota",
        ),
        ApiError::SequenceGap { .. } => (
            "a posição enviada salta a lacuna esperada da stream",
            true,
            "reenvie a partir de expected (no corpo) na mesma stream; a lacuna nunca é pulada",
        ),
        ApiError::InboxGap { .. } => (
            "um lote do Inbox saltou posição; o prefixo contíguo antes da lacuna ficou durável",
            true,
            "reenvie apenas de expected em diante; accepted e contiguous_prefix no corpo dizem o que já ficou",
        ),
        ApiError::GrantNotFound => (
            "token_id não corresponde a nenhum grant emitido por esta API",
            false,
            "confira o token_id em GET /v1/admin/grants/{token_id} ou emita um grant novo",
        ),
        ApiError::RandomnessUnavailable(_) => (
            "o RNG do sistema não respondeu; nenhuma credencial foi emitida",
            true,
            "repita a emissão; nada foi gravado nesta resposta",
        ),
        ApiError::CursorInvalid(_) => (
            "cursor ilegível, corrompido ou de versão desconhecida",
            false,
            "comece a leitura sem cursor e reuse o next_cursor devolvido",
        ),
        ApiError::CursorIncomparable(_) => (
            "cursor de outra partição, epoch, tenant ou filtro",
            false,
            "cursor não é traduzido: refaça a consulta com o filtro original ou comece sem cursor",
        ),
        ApiError::RetentionScopeUnresolved => (
            "o tenant não tem jurisdição declarada em tenant_region",
            false,
            "declare a jurisdição do tenant (operação do operador do nó) antes de reenviar",
        ),
        ApiError::IncidentIdConflict => (
            "o incident_id já pertence a outro tenant",
            false,
            "gere um identificador novo; a colisão é global por design e nada foi gravado",
        ),
        ApiError::IncidentNotFound => (
            "incidente inexistente, ou de outro tenant (a resposta é a mesma de propósito)",
            false,
            "verifique o identificador; não há como distinguir os dois pelo contrato",
        ),
        ApiError::IllegalTransition { .. } => (
            "a transição pedida não vale a partir do estado atual (state no corpo)",
            false,
            "leia GET /v1/incidents/{id} e siga o ciclo: replicate → ack-technical → acknowledge → close",
        ),
        ApiError::MissingHumanProof => (
            "acknowledged_human exige corpo com responder (prova de pessoa)",
            false,
            "reenvie com {\"responder\": \"<pessoa>\"}; sem pessoa identificada a transição não existe",
        ),
        ApiError::Invariant(_) => (
            "o contrato do incidente recusou a transição (nunca deveria acontecer)",
            false,
            "reporte ao operador do nó com o incident_id; nada foi gravado",
        ),
        ApiError::InvalidManifestSignature => (
            "a assinatura do manifesto não confere com a chave pública declarada",
            false,
            "assine o manifesto canônico com a chave privada do nó e reenvie; nada foi gravado",
        ),
        ApiError::ManifestExpired => (
            "issued_at do manifesto está fora da janela de frescor do nó",
            true,
            "gere um manifesto novo (mesma chave, mesmo epoch) e reenvie",
        ),
        ApiError::ManifestNotYetValid => (
            "issued_at do manifesto está no futuro além da tolerância",
            true,
            "alinhe o relógio do nó emissor (NTP) e reenvie com manifesto novo",
        ),
        ApiError::InvalidManifest(_) => (
            "manifesto malformado, com campo proibido ou internamente incoerente",
            false,
            "o corpo aceito é o schema SignedManifest de /openapi.json; deny_unknown_fields vale aqui",
        ),
        ApiError::ApprovalSignatureInvalid(_) => (
            "o certificado de aprovação não prova nada: assinatura, campo ou node_id do aprovador não conferem",
            false,
            "reassine o ApprovalCertificate com a chave privada do nó aprovador, cobrindo este join_request_id; nenhuma decisão foi alterada",
        ),
        ApiError::ApproverNotAuthorized(_) => (
            "quem assinou não é um nó autorizado no cluster do pedido (par revoked/pending, chave divergente da registrada ou fora do genesis)",
            false,
            "só nó authorized aprova: ver GET /v1/federation/peers e GODMESH_FEDERATION_GENESIS; nenhuma decisão foi alterada",
        ),
        ApiError::ApprovalExpired { .. } => (
            "o certificado de aprovação tem expires_at no passado (replay de decisão velha)",
            false,
            "emita um certificado novo com janela vigente e epoch não regressivo; o pedido continua pendente",
        ),
        ApiError::ApprovalNotYetValid { .. } => (
            "o certificado de aprovação tem not_before no futuro",
            true,
            "aguarde not_before (ou corrija o relógio do nó aprovador) e repita; nada foi alterado",
        ),
        ApiError::ApprovalEpochRegressed { .. } => (
            "o epoch do certificado é menor que o piso já assinado por este nó (anti-rollback)",
            false,
            "assine com a geração atual da identidade do nó; decisão velha não reescreve decisão nova",
        ),
        ApiError::FederationGovernanceDisabled => (
            "governança local de ingresso desabilitada para o cluster do pedido",
            false,
            "confira GODMESH_FEDERATION_GOVERNANCE no servidor",
        ),
        ApiError::NoAuthorizedApprover => (
            "não existe nenhum nó autorizado no cluster: sem genesis declarado e sem par authorized, aprovar é impossível",
            false,
            "declare GODMESH_FEDERATION_GENESIS com node_id:chave_pública dos primeiros nós (ADR-071 emenda §5); não há confiança por primeiro uso",
        ),
        ApiError::JoinRequestNotFound => (
            "join_request_id inexistente",
            false,
            "liste pedidos em GET /v1/federation/join-requests",
        ),
        ApiError::JoinRequestStateConflict { .. } => (
            "o pedido já foi decidido de outro modo (state no corpo)",
            false,
            "decisões de ingresso são terminais; nada foi alterado",
        ),
        ApiError::TrustTransition { .. } => (
            "a máquina de estados de confiança recusou a transição",
            false,
            "revogado não volta direto a autorizado: registre nova identidade (epoch novo)",
        ),
        ApiError::FederationInvariant(_) => (
            "estado persistido de federação fora do contrato (invariante de banco)",
            false,
            "reporte ao operador do nó; nada foi assumido como sadio",
        ),
        ApiError::EventSignatureInvalid(_) => (
            "a assinatura do envelope não confere com a chave registrada do nó, ou o registro não bate com o node_id",
            false,
            "assine os bytes canônicos de godmesh.federation.event.v1 com a chave do nó (GODMESH_NODE_KEY) e reenvie; nada foi gravado",
        ),
        ApiError::EventNodeUnauthorized(_) => (
            "o nó assinante não está registrado em federation_peer ou não está authorized (pendente/revogado/desconhecido)",
            false,
            "o nó precisa completar ingresso e aprovação (POST /v1/federation/join-requests); revogado não volta: nova identidade, novo epoch",
        ),
        ApiError::ConfigFenceBlocked(_) => (
            "a configuração do cluster bloqueia a escrita federada",
            true,
            "verifique a configuração efetiva do nó e repita após a recuperação",
        ),
        ApiError::EventScopeDenied(_) => (
            "fora do escopo concedido: origem diferente do assinante, tenant diferente do grant ou tupla (node, tenant, stream) sem linha em federation_peer_scope",
            false,
            "peça ao operador a concessão de escopo exata (federation_peer_scope) para o par (tenant, stream); ausência de linha é negativa por design",
        ),
        ApiError::EventNotFresh(_) => (
            "issued_at do envelope fora da janela de frescor do ingest (velho ou do futuro além da tolerância)",
            true,
            "alinhe o relógio (NTP) e reenvie o mesmo conteúdo com issued_at atual, assinado de novo; replay do envelope velho é recusado",
        ),
        ApiError::EventEpochStale { .. } => (
            "authority_epoch do envelope é menor que o maior epoch já durável no namespace (fencing ADR-061)",
            false,
            "não reenvie sob epoch antigo: sincronize com a autoridade atual (current no corpo) e emita sob ela; quarentena de epoch incomparável é S-sec-3",
        ),
        ApiError::Storage(_) => (
            "falha de persistência; nada foi confirmado",
            true,
            "repita o pedido com backoff; a operação é idempotente quando aceita Idempotency-Key",
        ),
    }
}

/// Uma instância representativa por variante. A **lista** é o piso manual do
/// catálogo; a cobertura por variante é garantida assim: `advice` não compila
/// com variante nova, e `catalogo_cobre_todo_advice` (unitário) exige que cada
/// variante desta lista tenha código distinto anunciado — rota documentada com
/// código fora do catálogo falha no teste de contrato.
fn probes() -> Vec<ApiError> {
        use godmesh_emergency::incident::IncidentError;
        use godmesh_identity::grant::GrantDecision;
        vec![
            ApiError::Unauthenticated,
            ApiError::InvalidCredential(crate::auth::CredentialError::EmptyTenant),
            ApiError::CredentialExpired,
            ApiError::CredentialNotYetValid,
            ApiError::CredentialRevoked,
            ApiError::InsufficientScope,
            ApiError::RevocationUnavailable,
            ApiError::IssuanceUnavailable,
            ApiError::InvalidRequest("sonda do catálogo".to_owned()),
            ApiError::GrantDenied(GrantDecision::DenyNotEnumerated),
            ApiError::RetentionPolicyMissing,
            ApiError::EventNamespaceReserved,
            ApiError::SequenceGap {
                expected: 1,
                got: 2,
            },
            ApiError::InboxGap {
                expected: 1,
                received: 2,
                accepted: 0,
                duplicates: 0,
                contiguous_prefix: 0,
            },
            ApiError::GrantNotFound,
            ApiError::RandomnessUnavailable("sonda".to_owned()),
            ApiError::CursorInvalid("sonda".to_owned()),
            ApiError::CursorIncomparable("sonda".to_owned()),
            ApiError::RetentionScopeUnresolved,
            ApiError::IncidentIdConflict,
            ApiError::IncidentNotFound,
            ApiError::IllegalTransition {
                current: "closed".to_owned(),
            },
            ApiError::MissingHumanProof,
            ApiError::Invariant(IncidentError::MissingTimestamp),
            ApiError::InvalidManifestSignature,
            ApiError::ManifestExpired,
            ApiError::ManifestNotYetValid,
            ApiError::InvalidManifest("sonda".to_owned()),
            ApiError::ApprovalSignatureInvalid("sonda".to_owned()),
            ApiError::ApproverNotAuthorized("sonda".to_owned()),
            ApiError::ApprovalExpired {
                expires_at: 1,
                now: 2,
            },
            ApiError::ApprovalNotYetValid {
                not_before: 2,
                now: 1,
            },
            ApiError::ApprovalEpochRegressed {
                recorded: 3,
                presented: 2,
            },
            ApiError::NoAuthorizedApprover,
            ApiError::FederationGovernanceDisabled,
            ApiError::JoinRequestNotFound,
            ApiError::PortableJoin(Join::NotFound),
            ApiError::PortableJoin(Join::Proof("malformed_frame")),
            ApiError::PortableJoin(Join::Issuer),
            ApiError::PortableJoin(Join::Validity),
            ApiError::PortableJoin(Join::Revoked),
            ApiError::PortableJoin(Join::Revocation),
            ApiError::PortableJoin(Join::Capacity),
            ApiError::PortableJoin(Join::Replacement),
            ApiError::PortableJoin(Join::RevocationConflict),
            ApiError::JoinRequestStateConflict {
                state: "approved".to_owned(),
            },
            ApiError::TrustTransition {
                current: "revoked".to_owned(),
                next: "authorized".to_owned(),
            },
            ApiError::FederationInvariant("sonda".to_owned()),
            ApiError::EventSignatureInvalid("sonda".to_owned()),
            ApiError::EventNodeUnauthorized("sonda".to_owned()),
            ApiError::ConfigFenceBlocked("sonda".to_owned()),
            ApiError::EventScopeDenied("sonda".to_owned()),
            ApiError::EventNotFresh("sonda".to_owned()),
            ApiError::EventEpochStale {
                current: 8,
                got: 7,
            },
            ApiError::Storage(sqlx::Error::RowNotFound),
        ]
}

/// O formato do erro como a **API devolve**: (status, código estável, detail).
/// É a fonte única do transporte; `advice` é a fonte única da explicação.
/// Os dois cobrem o enum inteiro por `match` exaustivo — sem wildcard.
#[must_use]
pub(crate) fn error_parts(e: &ApiError) -> (StatusCode, &'static str, String) {
    match e {
        ApiError::PortableJoin(error) => {
            let (status, code, detail) = portable_error(error);
            (status, code, detail.to_owned())
        }
        // 401 com `WWW-Authenticate` é o formato esperado por cliente HTTP.
        ApiError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "credencial ausente ou desconhecida".to_owned(),
        ),
        ApiError::InvalidCredential(inner) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_credential",
            inner.to_string(),
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
        ApiError::EventNamespaceReserved => (
            StatusCode::CONFLICT,
            "event_namespace_reserved",
            "o namespace da posição está reservado para replicação de metadados".to_owned(),
        ),
        ApiError::SequenceGap { expected, got } => (
            StatusCode::CONFLICT,
            "sequence_gap",
            format!("posição {got} salta a lacuna: a stream espera {expected}"),
        ),
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
        // outro tenant; existência de incidente alheio não é dado público.
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
        ApiError::InvalidManifestSignature => (
            StatusCode::UNAUTHORIZED,
            "invalid_manifest_signature",
            "assinatura do manifesto não confere".to_owned(),
        ),
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
        ApiError::ApprovalSignatureInvalid(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "approval_signature_invalid",
            format!("certificado de aprovação inválido: {detail}"),
        ),
        ApiError::ApproverNotAuthorized(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "approver_not_authorized",
            format!("aprovador não é nó autorizado: {detail}"),
        ),
        ApiError::ApprovalExpired { expires_at, now } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "approval_expired",
            format!("certificado de aprovação expirado: expires_at={expires_at}, now={now}"),
        ),
        ApiError::ApprovalNotYetValid { not_before, now } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "approval_not_yet_valid",
            format!("certificado de aprovação ainda não válido: not_before={not_before}, now={now}"),
        ),
        ApiError::ApprovalEpochRegressed { recorded, presented } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "approval_epoch_regressed",
            format!(
                "epoch do certificado ({presented}) é regressivo; este nó já assinou epoch {recorded}"
            ),
        ),
        ApiError::FederationGovernanceDisabled => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "federation_governance_disabled",
            "governança local de ingresso desabilitada para o cluster do pedido".to_owned(),
        ),
        ApiError::NoAuthorizedApprover => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "no_authorized_approver",
            "nenhum nó autorizado no cluster: sem genesis declarado e sem par authorized, aprovar é impossível".to_owned(),
        ),
        ApiError::JoinRequestNotFound => (
            StatusCode::NOT_FOUND,
            "join_request_not_found",
            "pedido de ingresso não encontrado".to_owned(),
        ),
        ApiError::JoinRequestStateConflict { state } => (
            StatusCode::CONFLICT,
            "join_request_state_conflict",
            format!("pedido de ingresso em estado {state}"),
        ),
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
        // --- S-sec-2: negações do envelope assinado. Todas 422: a credencial
        // é válida e a rota existe; o que falha é a prova de autoria do que o
        // corpo afirma. Nada foi gravado em nenhuma delas.
        ApiError::EventSignatureInvalid(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "event_signature_invalid",
            detail.clone(),
        ),
        ApiError::EventNodeUnauthorized(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "event_node_unauthorized",
            detail.clone(),
        ),
        ApiError::ConfigFenceBlocked(detail) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "config_fence_blocked",
            detail.clone(),
        ),
        ApiError::EventScopeDenied(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "event_scope_denied",
            detail.clone(),
        ),
        ApiError::EventNotFresh(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "event_not_fresh",
            detail.clone(),
        ),
        ApiError::EventEpochStale { current, got } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "event_epoch_stale",
            format!("epoch {got} é regressivo: a stream já reconheceu a autoridade {current}"),
        ),
        ApiError::Storage(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            "falha de persistência".to_owned(),
        ),
    }
}

/// Catálogo completo, estável por construção (uma vez, ordenado por código).
pub(crate) fn error_catalog() -> &'static [ErrorSpec] {
    static CAT: OnceLock<Vec<ErrorSpec>> = OnceLock::new();
    CAT.get_or_init(|| {
        let mut specs: Vec<ErrorSpec> = probes()
            .into_iter()
            .map(|probe| {
                let (status, code, _) = error_parts(&probe);
                let (when, retryable, hint) = advice(&probe);
                ErrorSpec {
                    error: code,
                    http_status: u16::from(status),
                    when,
                    retryable,
                    hint,
                }
            })
            .collect();
        // `sequence_gap` aparece duas vezes (stream e Inbox) com o mesmo
        // código; o catálogo publica a entrada uma única vez.
        specs.sort_by_key(|s| (s.error, s.http_status));
        specs.dedup_by_key(|s| s.error.to_owned());
        specs
    })
    .as_slice()
}

pub(crate) fn error_by_code(code: &str) -> Option<&'static ErrorSpec> {
    error_catalog().iter().find(|spec| spec.error == code)
}

/// Corpo JSON de erro como o contrato publica (usado em exemplos do
/// openapi.json e no corpo real via `ApiErrorResponse`).
pub(crate) fn error_example(code: &str) -> Option<Value> {
    let spec = error_by_code(code)?;
    Some(json!({
        "error": spec.error,
        "detail": "<texto específico do pedido>",
        "hint": spec.hint,
        "docs_url": "/v1/meta",
        "retryable": spec.retryable,
        "ack_class": "transport_accepted",
    }))
}

// ---------------------------------------------------------------------------
// GET /v1/meta — contrato legível por máquina, público, sem dado de tenant.
// ---------------------------------------------------------------------------

/// Aplica os tetos de corpo **efetivos** ao registro antes de ele virar
/// router/meta/openapi. Quem aplica o teto em runtime é o portão de abuso
/// (`http_gate`), com o valor resolvido de `AdmissionConfig` por classe de
/// corpo (`max_body_bytes`, `max_event_body_bytes`, `max_inbox_body_bytes`);
/// o que `GET /v1/meta` publica é exatamente esse valor efetivo — declarar
/// limite que o servidor não aplica seria mentira contratual. Roda na
/// composição, exatamente uma vez por processo.
pub(crate) fn apply_effective_limits(specs: &mut [RouteSpec], config: &AdmissionConfig) {
    for spec in specs {
        let Some(policy) = route_class(spec.method, spec.path) else {
            spec.max_body_bytes = None;
            continue;
        };
        // `BodyKind::None` = a rota não aceita corpo: `null`, não "zero bytes".
        spec.max_body_bytes = match policy.body_kind {
            BodyKind::None => None,
            _ => Some(policy.body_limit(config) as u64),
        };
    }
}

/// Publicações de rota para o meta: formato exato pedido pela tarefa, com os
/// **limites efetivos** deste processo — corpo e taxa saem do mesmo
/// `AdmissionConfig` que os portões aplicam, nunca de uma segunda tabela.
/// `rate: null` só aparece onde não há cota (rotas isentas por design:
/// `/healthz`, `/v1/meta`, `/openapi.json`); onde há cota, o número real.
fn route_limits_json(spec: &RouteSpec, config: &AdmissionConfig) -> Value {
    // Fora da tabela do portão = sem política = sem limite aplicado. Dizer
    // `null` aqui é o piso honesto: o servidor não limita o que não classifica.
    let Some(policy) = route_class(spec.method, spec.path) else {
        return json!({ "max_body_bytes": null, "rate": null });
    };
    // Teto efetivo já resolvido na composição (`apply_effective_limits`):
    // `null` quando a rota não aceita corpo.
    let max_body_bytes = spec.max_body_bytes.map(Value::from).unwrap_or(Value::Null);
    // Bucket efetivo: rota pública (sem credencial) é medida **por IP** pelo
    // `http_gate`; rota de escopo é medida por (tenant, token, classe) pelo
    // `grant_gate`. Publicar o bucket errado seria prometer a dimensionalidade
    // errada da cota.
    let (bucket, limits) = match spec.access {
        Access::Public => ("ip", config.ip_limits),
        Access::Scope(_) => ("grant", config.limits_for(policy.class)),
    };
    json!({
        "max_body_bytes": max_body_bytes,
        "rate": {
            "bucket": bucket,
            "class": policy.class.as_str(),
            "requests_per_sec": limits.requests_per_sec,
            "request_burst": limits.request_burst,
            "events_per_sec": limits.events_per_sec,
            "event_burst": limits.event_burst,
        },
    })
}

pub(crate) fn meta_routes_json(specs: &[RouteSpec], config: &AdmissionConfig) -> Value {
    Value::Array(
        specs
            .iter()
            .map(|spec| {
                json!({
                    "method": spec.method,
                    "path": spec.path,
                    "scope": spec.scope_name(),
                    "mutating": spec.mutating,
                    "destructive": spec.destructive,
                    "limits": route_limits_json(spec, config),
                })
            })
            .collect(),
    )
}

pub(crate) fn meta_errors_json() -> Value {
    Value::Array(
        error_catalog()
            .iter()
            .map(|spec| {
                json!({
                    "error": spec.error,
                    "http_status": spec.http_status,
                    "when": spec.when,
                    "retryable": spec.retryable,
                    "hint": spec.hint,
                })
            })
            .collect(),
    )
}

/// Estado efetivo do nó, não configuração desejada. O que o servidor não sabe
/// aparece como `null` (o piso honesto de `unknown` da ADR-072), nunca como
/// lista inventada.
pub(crate) async fn capabilities(state: &AppState) -> Value {
    let retention_classes: Option<Vec<String>> = match tokio::time::timeout(
        std::time::Duration::from_millis(500),
        sqlx::query_scalar::<_, String>("SELECT DISTINCT class FROM retention_class ORDER BY class")
            .fetch_all(&state.pool),
    )
    .await
    {
        Ok(Ok(classes)) => Some(classes),
        _ => None,
    };
    json!({
        "federation_enabled": state.federation.is_some(),
        // Estado efetivo do bootstrap de confiança (ADR-071 emenda §5): quantos
        // nós o operador declarou como aprovadores antes de existir par
        // autorizado. `0` não é "não medido" — é "sem genesis", e então aprovar
        // exige um par já autorizado.
        "federation_genesis_nodes": state.decisions.genesis_count(),
        "retention_classes": retention_classes,
        "outbox_enabled": state.outbox_enabled,
        "host_metrics_available": state.host_metrics_available,
    })
}

pub(crate) async fn get_meta(State(state): State<AppState>) -> Response {
    let capabilities = capabilities(&state).await;
    let body = json!({
        "api_version": API_VERSION,
        "supported_versions": SUPPORTED_VERSIONS,
        "compatibility_policy": COMPATIBILITY_POLICY,
        "routes": meta_routes_json(&state.route_specs, state.admission.config()),
        "errors": meta_errors_json(),
        "capabilities": capabilities,
        "openapi_url": "/openapi.json",
        "docs_url": "/openapi.json",
    });
    Json(body).into_response()
}

/// Esquema do corpo de `GET /v1/meta` para o documento OpenAPI (livre-form
/// object: o conteúdo é gerado em runtime, e a forma exata é provada pelo
/// teste de contrato contra o servidor real).
pub(crate) const META_SCHEMA_COMPONENT: &str = "MetaResponse";

// ---------------------------------------------------------------------------
// Testes unitários: o catálogo não pode divergir do `match` de transporte.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogo_tem_codigo_para_cada_sonda() {
        // Toda sonda precisa produzir um código conhecido; código vazio seria
        // catálogo mentindo.
        for probe in &probes() {
            let (_, code, _) = error_parts(probe);
            assert!(
                error_by_code(code).is_some(),
                "sonda sem entrada de catálogo: {code}"
            );
        }
    }

    #[test]
    fn catalogo_nao_tem_codigos_repetidos() {
        let cat = error_catalog();
        let mut seen: Vec<&str> = cat.iter().map(|s| s.error).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "catálogo com código duplicado");
    }

    #[test]
    fn dry_run_flag_eestrita() {
        assert!(matches!(query_flag("", "dry_run"), Flag::Absent));
        assert!(matches!(query_flag("dry_run=true", "dry_run"), Flag::True));
        assert!(matches!(query_flag("dry_run=false", "dry_run"), Flag::False));
        assert!(matches!(
            query_flag("a=1&dry_run=1&b=2", "dry_run"),
            Flag::Malformed
        ));
        assert!(matches!(
            query_flag("x=dry_run", "dry_run"),
            Flag::Absent
        ));
    }

    #[test]
    fn chave_de_idempotencia_valida_fronteira() {
        assert!(idempotency_key_is_valid(b"abc-123"));
        assert!(idempotency_key_is_valid(&[b'a'; MAX_IDEMPOTENCY_KEY_LEN]));
        assert!(!idempotency_key_is_valid(b""));
        assert!(!idempotency_key_is_valid(&[b'a'; MAX_IDEMPOTENCY_KEY_LEN + 1]));
        assert!(!idempotency_key_is_valid("com espaço".as_bytes()));
        assert!(!idempotency_key_is_valid("açento".as_bytes()));
    }
}
