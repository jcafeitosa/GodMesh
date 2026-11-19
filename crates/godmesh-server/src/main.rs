//! Binário do servidor HTTP do GodMesh.
//!
//! Sequência de inicialização, sem etapa silenciosa:
//! 1. configuração do ambiente;
//! 2. conexão ao Postgres;
//! 3. **migrações** de `GODMESH_MIGRATIONS_DIR` (default `migrations`), em ordem,
//!    idempotentes; qualquer falha impede o processo de subir;
//! 4. com `--migrate-only`, fecha o pool e sai 0;
//! 5. escuta, serve, e encerra por SIGTERM/SIGINT drenando o que está em voo.
//!
//! Configuração vem do ambiente, sem default silencioso:
//! - `DATABASE_URL` (obrigatório, sempre)
//! - `GODMESH_MIGRATIONS_DIR` (opcional; default `migrations`)
//! - `GODMESH_GRANTS` (obrigatório para servir): registro de grants verificados
//!   em JSON; sem ele nenhuma credencial resolve e todo `/v1/incidents`
//!   responderia 401. Não é exigido em `--migrate-only`, que não serve rota.
//! - `BIND_ADDR` (obrigatório para servir, exceto em `GODMESH_ENV=dev|development`,
//!   onde o default é `127.0.0.1:8080`)
//! - `GODMESH_NODE_KEY` (obrigatório para servir): caminho do arquivo da
//!   identidade Ed25519 do nó. Ausente/vazio → exit 78; arquivo com permissão
//!   mais aberta que `0600` → exit 78. Se o arquivo não existir, ele é criado
//!   `0600` (primeiro boot de um nó novo).
//! - `GODMESH_FEDERATION_BIND` (opcional; **desligado por default**): endereço
//!   UDP do listener QUIC/mTLS da malha. Sem ela não há transporte de federação
//!   e `/v1/federation/peers/{node_id}/link` responde `unknown` — o piso honesto.
//!   Ligada, exige `GODMESH_FEDERATION_CLUSTER` e `GODMESH_FEDERATION_REGION`.
//! - `GODMESH_FEDERATION_GENESIS` (opcional; vazio = sem bootstrap): lista de
//!   `<node_id>:<chave pública em hex>` dos primeiros nós do cluster, autorizados
//!   por configuração (ADR-071 emenda §5). Entrada malformada derruba o processo
//!   com EX_CONFIG: lista de confiança não tem item "ignorado".
//! - `GODMESH_FEDERATION_GOVERNANCE` (opcional): `single_approver:<cluster>`
//!   declara o cluster sem votantes. Ausente ou vazia desabilita decisões.
//! - `GODMESH_FEDERATION_PEERS_REFRESH_MS` (opcional; default 5000): intervalo de
//!   recarga da allowlist de pares a partir de `federation_peer`.
//! - `GODMESH_ENV` (opcional; ausente é tratado como `production`)
//! - `GODMESH_OUTBOX` (opcional; `off` desliga o publicador, default `on`)
//! - `GODMESH_OUTBOX_INTERVAL_MS` (opcional; default 1000 ms)
//! - `GODMESH_HOST_METRICS_INTERVAL_MS` (opcional; default 5000): intervalo da
//!   coleta de host (ADR-073 camada 1). É leitura pura de `/proc`/cgroup/`statfs`
//!   e roda em background; o scrape serve o último snapshot. `0` é recusado —
//!   um laço apertado lendo o kernel seria um DoS autoinfligido.
//! - `GODMESH_HOST_PROC_ROOT` (opcional; default `/`): raiz de leitura, para
//!   evidência reproduzível (`host-evidence proc-fs`). Precisa ser absoluta.
//! - Portão de abuso (cotas/tetos/admissão — ver `godmesh_server::admission`):
//!   todos opcionais, com default declarado e validado na subida; valor
//!   inválido derruba o processo com EX_CONFIG em vez de servir com default
//!   silencioso: `GODMESH_MAX_BODY_BYTES`, `GODMESH_MAX_EVENT_BODY_BYTES`,
//!   `GODMESH_MAX_INBOX_BODY_BYTES`, `GODMESH_REQUEST_TIMEOUT_MS`,
//!   `GODMESH_ADMISSION_PRESSURE_THRESHOLD`, `GODMESH_LIMIT_REQUESTS_PER_SEC`,
//!   `GODMESH_LIMIT_REQUEST_BURST`, `GODMESH_LIMIT_EVENTS_PER_SEC`,
//!   `GODMESH_LIMIT_EVENT_BURST`, `GODMESH_LIMIT_IP_RPS`, `GODMESH_LIMIT_IP_BURST`
//!   e os overrides por escopo `GODMESH_LIMIT_<ESCOPO>_<RPS|RBUCKET|EPS|EBUCKET>`.
//!
//! O publicador do outbox (ADR-061) roda como tarefa tokio ao lado do HTTP e
//! entrega no sink de teste `outbox_sink` (migração 0003). Ele só publica; a
//! confirmação vem depois do efeito ter sido executado.
//!
//! Ausência de configuração é erro explícito com mensagem e código de saída != 0.
//! Um default de loopback em produção deixava o serviço inalcançável de fora do
//! contêiner sem nenhum erro; por isso o ambiente controla se o default existe.
//!
//! Nenhum exemplo de configuração neste arquivo traz senha: o binário é publicado.

