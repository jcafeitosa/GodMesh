//! Cotas, limites de payload e admissão por classe — o portão de entrada contra abuso.
//!
//! Risco coberto (`docs/04-seguranca/abuso-e-malha-maligna.md` §4, item 1 e 2): um
//! grant válido hoje pode inundar o servidor, porque autorização não limita dano.
//! Este módulo declara e aplica três controles **antes** do handler:
//!
//! 1. **Limite de payload por rota** — corpo acima do teto declarado é recusado
//!    com 413 `payload_too_large` antes de qualquer desserialização ou INSERT.
//! 2. **Cotas por grant e por tenant** — token bucket por *(tenant, token, classe)*
//!    para requisições/s e, separadamente, eventos/s. Excedeu: 429 `rate_limited`
//!    com `Retry-After`. Requisições **sem** credencial são limitadas por IP de
//!    origem (quando o servidor tem `ConnectInfo`).
//! 3. **Admissão por classe** — sob pressão (requisições em voo acima do limiar
//!    configurado) as classes baixas (`event`, `sync`, `metrics`) recebem 503
//!    `admission_control` com `Retry-After`; `sos`, `ptt` e `incident` nunca são
//!    recusadas por pressão, e cada classe tem bucket próprio, logo o flood de
//!    uma classe **não consome** a cota de SOS.
//!
//! ## O que este controle NÃO é (declaração honesta)
//!
//! * O bucket vive **na memória do processo**. Com N réplicas atrás de load
//!   balancer cada uma aplica a sua cota — é controle de primeira linha, não
//!   garantia global. Cota distribuída exige estado compartilhado e não existe aqui.
//! * Nada foi calibrado por teste de carga real; os valores padrão são tetos
//!   declarados e documentados, ajustáveis por ambiente.
//! * O mapa de buckets tem teto de chaves; ao estourar, entradas ociosas são
//!   removidas (aproximação aceitável: perder um bucket só reinicia sua janela).
//!
//! A classe pode ser declarada pelo cliente via header `x-godmesh-class` (valores
//! `sos|ptt|incident|event|sync|metrics`). Sem header, a classe vem da tabela de
//! rotas. Isso é uma escolha rastreável: o antiabuso de SOS propriamente dito
//! (limite de aberturas por principal/janela, §4 item 3) é backlog separado, e
//! forjar classe SOS só compra prioridade de admissão, não cota de outra classe
//! nem qualquer escopo a mais — a autorização continua sendo o que o grant contém.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::auth::Scope;

/// Header que declara a classe de tráfego do pedido.
pub const CLASS_HEADER: &str = "x-godmesh-class";

/// Teto de chaves de bucket por processo; acima disso, ociosos são podados.
const MAX_BUCKETS: usize = 8_192;

/// Tempo sem uso a partir do qual um bucket é candidato à poda.
const IDLE_PRUNE: Duration = Duration::from_secs(300);

/// Classes de tráfego, da mais privilegiada à menos. A ordem é o invariante:
/// sob pressão, quem cai é a cauda, nunca a emergência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestClass {
    /// Emergência: nunca recusada por pressão nem pela cota de outra classe.
    Sos,
    /// Push-to-talk: mesma proteção de admissão do SOS.
    Ptt,
    /// Incidente (e operações de admin/federação, que precisam continuar
    /// funcionando sob pressão para *estancar* o abuso).
    Incident,
    /// Escrita/leitura de posições de evento.
    Event,
    /// Sincronização (inbox de posições, estado do prefixo).
    Sync,
    /// Métricas operacionais.
    Metrics,
}

impl RequestClass {
    /// Posição na hierarquia (0 = mais privilegiada).
    pub const fn rank(self) -> u8 {
        match self {
            Self::Sos => 0,
            Self::Ptt => 1,
            Self::Incident => 2,
            Self::Event => 3,
            Self::Sync => 4,
            Self::Metrics => 5,
        }
    }