#![forbid(unsafe_code)]

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use godmesh_federation::NodeIdentity;
use godmesh_federation::node::{
    FederationConfig, FederationNode, LinkAuthorizationError, LinkAuthorizer, LinkIdentity,
    LinkPermit,
};
use godmesh_federation::peer::PeerRegistry;
use godmesh_host::collect::{Collector, SharedSnapshot};
use godmesh_server::admission::AdmissionConfig;
use godmesh_server::auth::{Authorization, Clock, GrantRegistry, SystemClock};
use godmesh_server::config_bundle::{
    Activation, BundleService, Fence, Preflight, Rejection, WriteError,
};
use godmesh_server::config_https::{self, HttpsClient};
use godmesh_server::config_operational::{ConfigPlane, ConfigPlaneConfig, limit_statement_time};
use godmesh_server::http::HostTelemetry;
use godmesh_server::portable_join::{PortableJoin, PortableJoinConfig};
use godmesh_server::publisher::{OutboxConfig, Publisher};
use godmesh_server::replica_operational::{
    ReplicaConfig, ReplicaService, authorize_link_in_tx, authorize_link_until,
};
use godmesh_server::{federation, migrate, runtime};
use godmesh_sync::pg_outbox::{PgOutbox, PgSink};
use sqlx::postgres::PgPoolOptions;

mod replica_runtime;
#[cfg(all(test, feature = "snapshot-harness"))]
mod snapshot_lab;
use replica_runtime::ReplicaRuntime;

/// Endereço padrão aceito **apenas** em ambiente de desenvolvimento: só loopback.
/// Expor a interface exige configuração deliberada.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";

/// Valores de `GODMESH_ENV` que autorizam o default de loopback. Qualquer outro
/// valor — inclusive ausente — é tratado como produção (falha fechada).
const DEV_ENV_VALUES: [&str; 2] = ["dev", "development"];

/// O que o processo vai fazer depois de migrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Args {
    /// Aplica as migrações e sai 0.
    MigrateOnly,
    /// Aplica as migrações e serve.
    Serve,
}

impl Args {
    /// Interpreta os argumentos da linha de comando.
    ///
    /// # Errors
    /// Argumento desconhecido: migrar o banco por engano não pode ser o efeito de
    /// um typo.
    fn parse(raw: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut mode = Self::Serve;
        for arg in raw {
            match arg.as_str() {
                "--migrate-only" => mode = Self::MigrateOnly,
                other => {
                    return Err(format!(
                        "argumento desconhecido: {other}; uso: godmesh-server [--migrate-only]"
                    ));
                }
            }
        }
        Ok(mode)
    }
}

/// Intervalo padrão de recarga da allowlist de pares, em milissegundos.
const DEFAULT_FEDERATION_PEERS_REFRESH_MS: u64 = 5_000;

/// Configuração do runtime de federação deste processo.
///
/// A identidade é carregada **no boot** (não no primeiro handshake): um nó sem
/// identidade legível não sobe meio-vivo.
#[derive(Debug)]
struct FederationRuntime {
    identity: NodeIdentity,
    /// Listener QUIC. `None` é o default: transporte desligado, nenhum par
    /// entra e o diagnóstico de enlace responde `unknown`.
    listener: Option<FederationListener>,
    /// Intervalo de recarga da allowlist a partir de `federation_peer`.
    peers_refresh: Duration,
}

/// Listener QUIC efetivamente configurado.
#[derive(Debug)]
struct FederationListener {
    config: FederationConfig,
    bind: SocketAddr,
}

/// Configuração lida do ambiente.
struct Config {
    database_url: String,
    migrations_dir: PathBuf,
    mode: Mode,
    #[cfg(all(test, feature = "snapshot-harness"))]
    snapshot_lab: Option<(snapshot_lab::LabContext, snapshot_lab::LabRegistration)>,
}

/// Configuração específica do modo escolhido.
enum Mode {
    MigrateOnly,
    Serve {
        grants: GrantRegistry,
        bind_addr: String,
        outbox: OutboxConfig,
        /// Configuração da coleta de host (camada 1 de ADR-073).
        host: godmesh_host::Config,
        /// Em `Box` porque o runtime de federação é a maior parte do enum e
        /// `MigrateOnly` não carrega nada.
        federation: Box<FederationRuntime>,
        /// Em `Box` para o enum não crescer por um campo de config (clippy
        /// `large_enum_variant`); o conteúdo é idêntico.
        admission: Box<AdmissionConfig>,
        /// Bootstrap de confiança e governança de ingresso validados no boot.
        decisions: Arc<godmesh_server::federation::DecisionConfig>,
        portable: Option<Box<PortableJoinConfig>>,
        configuration: Option<Box<ConfigPlaneConfig>>,
        replica: Option<Box<ReplicaConfig>>,
    },
}

impl Config {
    /// Lê o ambiente e devolve erro legível quando algo obrigatório falta.
    ///
    /// # Errors
    /// `DATABASE_URL` ausente/vazia; `GODMESH_GRANTS` ausente, vazia ou inválida
    /// ao servir; `BIND_ADDR` ausente fora de `GODMESH_ENV=dev|development` ou
    /// inválido. Nenhuma mensagem inclui token.
    fn from_env(args: Args) -> Result<Self, String> {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(v) if !v.trim().is_empty() => v,
            Ok(_) => {
                return Err(
                    "DATABASE_URL está definida mas vazia; informe a URL do Postgres".to_owned(),
                );
            }
            Err(std::env::VarError::NotPresent) => {
                return Err(
                    "DATABASE_URL não definida; informe a URL do Postgres em DATABASE_URL (exemplo de formato: postgres://usuario:senha@host:porta/banco)"
                        .to_owned(),
                );
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err("DATABASE_URL contém bytes não-UTF8".to_owned());
            }
        };

        let migrations_dir = migrate::dir_from_env();

        let mode = match args {
            Args::MigrateOnly => Mode::MigrateOnly,
            Args::Serve => {
                let bind_addr = resolve_bind_addr(
                    std::env::var("BIND_ADDR").ok().as_deref(),
                    std::env::var("GODMESH_ENV").ok().as_deref(),
                )?;
                let grants = GrantRegistry::from_env()
                    .map_err(|e| format!("registro de grants indisponível: {e}"))?;
                let outbox =
                    OutboxConfig::from_env().map_err(|e| format!("configuração do outbox: {e}"))?;
                let host = godmesh_host::Config::from_env()
                    .map_err(|e| format!("configuração da telemetria de host: {e}"))?;
                let decisions = Arc::new(godmesh_server::federation::DecisionConfig::from_env()?);
                let federation = federation_runtime_from_env()?;
                let portable = PortableJoinConfig::from_env()
                    .map_err(|error| error.to_string())?
                    .map(Box::new);
                let configuration = ConfigPlaneConfig::from_env()
                    .map_err(|error| error.to_string())?
                    .map(Box::new);
                let replica = ReplicaConfig::from_env()
                    .map_err(|error| error.to_string())?
                    .map(Box::new);
                // O portão de abuso valida o ambiente na subida: tetos
                // inválidos derrubam o processo (EX_CONFIG) em vez de virar
                // default silencioso servindo tráfego.
                let admission = AdmissionConfig::from_env()
                    .map_err(|e| format!("configuração de cotas/admissão: {e}"))?;
                Mode::Serve {
                    grants,
                    bind_addr,
                    outbox,
                    host,
                    federation: Box::new(federation),
                    admission: Box::new(admission),
                    decisions,
                    portable,
                    configuration,
                    replica,
                }
            }
        };

        Ok(Self {
            database_url,
            migrations_dir,
            mode,
            #[cfg(all(test, feature = "snapshot-harness"))]
            snapshot_lab: None,
        })
    }
}

/// Decide o endereço de escuta a partir do ambiente.
///
/// Fora de `dev`/`development`, `BIND_ADDR` é obrigatório: um default de loopback
/// em produção faz o contêiner subir "saudável" e inalcançável.
///
/// # Errors
/// Devolve mensagem citando `BIND_ADDR` quando ela falta e o ambiente não é dev.
fn resolve_bind_addr(bind_addr: Option<&str>, env: Option<&str>) -> Result<String, String> {
    if let Some(v) = bind_addr.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(v.to_owned());
    }

    let env_normalizado = env.map(str::trim).map(str::to_ascii_lowercase);
    if env_normalizado
        .as_deref()
        .is_some_and(|e| DEV_ENV_VALUES.contains(&e))
    {
        return Ok(DEFAULT_BIND_ADDR.to_owned());
    }

    Err(format!(
        "BIND_ADDR não definida; com GODMESH_ENV={} o endereço de escuta é obrigatório \
         (exemplo: BIND_ADDR=0.0.0.0:8080); o default {DEFAULT_BIND_ADDR} só vale em GODMESH_ENV=dev",
        env.unwrap_or("<ausente> = production")
    ))
}

/// Lê a configuração do runtime de federação do ambiente.
///
/// Ver [`federation_runtime_from`] para as regras e as falhas.
fn federation_runtime_from_env() -> Result<FederationRuntime, String> {
    federation_runtime_from(|name| std::env::var(name).ok())
}