    /// Nome estável para header, log e documentação.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sos => "sos",
            Self::Ptt => "ptt",
            Self::Incident => "incident",
            Self::Event => "event",
            Self::Sync => "sync",
            Self::Metrics => "metrics",
        }
    }

    /// Recusa sob pressão? Só classes a partir de `event`; SOS/PTT/incidente
    /// seguem sempre. É o invariante §4 item 2.
    pub const fn sheds_under_pressure(self) -> bool {
        self.rank() >= Self::Event.rank()
    }

    /// Escopo cujo parâmetro de cota governa esta classe. A cota é declarada
    /// **por escopo**; a classe resolve em qual escopo ela cai.
    pub const fn limits_scope(self) -> Scope {
        match self {
            Self::Sos | Self::Ptt | Self::Incident => Scope::IncidentWrite,
            Self::Event => Scope::EventWrite,
            Self::Sync => Scope::SyncWrite,
            Self::Metrics => Scope::AdminMetrics,
        }
    }

    /// Parse do header do cliente. Valor desconhecido é recusado (422), nunca
    /// reinterpretado como outra classe.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sos" => Some(Self::Sos),
            "ptt" => Some(Self::Ptt),
            "incident" => Some(Self::Incident),
            "event" => Some(Self::Event),
            "sync" => Some(Self::Sync),
            "metrics" => Some(Self::Metrics),
            _ => None,
        }
    }
}

/// A qual teto de corpo uma rota obedece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// Sem corpo aplicável (GET e afins) — nada a limitar.
    None,
    /// Teto geral [`AdmissionConfig::max_body_bytes`].
    Default,
    /// Teto de escrita de evento [`AdmissionConfig::max_event_body_bytes`].
    Event,
    /// Teto de lote de inbox [`AdmissionConfig::max_inbox_body_bytes`].
    Inbox,
}

/// Política efetiva de uma rota: classe, teto de corpo e timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutePolicy {
    pub class: RequestClass,
    pub body_kind: BodyKind,
}

/// Tabela estática de rotas → classe. A guarda é **antes** do handler e por isso
/// precisa classificar sem executar nada do domínio. Rotas fora da tabela (ex.:
/// `/healthz`, 404) ficam isentas de cota: limitar o healthcheck de um LB seria
/// tiro no pé, e 404 não tem efeito a limitar.
///
/// `admin` e a **decisão** de federação entram na classe `incident`: são as
/// operações que precisam continuar acessíveis sob pressão para revogar
/// exatamente o grant que inunda. Não existe classe `admin` na taxonomia
/// declarada (`sos > ptt > incident > event > sync > metrics`); `incident` é o
/// degrau mais alto que não é emergência, e é o que o contrato de SEC1 usa para
/// tráfego administrativo.
///
/// A exceção é `POST /v1/federation/join-requests`, que é **pública**
/// (sem credencial): dar-lhe prioridade de incidente seria dizer que o único
/// endpoint anônimo do servidor nunca pode ser recusado por pressão — exatamente
/// o vetor que um nó malicioso usa para inundar o ingresso. Ele entra em `sync`,
/// a classe mais baixa que ainda descreve troca com o mundo de fora, e por isso
/// sua cota por IP tem bucket próprio, separado das rotas administrativas. O
/// teto de corpo continua o geral (`manifesto` cabe no `max_body_bytes`).
#[must_use]
pub fn route_class(method: &str, path: &str) -> Option<RoutePolicy> {
    let policy = |class: RequestClass, body_kind: BodyKind| Some(RoutePolicy { class, body_kind });
    if path == "/metrics" {
        return policy(RequestClass::Metrics, BodyKind::None);
    }
    if path == "/v1/events" {
        // POST escreve evento (teto próprio); GET lê (sem corpo).
        return if method == "POST" {
            policy(RequestClass::Event, BodyKind::Event)
        } else {
            policy(RequestClass::Event, BodyKind::None)
        };
    }
    if path == "/v1/sync/inbox" {
        return policy(RequestClass::Sync, BodyKind::Inbox);
    }
    if path.starts_with("/v1/sync") {
        return policy(RequestClass::Sync, BodyKind::None);
    }
    if path.starts_with("/v1/incidents") {
        return policy(RequestClass::Incident, BodyKind::Default);
    }
    if path.starts_with("/v1/admin") {
        return policy(RequestClass::Incident, BodyKind::Default);
    }
    if path == "/v1/federation/join-requests" && method == "POST" {
        // Rota pública de ingresso: anônima, logo sem prioridade administrativa.
        return policy(RequestClass::Sync, BodyKind::Default);
    }
    if path.starts_with("/v1/federation") {
        // Ler pedidos/pares e aprovar/rejeitar: administração sob pressão.
        return policy(RequestClass::Incident, BodyKind::Default);
    }
    None
}