/// Interpreta a configuração de federação a partir de um leitor de variáveis.
///
/// Falha **alto** e antes de qualquer efeito: identidade ausente, chave legível
/// por outros, endereço inválido ou cluster/região ausentes com o listener
/// ligado. Nenhuma dessas condições cai em default silencioso. Separado do
/// ambiente para que o teste exercite exatamente essas falhas sem mexer em
/// `std::env` (que é global e concorrente entre testes).
///
/// # Errors
/// Mensagem que cita a variável culpada; nunca inclui o conteúdo da chave.
fn federation_runtime_from(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<FederationRuntime, String> {
    let identity =
        NodeIdentity::from_env_with(&lookup).map_err(|e| format!("identidade de nó indisponível: {e}"))?;

    let bind = match lookup("GODMESH_FEDERATION_BIND") {
        Some(raw) if !raw.trim().is_empty() => raw.trim().parse::<SocketAddr>().map_err(|e| {
            format!("GODMESH_FEDERATION_BIND inválida (esperado IP:porta): {e}")
        })?,
        _ => {
            // Sem listener: nenhum nó entra, nenhum heartbeat existe e o
            // diagnóstico de enlace responde `unknown`. Isso é explícito.
            return Ok(FederationRuntime {
                identity,
                listener: None,
                peers_refresh: Duration::ZERO,
            });
        }
    };

    let required = |name: &str| -> Result<String, String> {
        match lookup(name) {
            Some(v) if !v.trim().is_empty() => Ok(v.trim().to_owned()),
            _ => Err(format!(
                "{name} não definida; com GODMESH_FEDERATION_BIND ativo o cluster e a região \
                 do nó são obrigatórios (sem default silencioso)"
            )),
        }
    };
    let cluster = required("GODMESH_FEDERATION_CLUSTER")?;
    let region = required("GODMESH_FEDERATION_REGION")?;
    let version = lookup("GODMESH_FEDERATION_VERSION")
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());

    let peers_refresh = match lookup("GODMESH_FEDERATION_PEERS_REFRESH_MS") {
        Some(raw) => raw
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("GODMESH_FEDERATION_PEERS_REFRESH_MS inválido: {e}"))?,
        None => DEFAULT_FEDERATION_PEERS_REFRESH_MS,
    };

    Ok(FederationRuntime {
        identity,
        listener: Some(FederationListener {
            config: FederationConfig::new(cluster, region, version),
            bind,
        }),
        peers_refresh: Duration::from_millis(peers_refresh.max(1)),
    })
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("godmesh-server: {msg}");
            return ExitCode::from(64); // EX_USAGE
        }
    };

    let config = match Config::from_env(args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("godmesh-server: configuração inválida: {msg}");
            return ExitCode::from(78); // EX_CONFIG
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("godmesh-server: falha ao criar runtime tokio: {e}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("godmesh-server: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// Liga a autoridade do PostgreSQL às decisões locais de enlace.
struct PortableLinkAuthorizer {
    portable: Arc<PortableJoin>,
    configuration: Option<Arc<BundleService>>,
    changes: tokio::sync::watch::Receiver<u64>,
}

impl LinkAuthorizer for PortableLinkAuthorizer {
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            let mut transaction = self
                .portable
                .pool()
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(|_| LinkAuthorizationError::Unavailable)?;
            let expires = authorize_link_in_tx(
                &self.portable,
                self.configuration.as_deref(),
                local,
                remote,
                &mut transaction,
            )
            .await?
            .map(monotonic_lease_deadline)
            .transpose()?;
            Ok(LinkPermit::new(Box::new(transaction), expires))
        })
    }

    fn authorize_until<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
        deadline: tokio::time::Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            let (protection, expires_at) = authorize_link_until(
                Arc::clone(&self.portable),
                self.configuration.clone(),
                local,
                remote,
                deadline,
            )
            .await?;
            let expires = expires_at.map(monotonic_lease_deadline).transpose()?;
            Ok(LinkPermit::new(protection, expires))
        })
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.clone()
    }
}

fn monotonic_lease_deadline(expires_at: i64) -> Result<Instant, LinkAuthorizationError> {
    let seconds = u64::try_from(expires_at).map_err(|_| LinkAuthorizationError::Denied)?;
    let wall_deadline = UNIX_EPOCH
        .checked_add(Duration::from_secs(seconds))
        .ok_or(LinkAuthorizationError::Denied)?;
    let monotonic_now = Instant::now();
    let remaining = wall_deadline
        .duration_since(SystemTime::now())
        .map_err(|_| LinkAuthorizationError::Denied)?;
    monotonic_now
        .checked_add(remaining)
        .ok_or(LinkAuthorizationError::Denied)
}

/// Ativa a pendência e inspeciona o fence com o mesmo guard e segundo.
async fn observe_configuration(
    portable: &PortableJoin,
    configuration: &BundleService,
    clock: &dyn Clock,
    deadline: Option<tokio::time::Instant>,
) -> Result<(Activation, Fence, Option<i64>), WriteError> {
    let mut tx = portable
        .pool()
        .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
        .await?;
    if let Some(deadline) = deadline {
        limit_statement_time(&mut tx, deadline).await?;
    }
    let mut guard = portable.lock_cluster_guard(&mut tx).await?;
    let now = clock.now_unix();
    let activation = configuration
        .activate_due_in_tx(&mut guard, portable, now)
        .await?;
    let fence = configuration
        .classify_fence_in_tx(&mut guard, 1, now)
        .await?;
    let next = match fence {
        Fence::Allowed { recheck_at, .. } => Some(recheck_at),
        _ => configuration.next_transition_in_tx(&mut guard, now).await?,
    };
    if let Some(deadline) = deadline {
        limit_statement_time(&mut tx, deadline).await?;
    }
    tx.commit().await?;
    Ok((activation, fence, next))
}

fn spawn_configuration_lifecycle(
    portable: Arc<PortableJoin>,
    configuration: Arc<BundleService>,
    mut committed: tokio::sync::watch::Receiver<u64>,
    interval: Duration,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    changes: tokio::sync::watch::Sender<u64>,
) -> tokio::task::JoinHandle<()> {
    let mut trust = portable.trust_changes();
    tokio::spawn(async move {
        loop {
            let deadline = tokio::time::Instant::now() + interval;
            let observed = tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                result = tokio::time::timeout_at(
                    deadline,
                    observe_configuration(&portable, &configuration, &SystemClock, Some(deadline))
                ) => result,
            };
            if !matches!(
                observed,
                Ok(Ok((Activation::NoChange | Activation::Blocked(_), Fence::Allowed { .. }, _)))
            ) {
                // Publica a ativação após commit e invalida evidência se o banco falhar.
                changes.send_modify(|value| *value = value.wrapping_add(1));
            }
            let mut delay = interval;
            if let Ok(Ok((_, _, Some(next)))) = observed {
                delay = match monotonic_lease_deadline(next) {
                    Ok(deadline) => {
                        interval.min(deadline.saturating_duration_since(Instant::now()))
                    }
                    Err(_) => Duration::ZERO,
                };
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                change = trust.changed() => {
                    if change.is_err() { break; }
                    changes.send_modify(|value| *value = value.wrapping_add(1));
                }
                change = committed.changed() => {
                    if change.is_err() { break; }
                }
                _ = tokio::time::sleep(delay) => {}
            }
        }
    })
}

/// Conecta, migra, e serve até receber sinal de término (ou só migra).
async fn run(config: Config) -> Result<(), String> {
    #[cfg(all(test, feature = "snapshot-harness"))]
    let lab = {
        if config.snapshot_lab.is_some()
            && !matches!(config.mode, Mode::Serve { replica: Some(_), .. })
        {
            return Err("snapshot de laboratório exige réplica M4 configurada".to_owned());
        }
        config.snapshot_lab
    };
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .map_err(|e| format!("não foi possível conectar ao Postgres em DATABASE_URL: {e}"))?;

    #[cfg(all(test, feature = "snapshot-harness"))]
    if let Some((_, registration)) = &lab {
        registration.verify_database(&pool).await?;
    }

    let report = migrate::migrate(&pool, &config.migrations_dir)
        .await
        .map_err(|e| {
            format!(
                "migração falhou ({}, banco não subiu): {e}",
                migrate::SCHEMA_VERSION
            )
        })?;
    println!(
        "godmesh-server: migrações em {} — {} aplicada(s) [{}], {} já aplicada(s) [{}]",
        report.dir.display(),
        report.applied.len(),
        report.applied.join(", "),
        report.skipped_count(),
        report.skipped.join(", "),
    );

    let Mode::Serve {
        grants,
        bind_addr,
        outbox,
        host,
        federation,
        admission,
        decisions,
        portable,
        configuration,
        replica,
    } = config.mode
    else {
        pool.close().await;
        return Ok(());
    };
    let FederationRuntime {
        identity,
        listener: federation_listener,
        peers_refresh,
    } = *federation;
    PortableJoin::preflight(&pool, portable.as_deref())
        .await
        .map_err(|error| format!("preflight do ingresso portátil: {error}"))?;
    let identity = Arc::new(identity);
    let portable = portable.map(|policy| {
        Arc::new(PortableJoin::new(
            *policy,
            pool.clone(),
            Arc::clone(&identity),
        ))
    });

    let policy = configuration.as_ref().map(|config| config.policy.clone());
    let preflight = BundleService::preflight(&pool, portable.as_deref(), policy)
        .await
        .map_err(|error| format!("preflight da configuração M3: {error}"))?;
    let (config_plane, replica_runtime) = match (preflight, configuration, portable.as_ref()) {
        (Preflight::Disabled, None, _) if replica.is_some() => {
            return Err("m4_preflight_mismatch: a réplica exige M1C e M3 configuradas".to_owned());
        }
        (Preflight::Disabled, None, _) => (None, None),
        (Preflight::Enabled(service), Some(config), Some(portable)) => {
            let service = Arc::new(service);
            let replica_runtime = if let Some(replica) = replica {
                if federation_listener.is_none() {
                    return Err("m4_preflight_mismatch: a réplica exige o listener QUIC".to_owned());
                }
                let replica = ReplicaService::preflight(
                    *replica,
                    Arc::clone(portable),
                    Arc::clone(&service),
                    &identity,
                )
                .await
                .map_err(|error| error.to_string())?;
                let runtime = ReplicaRuntime::new(replica);
                #[cfg(all(test, feature = "snapshot-harness"))]
                let runtime = if let Some((context, _)) = lab {
                    runtime.with_snapshot(context, Arc::clone(&identity)).await?
                } else {
                    runtime
                };
                Some(Arc::new(runtime))
            } else {
                None
            };
            // Nós novos precisam do plano de controle para ingressar; autoridade
            // ausente bloqueia seus efetores, mas permite o bootstrap M1C.
            match observe_configuration(portable, &service, &SystemClock, None).await {
                Ok(_) | Err(WriteError::Rejected(Rejection::LocalAuthority)) => {}
                Err(error) => return Err(format!("estado efetivo da configuração M3: {error}")),
            }
            let https_addr = config.bind;
            let pins = config.tls_pins();
            let limits = config.https_limits();
            let interval = config.link_recheck_interval;
            let client = HttpsClient::new(&identity, limits)
                .map_err(|error| format!("cliente de configuração M3: {error}"))?;
            let plane = Arc::new(
                ConfigPlane::new(
                    *config,
                    service,
                    Arc::clone(portable),
                    Arc::clone(&identity),
                    Arc::new(Authorization::new(grants.clone(), pool.clone())),
                    (*admission).clone(),
                )
                .map_err(|error| format!("configuração M3: {error}"))?,
            );
            let tls_listener = tokio::net::TcpListener::bind(https_addr)
                .await
                .map_err(|error| format!("listener HTTPS de configuração: {error}"))?;
            (Some((plane, tls_listener, client, limits, pins, interval)), replica_runtime)
        }
        _ => return Err("estado de preflight incoerente para a configuração M3".to_owned()),
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| format!("não foi possível escutar em BIND_ADDR={bind_addr}: {e}"))?;

    let local = listener
        .local_addr()
        .map_err(|e| format!("listener sem endereço local: {e}"))?;
    println!(
        "godmesh-server: escutando em http://{local} ({} grant(s) configurado(s))",
        grants.len()
    );
    // `public_key` é dado público (é o que verifica manifesto e o que o
    // registro de pares do outro nó guarda); a semente nunca é impressa.
    println!(
        "godmesh-server: identidade de nó carregada: node_id={} public_key={}",
        identity.node_id(),
        identity.public_key_hex()
    );
    println!(
        "godmesh-server: bootstrap de federação declarado: {} nó(s) genesis",
        decisions.genesis_count()
    );

    // Um único sinal de término alimenta o HTTP, o publicador e a federação.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let sinal = runtime::shutdown_signal();
    let stop_on_signal = shutdown_tx.clone();
    tokio::spawn(async move {
        sinal.await;
        let _ = stop_on_signal.send(true);
    });

    let config_service = config_plane
        .as_ref()
        .map(|(plane, ..)| Arc::clone(plane.state()));
    let config_signal = config_plane
        .as_ref()
        .map(|_| tokio::sync::watch::channel(0_u64));
    let config_changes = config_signal.as_ref().map(|(_, receiver)| receiver.clone());

    // Runtime de federação: allowlist carregada de `federation_peer` (a decisão
    // administrativa é a autoridade) e recarregada periodicamente, para que
    // aprovar/revogar pela API valha sem reiniciar o processo.
    let runtime_federation = match federation_listener {
        None => {
            println!(
                "godmesh-server: federação desligada (GODMESH_FEDERATION_BIND ausente); \
                 diagnóstico de enlace responde unknown"
            );
            None
        }
        Some(listener) => {
            let listener_config = listener.config;
            let listener_bind = listener.bind;
            let registry = Arc::new(PeerRegistry::new());
            let peers = federation::list_peers(&pool)
                .await
                .map_err(|e| format!("registro de pares indisponível: {e:?}"))?;
            registry.replace(peers);
            let authorizer = portable.as_ref().map(|service| {
                Arc::new(PortableLinkAuthorizer {
                    portable: Arc::clone(service),
                    configuration: config_service.clone(),
                    changes: config_changes
                        .clone()
                        .unwrap_or_else(|| service.trust_changes()),
                }) as Arc<dyn LinkAuthorizer>
            });
            let node = match (replica_runtime, authorizer) {
                (Some(handler), Some(authorizer)) => handler.bind(
                    Arc::clone(&identity),
                    listener_config,
                    Arc::clone(&registry),
                    listener_bind,
                    authorizer,
                ),
                (Some(_), None) => {
                    return Err("m4_preflight_mismatch: a réplica exige autorização portátil".to_owned());
                }
                (None, authorizer) => FederationNode::bind_with_authorizer(
                    Arc::clone(&identity),
                    listener_config,
                    Arc::clone(&registry),
                    listener_bind,
                    authorizer,
                ),
            }
            .map_err(|e| format!("listener de federação não abriu: {e}"))?;
            let addr = node
                .local_addr()
                .map(|addr| format!("quic://{addr}"))
                .unwrap_or_else(|| format!("quic://{listener_bind}"));
            println!(
                "godmesh-server: federação escutando em {addr} ({} par(es) no registro, \
                 ALPN godmesh/1)",
                registry.len()
            );
            Some(Arc::new(node))
        }
    };

    let peer_task = runtime_federation.as_ref().map(|node| {
        godmesh_server::peer_maintenance::spawn(
            pool.clone(),
            Arc::clone(node),
            peers_refresh,
            shutdown_rx.clone(),
        )
    });

    // Só inicia tarefas de controle após a montagem dos listeners.
    let control_tasks = match (config_plane, config_signal) {
        (Some((plane, listener, client, limits, pins, interval)), Some((changes, _))) => {
            let portable = Arc::clone(portable.as_ref().ok_or("M3 exige ingresso portátil")?);
            let lifecycle = spawn_configuration_lifecycle(
                portable,
                Arc::clone(plane.state()),
                plane.changes(),
                interval,
                shutdown_rx.clone(),
                changes,
            );
            let router = plane.router();
            println!(
                "godmesh-server: configuração M3 escutando em https://{}",
                listener.local_addr().map_err(|error| error.to_string())?
            );
            let tls_identity = Arc::clone(&identity);
            let tls_stop = shutdown_rx.clone();
            let stop_on_tls_failure = shutdown_tx.clone();
            let listener_task = tokio::spawn(async move {
                let result =
                    config_https::serve(listener, router, &tls_identity, pins, limits, tls_stop)
                        .await;
                if result.is_err() {
                    let _ = stop_on_tls_failure.send(true);
                }
                result
            });
            let poll_stop = shutdown_rx.clone();
            let poll_task = tokio::spawn(async move {
                plane.run_polling(&client, poll_stop).await;
            });
            Some((lifecycle, listener_task, poll_task))
        }
        _ => None,
    };

    let publisher = if outbox.enabled {
        println!(
            "godmesh-server: outbox publicador ativo a cada {} ms (sink de teste outbox_sink)",
            outbox.interval.as_millis()
        );
        let mut parar = shutdown_rx.clone();
        let publisher = Publisher::new(PgOutbox::new(pool.clone()), PgSink::new(pool.clone()));
        Some(tokio::spawn(async move {
            publisher
                .run(outbox.interval, async move {
                    let _ = parar.changed().await;
                })
                .await;
        }))
    } else {
        println!("godmesh-server: outbox publicador desligado (GODMESH_OUTBOX=off)");
        None
    };

    // Coleta de host (ADR-073 camada 1): um snapshot a cada intervalo, lido em
    // background. O handler de `/metrics` e o `/healthz` só leem o último.
    // Nenhuma ação sobre o host acontece aqui — é observação pura.
    let host_snapshot = SharedSnapshot::new();
    let host_task = spawn_host_collector(
        host.clone(),
        host_snapshot.clone(),
        shutdown_rx.clone(),
    );

    let mut encerrar = shutdown_rx;
    // `router_runtime_with_host`: a única porta que declara capacidades
    // efetivas — publicador ligado/desligado, federação anexada/ausente, cotas
    // validadas no boot e telemetria de host — exatamente como este processo
    // é, em `GET /v1/meta`. Sem `GODMESH_FEDERATION_BIND` o diagnóstico
    // responde `unknown` — não "saúde" por omissão. Sem coletor de host o
    // `/metrics` declararia `source="none"`; aqui há coletor de verdade.
    let federation_links = runtime_federation.as_ref().map(|node| node.links());
    let mut app = godmesh_server::http::router_runtime_with_portable(
        pool.clone(),
        grants,
        federation_links,
        *admission,
        outbox.enabled,
        HostTelemetry::new(host_snapshot.clone(), host.stale_after()),
        decisions,
        portable,
    );
    if let Some(service) = config_service {
        app = app.layer(axum::Extension(service));
    }
    let result = runtime::serve_with_shared_pool(listener, app, async move {
        let _ = encerrar.wait_for(|stopped| *stopped).await;
    })
    .await;
    let served = runtime::finish_after_http(result, &shutdown_tx, &pool, async move {
        if let Some(task) = peer_task {
            let _ = task.await;
        }
        if let Some(node) = runtime_federation {
            node.shutdown();
        }
        if let Some(publisher) = publisher {
            let _ = publisher.await;
        }
        if let Some(task) = host_task {
            let _ = task.await;
        }
        if let Some((lifecycle, listener_task, polling)) = control_tasks {
            let _ = lifecycle.await;
            let _ = polling.await;
            listener_task
                .await
                .map_err(|error| format!("listener M3: {error}"))?
                .map_err(|error| format!("listener M3 encerrou com erro: {error}"))?;
        }
        Ok(())
    })
    .await?;

    println!(
        "godmesh-server: shutdown solicitado; {} requisição(ões) em voo drenada(s)",
        served.drained
    );
    println!("godmesh-server: encerrado");
    Ok(())
}