impl RoutePolicy {
    /// Teto de corpo efetivo desta rota na configuração dada.
    #[must_use]
    pub fn body_limit(&self, config: &AdmissionConfig) -> usize {
        match self.body_kind {
            BodyKind::None => 0,
            BodyKind::Default => config.max_body_bytes,
            BodyKind::Event => config.max_event_body_bytes,
            BodyKind::Inbox => config.max_inbox_body_bytes,
        }
    }

    /// Timeout efetivo desta rota. Hoje todas as rotas com política partilham o
    /// teto declarado `GODMESH_REQUEST_TIMEOUT_MS`; a resolução é **por rota**
    /// porque a tabela pode dar a cada família um valor próprio sem tocar nos
    /// portões.
    #[must_use]
    pub const fn timeout(&self, config: &AdmissionConfig) -> Duration {
        config.request_timeout
    }
}

/// Cota de um escopo: requisições/s e eventos/s, com burst explícito.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScopeLimits {
    pub requests_per_sec: f64,
    pub request_burst: f64,
    pub events_per_sec: f64,
    pub event_burst: f64,
}

/// Configuração declarada dos limites. Os valores padrão são altos de propósito:
/// a rajada curta dos testes de E2E existentes não pode ser barrada; baixar por
/// escopo é o caminho esperado quando se quer cota agressiva.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionConfig {
    /// Teto geral de corpo (rotas de escrita sem teto próprio). Default 64 KiB.
    pub max_body_bytes: usize,
    /// Teto de `POST /v1/events`. Default 64 KiB.
    pub max_event_body_bytes: usize,
    /// Teto de `POST /v1/sync/inbox` (lote de até 100 posições). Default 256 KiB.
    pub max_inbox_body_bytes: usize,
    /// Timeout de requisição aplicado às rotas com política. Default 10 s.
    pub request_timeout: Duration,
    /// Acima deste número de requisições em voo, classes baixas são recusadas.
    /// Default 64. `0` = sempre sob pressão (usado por teste).
    pub pressure_threshold: u32,
    /// Cota padrão para todos os escopos, salvo override.
    pub default_limits: ScopeLimits,
    /// Overrides por escopo (`GODMESH_LIMIT_EVENT_WRITE_RPS` & c.).
    pub per_scope: HashMap<Scope, ScopeLimits>,
    /// Cota para requisições **sem** credencial, por IP de origem.
    pub ip_limits: ScopeLimits,
}

/// Padrões declarados e documentados (valores altos; ver docs do módulo).
impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 64 * 1024,
            max_event_body_bytes: 64 * 1024,
            max_inbox_body_bytes: 256 * 1024,
            request_timeout: Duration::from_millis(10_000),
            pressure_threshold: 64,
            default_limits: ScopeLimits {
                requests_per_sec: 1_000.0,
                request_burst: 1_000.0,
                events_per_sec: 1_000.0,
                event_burst: 1_000.0,
            },
            per_scope: HashMap::new(),
            ip_limits: ScopeLimits {
                requests_per_sec: 500.0,
                request_burst: 1_000.0,
                events_per_sec: 500.0,
                event_burst: 1_000.0,
            },
        }
    }
}

impl AdmissionConfig {
    /// Maior teto de corpo configurado entre todas as famílias de rota.
    ///
    /// É o valor com que o teto interno do extrator do axum (2 MiB por padrão,
    /// `DefaultBodyLimit`) precisa ser alinhado: acima dele o axum recusaria
    /// corpo sozinho, com resposta fora do contrato da fronteira. `max(1)`
    /// porque `DefaultBodyLimit::max(0)` recusaria qualquer corpo lido por
    /// extrator, inclusive onde o portão já mediu.
    #[must_use]
    pub fn payload_ceiling(&self) -> usize {
        self.max_body_bytes
            .max(self.max_event_body_bytes)
            .max(self.max_inbox_body_bytes)
            .max(1)
    }

    /// Nomes de ambiente, defaults e validação em uma só tabela (fonte única).
    const ENV_MAX_BODY: &'static str = "GODMESH_MAX_BODY_BYTES";
    const ENV_MAX_EVENT_BODY: &'static str = "GODMESH_MAX_EVENT_BODY_BYTES";
    const ENV_MAX_INBOX_BODY: &'static str = "GODMESH_MAX_INBOX_BODY_BYTES";
    const ENV_TIMEOUT_MS: &'static str = "GODMESH_REQUEST_TIMEOUT_MS";
    const ENV_PRESSURE: &'static str = "GODMESH_ADMISSION_PRESSURE_THRESHOLD";
    const ENV_DEFAULT_RPS: &'static str = "GODMESH_LIMIT_REQUESTS_PER_SEC";
    const ENV_DEFAULT_RBURST: &'static str = "GODMESH_LIMIT_REQUEST_BURST";
    const ENV_DEFAULT_EPS: &'static str = "GODMESH_LIMIT_EVENTS_PER_SEC";
    const ENV_DEFAULT_EBURST: &'static str = "GODMESH_LIMIT_EVENT_BURST";
    const ENV_IP_RPS: &'static str = "GODMESH_LIMIT_IP_RPS";
    const ENV_IP_BURST: &'static str = "GODMESH_LIMIT_IP_BURST";