/// Coleta o host no intervalo configurado e publica o snapshot.
///
/// Por que `spawn_blocking`: a leitura de `/proc` é I/O síncuo de ~1 ms
/// (medido em `crates/godmesh-host/tests/cost.rs`) e não pertence à thread de
/// um worker async, onde atrasaria handlers. O custo é pago uma vez por
/// intervalo, nunca por scrape.
///
/// A primeira coleta acontece **antes** do primeiro tick: sem isso, o nó
/// nasceria com `/metrics` dizendo "nenhuma leitura" durante um intervalo
/// inteiro, e um painel recém-subido é justamente quando alguém olha.
fn spawn_host_collector(
    config: godmesh_host::Config,
    published: SharedSnapshot,
    mut parar: tokio::sync::watch::Receiver<bool>,
) -> Option<tokio::task::JoinHandle<()>> {
    let interval = config.interval;
    let raiz = config.proc_root.display().to_string();
    println!(
        "godmesh-server: telemetria de host ativa (camada 1, read-only): snapshot a cada \
         {} ms, raiz {raiz}, /metrics com prefixo godmesh_host_",
        interval.as_millis()
    );

    Some(tokio::spawn(async move {
        let collector = Collector::with_read(
            godmesh_host::read::Read::new(&config.proc_root),
            std::sync::Arc::new(godmesh_host::collect::SysinfoVolumes),
        );
        publicar(&collector, &published).await;
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = parar.changed() => return,
            }
            publicar(&collector, &published).await;
        }
    }))
}

/// Uma coleta, executada fora das threads do runtime.
///
/// `spawn_blocking` porque a leitura de `/proc` é I/O síncuo: ~1 ms medidos,
/// mas um kernel sob pressão pode demorar muito mais, e isso não pode atrasar
/// um handler de incidente. O `await` aqui é o que garante uma coleta por vez
/// — sem ele, um ciclo lento se sobreporia ao próximo.
///
/// Falha não apaga o painel: se a tarefa de background entrar em pânico, o
/// último snapshot publicado continua servido, e `snapshot_age_seconds` passa
/// a envelhecer até o `/metrics` declarar o número velho como indisponível.
async fn publicar(collector: &Collector, published: &SharedSnapshot) {
    let collector = collector.clone();
    let published = published.clone();
    if tokio::task::spawn_blocking(move || {
        let snapshot = collector.collect();
        published.store(snapshot);
    })
    .await
    .is_err()
    {
        eprintln!(
            "godmesh-server: a coleta de host falhou; mantendo o último snapshot \
             publicado (idade visível em godmesh_host_snapshot_age_seconds)"
        );
    }
}

// Módulo de testes no fim do arquivo: `clippy::items-after-test-module`
// (negado por `-D clippy::all`) exige que nenhum item venha depois dele.
#[cfg(test)]
mod tests {
    use super::{
        Args, DEFAULT_BIND_ADDR, federation_runtime_from, resolve_bind_addr,
    };
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// Diretório temporário próprio por teste (nenhum teste compartilha chave).
    fn temp_key(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "godmesh-boot-federation-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir temporário");
        dir.join("node.key")
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn fed_sem_godmesh_node_key_falha_citando_a_variavel() {
        let erro = federation_runtime_from(env(&[])).expect_err("sem identidade não sobe");
        assert!(
            erro.contains("GODMESH_NODE_KEY"),
            "mensagem precisa citar GODMESH_NODE_KEY: {erro}"
        );
    }