    /// Prefixo do override por escopo: `GODMESH_LIMIT_<ESCOPO>_<PARÂMETRO>`,
    /// ex.: `GODMESH_LIMIT_EVENT_WRITE_RPS`, `GODMESH_LIMIT_SYNC_WRITE_EPS`.
    fn scope_env(scope: Scope, param: &str) -> String {
        format!(
            "GODMESH_LIMIT_{}_{}",
            scope.as_str().to_ascii_uppercase().replace(':', "_"),
            param
        )
    }

    /// Lê o ambiente. Erro é mensagem legível citando a variável; nada é
    /// ignorado em silêncio — valor declarado inválido derruba a subida.
    ///
    /// # Errors
    /// Valor não numérico, zero ou negativo onde o contrato exige positivo.
    pub fn from_env() -> Result<Self, String> {
        Self::resolve(|name| std::env::var(name).ok())
    }

    /// Versão pura de [`Self::from_env`], testável sem ambiente global.
    ///
    /// # Errors
    /// Mesmas recusas de [`Self::from_env`].
    pub fn resolve(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let mut config = Self::default();
        config.max_body_bytes = parse_bytes(&get, Self::ENV_MAX_BODY, config.max_body_bytes)?;
        config.max_event_body_bytes =
            parse_bytes(&get, Self::ENV_MAX_EVENT_BODY, config.max_event_body_bytes)?;
        config.max_inbox_body_bytes =
            parse_bytes(&get, Self::ENV_MAX_INBOX_BODY, config.max_inbox_body_bytes)?;
        let timeout_ms =
            parse_positive_u64(&get, Self::ENV_TIMEOUT_MS, config.request_timeout.as_millis() as u64)?;
        config.request_timeout = Duration::from_millis(timeout_ms);
        // `0` é permitido aqui e significa "sempre sob pressão" — é como o teste
        // prova o comportamento de admissão sem concurrency artificial.
        config.pressure_threshold =
            parse_nonneg_u64(&get, Self::ENV_PRESSURE, u64::from(config.pressure_threshold))? as u32;

        let mut limits = config.default_limits;
        limits.requests_per_sec =
            parse_positive_f64(&get, Self::ENV_DEFAULT_RPS, limits.requests_per_sec)?;
        limits.request_burst =
            parse_positive_f64(&get, Self::ENV_DEFAULT_RBURST, limits.request_burst)?;
        limits.events_per_sec =
            parse_positive_f64(&get, Self::ENV_DEFAULT_EPS, limits.events_per_sec)?;
        limits.event_burst =
            parse_positive_f64(&get, Self::ENV_DEFAULT_EBURST, limits.event_burst)?;
        config.default_limits = limits;

        let mut ip = config.ip_limits;
        ip.requests_per_sec = parse_positive_f64(&get, Self::ENV_IP_RPS, ip.requests_per_sec)?;
        ip.request_burst = parse_positive_f64(&get, Self::ENV_IP_BURST, ip.request_burst)?;
        ip.events_per_sec = ip.requests_per_sec;
        ip.event_burst = ip.request_burst;
        config.ip_limits = ip;

        // Overrides por escopo: qualquer um dos quatro parâmetros presentes cria
        // o override, com os demais herdando o default já resolvido.
        for scope in Scope::ALL {
            let mut scope_limits = config.default_limits;
            let mut declared = false;
            for (param, slot) in [
                ("RPS", 0),
                ("RBUCKET", 1),
                ("EPS", 2),
                ("EBUCKET", 3),
            ] {
                let name = Self::scope_env(scope, param);
                if let Some(raw) = get(&name) {
                    declared = true;
                    let value = parse_f64_in(&name, &raw)?;
                    match slot {
                        0 => scope_limits.requests_per_sec = value,
                        1 => scope_limits.request_burst = value,
                        2 => scope_limits.events_per_sec = value,
                        _ => scope_limits.event_burst = value,
                    }
                }
            }
            if declared {
                config.per_scope.insert(scope, scope_limits);
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Cota efetiva de uma classe (via escopo que a governa).
    #[must_use]
    pub fn limits_for(&self, class: RequestClass) -> ScopeLimits {
        self.per_scope
            .get(&class.limits_scope())
            .copied()
            .unwrap_or(self.default_limits)
    }

    /// Recusa combinações sem sentido antes de servir a primeira requisição.
    ///
    /// # Errors
    /// Cota ou teto não positivo.
    fn validate(&self) -> Result<(), String> {
        if self.max_body_bytes == 0
            || self.max_event_body_bytes == 0
            || self.max_inbox_body_bytes == 0
        {
            return Err("tetos de corpo precisam ser maiores que zero".to_owned());
        }
        if self.request_timeout.is_zero() {
            return Err(format!("{} precisa ser maior que zero", Self::ENV_TIMEOUT_MS));
        }
        let named = [("default", self.default_limits), ("ip", self.ip_limits)];
        for (name, limits) in named {
            if limits.requests_per_sec <= 0.0 || limits.events_per_sec <= 0.0 {
                return Err(format!("cota do escopo {name} precisa ser maior que zero"));
            }
        }
        for (scope, limits) in &self.per_scope {
            if limits.requests_per_sec <= 0.0
                || limits.events_per_sec <= 0.0
                || limits.request_burst <= 0.0
                || limits.event_burst <= 0.0
            {
                return Err(format!(
                    "cota do escopo {} precisa ser maior que zero",
                    scope.as_str()
                ));
            }
        }
        Ok(())
    }
}

fn parse_f64_in(name: &str, raw: &str) -> Result<f64, String> {
    let value: f64 = raw
        .trim()
        .parse()
        .map_err(|_| format!("{name} precisa de número, veio {raw:?}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{name} precisa ser número positivo, veio {raw:?}"));
    }
    Ok(value)
}

fn parse_positive_f64(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: f64,
) -> Result<f64, String> {
    match get(name) {
        None => Ok(default),
        Some(raw) => parse_f64_in(name, &raw),
    }
}

fn parse_positive_u64(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: u64,
) -> Result<u64, String> {
    parse_nonneg_u64(get, name, default).and_then(|v| {
        if v == 0 {
            Err(format!("{name} precisa ser maior que zero"))
        } else {
            Ok(v)
        }
    })
}

fn parse_nonneg_u64(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: u64,
) -> Result<u64, String> {
    match get(name) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse()
            .map_err(|_| format!("{name} precisa de inteiro não negativo, veio {raw:?}")),
    }
}

/// Tetos de corpo aceitam `0` como "sem exigência"? Não: zero significaria
/// "nenhum corpo", o que quebraria até replay. Exigimos positivo.
fn parse_bytes(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: usize,
) -> Result<usize, String> {
    parse_positive_u64(get, name, default as u64).map(|v| v as usize)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BucketKind {
    Requests,
    Events,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BucketKey {
    tenant: String,
    token: String,
    class: RequestClass,
    kind: BucketKind,
}

/// Token bucket clássico: capacidade fixa, reabastecimento contínuo à taxa.
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

impl Bucket {
    /// Tenta consumir `cost`. Devolve `None` quando aceitou; `Some(secs)` quando
    /// recusou, com o `Retry-After` mínimo em segundos para ter `cost` tokens.
    fn take(&mut self, cost: f64, capacity: f64, rate: f64, now: Instant) -> Option<u64> {
        let elapsed = now.duration_since(self.last).as_secs_f64().max(0.0);
        self.last = now;
        self.tokens = (self.tokens + elapsed * rate).min(capacity);
        if self.tokens >= cost {
            self.tokens -= cost;
            return None;
        }
        let missing = cost - self.tokens;
        let secs = (missing / rate).ceil() as u64;
        Some(secs.max(1))
    }
}

/// Estado vivo do portão: buckets por chave + contador de requisições em voo.
///
/// **Por processo**, sem coordenação entre réplicas — ver docs do módulo.
#[derive(Debug)]
pub struct Admission {
    config: AdmissionConfig,
    buckets: Mutex<HashMap<BucketKey, Bucket>>,
    in_flight: Arc<AtomicU32>,
}

/// Resultado de uma checagem de cota: aceito, ou recusado com `Retry-After`.
pub type RateDecision = Result<(), u64>;

impl Admission {
    #[must_use]
    pub fn new(config: AdmissionConfig) -> Self {
        Self {
            config,
            buckets: Mutex::new(HashMap::new()),
            in_flight: Arc::new(AtomicU32::new(0)),
        }
    }

    #[must_use]
    pub fn config(&self) -> &AdmissionConfig {
        &self.config
    }

    /// Requisições em voo agora (pressão da fila de admissão).
    #[must_use]
    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Sob pressão? Compara o voo atual com o limiar declarado.
    #[must_use]
    pub fn under_pressure(&self) -> bool {
        self.in_flight() >= self.config.pressure_threshold
    }

    /// Entra em voo; o guard devolve o slot no drop (inclusive em cancelamento,
    /// porque o future inteiro é descartado junto com o guard).
    #[must_use]
    pub fn enter(&self) -> FlightGuard {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        FlightGuard(Arc::clone(&self.in_flight))
    }

    /// Cota de requisições do grant nesta classe.
    pub fn check_request(&self, tenant: &str, token: &str, class: RequestClass) -> RateDecision {
        let limits = self.config.limits_for(class);
        self.take(
            BucketKey {
                tenant: tenant.to_owned(),
                token: token.to_owned(),
                class,
                kind: BucketKind::Requests,
            },
            1.0,
            limits.request_burst,
            limits.requests_per_sec,
        )
    }

    /// Cota de eventos do grant nesta classe. `cost` é o número de posições de
    /// evento que o pedido carrega (1 para `POST /v1/events`, lote para inbox).
    /// Custo zero não toca no bucket.
    pub fn check_events(
        &self,
        tenant: &str,
        token: &str,
        class: RequestClass,
        cost: u64,
    ) -> RateDecision {
        if cost == 0 {
            return Ok(());
        }
        let limits = self.config.limits_for(class);
        self.take(
            BucketKey {
                tenant: tenant.to_owned(),
                token: token.to_owned(),
                class,
                kind: BucketKind::Events,
            },
            cost as f64,
            limits.event_burst,
            limits.events_per_sec,
        )
    }

    /// Cota por IP para requisições sem credencial (portão pré-autenticação).
    pub fn check_anonymous(&self, ip: &str, class: RequestClass) -> RateDecision {
        let limits = self.config.ip_limits;
        self.take(
            BucketKey {
                tenant: format!("ip:{ip}"),
                token: "-".to_owned(),
                class,
                kind: BucketKind::Requests,
            },
            1.0,
            limits.request_burst,
            limits.requests_per_sec,
        )
    }

    fn take(
        &self,
        key: BucketKey,
        cost: f64,
        capacity: f64,
        rate: f64,
    ) -> RateDecision {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let bucket = buckets.entry(key).or_insert(Bucket {
            tokens: capacity,
            last: now,
        });
        if let Some(retry) = bucket.take(cost, capacity, rate, now) {
            return Err(retry);
        }
        // Poda aproximada: o teto protege contra esgotamento de memória por
        // chaves novas; perder bucket ocioso só reinicia a janela daquela chave.
        if buckets.len() > MAX_BUCKETS {
            buckets.retain(|_, b| now.duration_since(b.last) < IDLE_PRUNE);
        }
        Ok(())
    }
}

/// Devolve o slot de voo no drop.
#[derive(Debug)]
pub struct FlightGuard(Arc<AtomicU32>);

impl Drop for FlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Custo em eventos de um lote de inbox: o número de posições declaradas.
///
/// Corpo ilegível custa zero aqui — quem recusa é o handler, com 422; a cota não
/// julga validade, só volume.
#[must_use]
pub fn inbox_event_cost(body: &[u8]) -> u64 {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("positions")
                .and_then(serde_json::Value::as_array)
                .map(|positions| positions.len() as u64)
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AdmissionConfig {
        AdmissionConfig::default()
    }

    #[test]
    fn teto_maior_configurado_alinha_o_extrator_do_axum() {
        // Default: o maior teto é o do inbox (256 KiB) — bem abaixo dos 2 MiB
        // silenciosos do framework, que portanto nunca contraria o portão.
        assert_eq!(AdmissionConfig::default().payload_ceiling(), 256 * 1024);
        // Operador acima de 2 MiB: o teto do extrator tem que acompanhar.
        let config = AdmissionConfig {
            max_event_body_bytes: 3 * 1024 * 1024,
            ..AdmissionConfig::default()
        };
        assert_eq!(config.payload_ceiling(), 3 * 1024 * 1024);
        // Teto zero em tudo não desliga o extrator para 0 (recusaria qualquer
        // corpo lido por extrator, inclusive o que o portão já mediu).
        let config = AdmissionConfig {
            max_body_bytes: 0,
            max_event_body_bytes: 0,
            max_inbox_body_bytes: 0,
            ..AdmissionConfig::default()
        };
        assert_eq!(config.payload_ceiling(), 1);
    }

    #[test]
    fn classes_are_ordered_sos_above_the_rest() {
        assert!(
            RequestClass::Sos.rank() < RequestClass::Ptt.rank()
                && RequestClass::Ptt.rank() < RequestClass::Incident.rank()
                && RequestClass::Incident.rank() < RequestClass::Event.rank()
                && RequestClass::Event.rank() < RequestClass::Sync.rank()
                && RequestClass::Sync.rank() < RequestClass::Metrics.rank()
        );
        assert!(!RequestClass::Sos.sheds_under_pressure());
        assert!(!RequestClass::Ptt.sheds_under_pressure());
        assert!(!RequestClass::Incident.sheds_under_pressure());
        assert!(RequestClass::Event.sheds_under_pressure());
        assert!(RequestClass::Sync.sheds_under_pressure());
        assert!(RequestClass::Metrics.sheds_under_pressure());
    }

    #[test]
    fn class_header_parses_declared_values_only() {
        assert_eq!(RequestClass::parse("SOS"), Some(RequestClass::Sos));
        assert_eq!(RequestClass::parse(" sync "), Some(RequestClass::Sync));
        assert_eq!(RequestClass::parse("banana"), None);
    }

    #[test]
    fn route_table_covers_every_protected_family() {
        assert_eq!(route_class("POST", "/v1/events").map(|p| p.class), Some(RequestClass::Event));
        assert_eq!(route_class("GET", "/v1/events").map(|p| p.body_kind), Some(BodyKind::None));
        assert_eq!(route_class("POST", "/v1/sync/inbox").map(|p| p.class), Some(RequestClass::Sync));
        assert_eq!(route_class("POST", "/v1/incidents").map(|p| p.class), Some(RequestClass::Incident));
        assert_eq!(route_class("GET", "/v1/incidents/x/ack-technical").map(|p| p.class), Some(RequestClass::Incident));
        assert_eq!(route_class("GET", "/metrics").map(|p| p.class), Some(RequestClass::Metrics));
        assert_eq!(route_class("GET", "/healthz").map(|p| p.class), None);
        assert_eq!(route_class("GET", "/nope").map(|p| p.class), None);
    }

    #[test]
    fn rota_publica_de_ingresso_e_sync_e_decisao_de_federacao_e_incidente() {
        // O único endpoint anônimo não herda prioridade administrativa.
        assert_eq!(
            route_class("POST", "/v1/federation/join-requests").map(|p| p.class),
            Some(RequestClass::Sync)
        );
        // Ler e decidir continuam acessíveis sob pressão (é como se estanca o
        // abuso), e o corpo do pedido segue o teto geral de payload.
        assert_eq!(
            route_class("GET", "/v1/federation/join-requests").map(|p| p.class),
            Some(RequestClass::Incident)
        );
        assert_eq!(
            route_class("POST", "/v1/federation/join-requests/jqr_1/approve")
                .map(|p| p.class),
            Some(RequestClass::Incident)
        );
        assert_eq!(
            route_class(
                "GET",
                "/v1/federation/peers/gmn_0000000000000000000000000000000000000000000000000000000000000000/link"
            )
            .map(|p| p.class),
            Some(RequestClass::Incident)
        );
        assert_eq!(
            route_class("POST", "/v1/federation/join-requests").map(|p| p.body_kind),
            Some(BodyKind::Default)
        );
    }

    #[test]
    fn bucket_refills_at_declared_rate_and_returns_retry_after() {
        // burst 2: duas aceita, a terceira recusa com Retry-After >= 1.
        let limits = ScopeLimits {
            requests_per_sec: 2.0,
            request_burst: 2.0,
            events_per_sec: 2.0,
            event_burst: 2.0,
        };
        let mut c = config();
        c.per_scope.insert(RequestClass::Event.limits_scope(), limits);
        let admission = Admission::new(c);
        assert!(admission.check_request("t", "k", RequestClass::Event).is_ok());
        assert!(admission.check_request("t", "k", RequestClass::Event).is_ok());
        let retry = admission
            .check_request("t", "k", RequestClass::Event)
            .expect_err("terceira precisa estourar");
        assert!(retry >= 1);
    }

    #[test]
    fn one_class_never_consumes_another_class_bucket() {
        let mut c = config();
        c.default_limits = ScopeLimits {
            requests_per_sec: 1.0,
            request_burst: 1.0,
            events_per_sec: 1.0,
            event_burst: 1.0,
        };
        let admission = Admission::new(c);
        assert!(admission.check_request("t", "k", RequestClass::Event).is_ok());
        // Flood de `event` não toca no bucket de `sos` do mesmo grant.
        assert!(admission.check_request("t", "k", RequestClass::Event).is_err());
        assert!(admission.check_request("t", "k", RequestClass::Sos).is_ok());
    }

    #[test]
    fn tenants_do_not_share_buckets() {
        let mut c = config();
        c.default_limits = ScopeLimits {
            requests_per_sec: 1.0,
            request_burst: 1.0,
            events_per_sec: 1.0,
            event_burst: 1.0,
        };
        let admission = Admission::new(c);
        assert!(admission.check_request("a", "k", RequestClass::Event).is_ok());
        assert!(admission.check_request("a", "k", RequestClass::Event).is_err());
        assert!(admission.check_request("b", "k", RequestClass::Event).is_ok());
    }

    #[test]
    fn flight_guard_balances_the_counter() {
        let admission = Admission::new(config());
        {
            let _g = admission.enter();
            assert_eq!(admission.in_flight(), 1);
        }
        assert_eq!(admission.in_flight(), 0);
    }

    #[test]
    fn inbox_cost_counts_declared_positions_and_survives_garbage() {
        assert_eq!(inbox_event_cost(br#"{"positions":[1,2,3]}"#), 3);
        assert_eq!(inbox_event_cost(br#"{"positions":[]}"#), 0);
        assert_eq!(inbox_event_cost(br#"not json"#), 0);
        assert_eq!(inbox_event_cost(br#"{"no_positions":1}"#), 0);
    }

    #[test]
    fn config_defaults_match_documented_values() {
        let c = config();
        assert_eq!(c.max_event_body_bytes, 64 * 1024);
        assert_eq!(c.max_inbox_body_bytes, 256 * 1024);
        assert_eq!(c.request_timeout, Duration::from_millis(10_000));
        assert_eq!(c.pressure_threshold, 64);
    }

    #[test]
    fn config_resolve_reads_env_style_map() {
        let vars: HashMap<String, String> = [
            (AdmissionConfig::ENV_MAX_EVENT_BODY.to_owned(), "8192".to_owned()),
            (AdmissionConfig::ENV_TIMEOUT_MS.to_owned(), "500".to_owned()),
            ("GODMESH_LIMIT_EVENT_WRITE_RPS".to_owned(), "10".to_owned()),
            ("GODMESH_LIMIT_EVENT_WRITE_EBUCKET".to_owned(), "50".to_owned()),
        ]
        .into_iter()
        .collect();
        let c = AdmissionConfig::resolve(|n| vars.get(n).cloned()).expect("config válida");
        assert_eq!(c.max_event_body_bytes, 8192);
        assert_eq!(c.request_timeout, Duration::from_millis(500));
        let limits = c.limits_for(RequestClass::Event);
        assert_eq!(limits.requests_per_sec, 10.0);
        assert_eq!(limits.event_burst, 50.0);
        // cada parâmetro é um knob separado; ausente herda o default resolvido
        assert_eq!(limits.events_per_sec, 1_000.0);
        assert_eq!(limits.request_burst, 1_000.0);
        // outra classe fica no default
        assert_eq!(c.limits_for(RequestClass::Sync).requests_per_sec, 1_000.0);
    }

    #[test]
    fn config_resolve_refuses_invalid_values_naming_the_var() {
        for (name, value) in [
            (AdmissionConfig::ENV_MAX_BODY, "0"),
            (AdmissionConfig::ENV_TIMEOUT_MS, "-1"),
            (AdmissionConfig::ENV_DEFAULT_RPS, "abc"),
            ("GODMESH_LIMIT_EVENT_WRITE_RPS", "0"),
        ] {
            let vars: HashMap<String, String> = [(name.to_owned(), value.to_owned())]
                .into_iter()
                .collect();
            let e = AdmissionConfig::resolve(|n| vars.get(n).cloned())
                .expect_err("valor inválido precisa derrubar");
            assert!(e.contains(name), "erro deve citar a variável: {e}");
        }
    }
}