    #[test]
    fn fed_com_arquivo_de_chave_aberto_falha_fechado() {
        let path = temp_key("aberto");
        let key = path.to_string_lossy().into_owned();
        // Primeiro boot cria a chave 0600.
        federation_runtime_from(env(&[("GODMESH_NODE_KEY", &key)])).expect("cria identidade");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let erro = federation_runtime_from(env(&[("GODMESH_NODE_KEY", &key)]))
            .expect_err("chave legível por outros não é identidade confiável");
        assert!(erro.contains("permissão"), "{erro}");
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    #[test]
    fn fed_desligada_por_default_e_carrega_a_identidade() {
        let path = temp_key("desligada");
        let key = path.to_string_lossy().into_owned();
        let runtime = federation_runtime_from(env(&[("GODMESH_NODE_KEY", &key)]))
            .expect("sem GODMESH_FEDERATION_BIND nada mais é exigido");
        assert!(runtime.listener.is_none(), "transporte desligado por default");
        assert!(runtime.identity.node_id().starts_with("gmn_"));
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    #[test]
    fn fed_com_bind_invalida_falha_citando_a_variavel() {
        let path = temp_key("bind");
        let key = path.to_string_lossy().into_owned();
        let erro = federation_runtime_from(env(&[
            ("GODMESH_NODE_KEY", &key),
            ("GODMESH_FEDERATION_BIND", "nao-e-endereco"),
        ]))
        .expect_err("endereço inválido não vira default");
        assert!(erro.contains("GODMESH_FEDERATION_BIND"), "{erro}");
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    #[test]
    fn fed_com_bind_exige_cluster_e_regiao() {
        let path = temp_key("cluster");
        let key = path.to_string_lossy().into_owned();
        let erro = federation_runtime_from(env(&[
            ("GODMESH_NODE_KEY", &key),
            ("GODMESH_FEDERATION_BIND", "127.0.0.1:0"),
        ]))
        .expect_err("cluster ausente não tem default silencioso");
        assert!(erro.contains("GODMESH_FEDERATION_CLUSTER"), "{erro}");

        let erro = federation_runtime_from(env(&[
            ("GODMESH_NODE_KEY", &key),
            ("GODMESH_FEDERATION_BIND", "127.0.0.1:0"),
            ("GODMESH_FEDERATION_CLUSTER", "cluster-br-sul"),
        ]))
        .expect_err("região ausente não tem default silencioso");
        assert!(erro.contains("GODMESH_FEDERATION_REGION"), "{erro}");
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    #[test]
    fn fed_com_bind_completa_liga_o_listener() {
        let path = temp_key("ok");
        let key = path.to_string_lossy().into_owned();
        let runtime = federation_runtime_from(env(&[
            ("GODMESH_NODE_KEY", &key),
            ("GODMESH_FEDERATION_BIND", "127.0.0.1:0"),
            ("GODMESH_FEDERATION_CLUSTER", "cluster-br-sul"),
            ("GODMESH_FEDERATION_REGION", "br-sul"),
        ]))
        .expect("configuração completa");
        let listener = runtime.listener.expect("listener ligado");
        assert_eq!(listener.config.cluster, "cluster-br-sul");
        assert_eq!(listener.config.region, "br-sul");
        assert_eq!(listener.bind.port(), 0);
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }

    #[test]
    fn bind_addr_ausente_em_producao_falha_citando_a_variavel() {
        let erro = resolve_bind_addr(None, Some("production")).expect_err("deve falhar");
        assert!(
            erro.contains("BIND_ADDR"),
            "mensagem precisa citar BIND_ADDR: {erro}"
        );
    }

    #[test]
    fn bind_addr_ausente_sem_godmesh_env_e_tratada_como_producao() {
        assert!(resolve_bind_addr(None, None).is_err());
    }

    #[test]
    fn bind_addr_vazia_em_producao_falha() {
        assert!(resolve_bind_addr(Some("   "), Some("production")).is_err());
    }

    #[test]
    fn bind_addr_desconhecida_falha_fechada() {
        assert!(resolve_bind_addr(None, Some("staging")).is_err());
    }

    #[test]
    fn bind_addr_default_loopback_so_em_dev() {
        assert_eq!(
            resolve_bind_addr(None, Some("dev")).unwrap(),
            DEFAULT_BIND_ADDR
        );
        assert_eq!(
            resolve_bind_addr(None, Some("development")).unwrap(),
            DEFAULT_BIND_ADDR
        );
        assert_eq!(
            resolve_bind_addr(None, Some("DEV")).unwrap(),
            DEFAULT_BIND_ADDR
        );
    }

    #[test]
    fn bind_addr_explicita_vale_em_qualquer_ambiente() {
        assert_eq!(
            resolve_bind_addr(Some("0.0.0.0:8080"), Some("production")).unwrap(),
            "0.0.0.0:8080"
        );
    }

    #[test]
    fn sem_argumento_serve_e_migrate_only_migra_e_sai() {
        assert_eq!(Args::parse([]).unwrap(), Args::Serve);
        assert_eq!(
            Args::parse(["--migrate-only".to_owned()]).unwrap(),
            Args::MigrateOnly
        );
    }

    #[test]
    fn argumento_desconhecido_falha_antes_de_qualquer_efeito() {
        let erro = Args::parse(["--migrate".to_owned()]).expect_err("deve falhar");
        assert!(
            erro.contains("--migrate"),
            "mensagem precisa citar o argumento: {erro}"
        );
    }
}
