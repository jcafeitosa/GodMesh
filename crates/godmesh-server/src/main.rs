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
use std::pin::Pin;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use godmesh_federation::link::LinkState;
use godmesh_federation::node::{
    FederationConfig, FederationNode, LinkAuthorizationError, LinkAuthorizer, LinkIdentity,
    LinkPermit,
};
use godmesh_federation::peer::PeerRegistry;
use godmesh_federation::NodeIdentity;
use godmesh_host::collect::{Collector, SharedSnapshot};
use godmesh_server::admission::AdmissionConfig;
use godmesh_server::auth::GrantRegistry;
use godmesh_server::http::HostTelemetry;
use godmesh_server::portable_join::{PortableJoin, PortableJoinConfig, PortableJoinError};
use godmesh_server::publisher::{OutboxConfig, Publisher};
use godmesh_server::{federation, migrate, runtime};
use godmesh_sync::pg_outbox::{PgOutbox, PgSink};
use sqlx::postgres::PgPoolOptions;

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
                }
            }
        };

        Ok(Self {
            database_url,
            migrations_dir,
            mode,
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
struct PortableLinkAuthorizer(Arc<PortableJoin>);

impl LinkAuthorizer for PortableLinkAuthorizer {
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            if self.0.cluster() != local.cluster || local.cluster != remote.cluster {
                return Err(LinkAuthorizationError::Denied);
            }
            let mut transaction = self
                .0
                .pool()
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(|_| LinkAuthorizationError::Unavailable)?;
            let local_expiry = self
                .0
                .authorize_local(&mut transaction)
                .await
                .map_err(link_authorization_error)?;
            let remote_expiry = self
                .0
                .authorize_peer(&mut transaction, &remote.node_id, &hex::encode(remote.public_key))
                .await
                .map_err(link_authorization_error)?;
            let expires_at = [local_expiry, remote_expiry]
                .into_iter()
                .flatten()
                .min()
                .map(monotonic_lease_deadline)
                .transpose()?;
            // O crate aplica a decisão com a transação ainda protegendo o cluster.
            Ok(LinkPermit::new(Box::new(transaction), expires_at))
        })
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.0.trust_changes()
    }
}

fn link_authorization_error(error: PortableJoinError) -> LinkAuthorizationError {
    match error {
        PortableJoinError::Storage(_) => LinkAuthorizationError::Unavailable,
        _ => LinkAuthorizationError::Denied,
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

/// Conecta, migra, e serve até receber sinal de término (ou só migra).
async fn run(config: Config) -> Result<(), String> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .map_err(|e| format!("não foi possível conectar ao Postgres em DATABASE_URL: {e}"))?;

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
    let portable = portable
        .map(|policy| Arc::new(PortableJoin::new(*policy, pool.clone(), Arc::clone(&identity))));

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
    tokio::spawn(async move {
        sinal.await;
        let _ = shutdown_tx.send(true);
    });

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
                Arc::new(PortableLinkAuthorizer(Arc::clone(service))) as Arc<dyn LinkAuthorizer>
            });
            let node = FederationNode::bind_with_authorizer(
                identity,
                listener_config,
                Arc::clone(&registry),
                listener_bind,
                authorizer,
            )
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

    if let Some(node) = &runtime_federation {
        // A manutenção usa o **mesmo** registro que o verificador de certificado
        // do nó: `PeerRegistry` é compartilhado por `Arc`.
        spawn_peers_maintenance(
            pool.clone(),
            Arc::clone(node),
            peers_refresh,
            shutdown_rx.clone(),
        );
    }

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
    let app = godmesh_server::http::router_runtime_with_portable(
        pool.clone(),
        grants,
        federation_links,
        *admission,
        outbox.enabled,
        HostTelemetry::new(host_snapshot.clone(), host.stale_after()),
        decisions,
        portable,
    );
    let served = runtime::serve(listener, app, pool.clone(), async move {
        let _ = encerrar.changed().await;
    })
    .await?;

    if let Some(node) = &runtime_federation {
        node.shutdown();
    }
    if let Some(publisher) = publisher {
        let _ = publisher.await;
    }
    if let Some(task) = host_task {
        let _ = task.await;
    }

    println!(
        "godmesh-server: shutdown solicitado; {} requisição(ões) em voo drenada(s)",
        served.drained
    );
    println!("godmesh-server: encerrado");
    Ok(())
}

/// Recarrega a allowlist e mantém os enlaces de saída com os pares autorizados.
///
/// Duas responsabilidades, nesta ordem a cada ciclo:
///
/// 1. **Allowlist**: recarrega `federation_peer`. Aprovar/revogar pela API passa
///    a valer sem reiniciar o processo. Falha de leitura **não** limpa o
///    registro: erro de banco não pode virar "nenhum par" nem "todos os pares" —
///    mantém-se o último snapshot conhecido e o erro vai para stderr.
/// 2. **Enlace**: disca para cada par `authorized` que ainda não está `ready`,
///    usando os `endpoints` que ele declarou no manifesto (formato `quic://IP:porta`).
///    Sem esta etapa o nó só escutaria: a malha nunca fecharia handshake.
///    Falha de discagem não promove nada — o estado só vira `ready` por
///    handshake verificado (ADR-072 §2, invariante 6).
fn spawn_peers_maintenance(
    pool: sqlx::PgPool,
    node: Arc<FederationNode>,
    interval: Duration,
    mut parar: tokio::sync::watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = parar.changed() => return,
            }
            match federation::list_peers(&pool).await {
                Ok(peers) => node.registry().replace(peers),
                Err(e) => eprintln!(
                    "godmesh-server: recarga da allowlist de pares falhou; mantendo o último \
                     snapshot conhecido: {e:?}"
                ),
            }
            dial_pending_peers(&node).await;
        }
    });
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

/// Disca para os pares autorizados que não têm enlace `ready`.
///
/// `ready` observado é o único motivo para não discar de novo: `unknown` e
/// `rejected` voltam a ser tentados no próximo ciclo, porque a decisão pode ter
/// mudado (aprovação nova) — e a tentativa é barata.
async fn dial_pending_peers(node: &FederationNode) {
    for peer in node.registry().peers() {
        if !peer.is_authorized() || peer.node_id == node.identity().node_id() {
            continue;
        }
        if node
            .links()
            .snapshot(&peer.node_id)
            .is_some_and(|snapshot| snapshot.state == LinkState::Ready)
        {
            continue;
        }
        for endpoint in &peer.endpoints {
            let Some(addr) = parse_quic_endpoint(endpoint) else {
                eprintln!(
                    "godmesh-server: endpoint de {} ignorado (esperado quic://IP:porta): {endpoint}",
                    peer.node_id
                );
                continue;
            };
            match node.dial(addr).await {
                Ok(_) => break,
                Err(e) => eprintln!(
                    "godmesh-server: enlace com {} por {endpoint} não subiu: {e}",
                    peer.node_id
                ),
            }
        }
    }
}

/// `quic://IP:porta` do manifesto → endereço de socket.
///
/// Esquema diferente é `None` (não se disca para o que não foi declarado como
/// QUIC), e o chamador registra o motivo — endereço não é permissão (ADR-072 §2,
/// invariante 8), então nada aqui autoriza nada.
fn parse_quic_endpoint(endpoint: &str) -> Option<SocketAddr> {
    endpoint
        .trim()
        .strip_prefix("quic://")
        .and_then(|addr| addr.parse::<SocketAddr>().ok())
}

// Módulo de testes no fim do arquivo: `clippy::items-after-test-module`
// (negado por `-D clippy::all`) exige que nenhum item venha depois dele.
#[cfg(test)]
mod tests {
    use super::{
        Args, DEFAULT_BIND_ADDR, federation_runtime_from, parse_quic_endpoint, resolve_bind_addr,
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
    fn endpoint_do_manifesto_so_vira_socket_em_quic() {
        let esperado: std::net::SocketAddr = "127.0.0.1:7443".parse().expect("endereço");
        assert_eq!(
            parse_quic_endpoint("quic://127.0.0.1:7443"),
            Some(esperado)
        );
        assert_eq!(parse_quic_endpoint("  quic://127.0.0.1:7443  "), Some(esperado));
        // Esquema diferente ou endereço ilegível não são "quase certos".
        assert_eq!(parse_quic_endpoint("https://127.0.0.1:7443"), None);
        assert_eq!(parse_quic_endpoint("127.0.0.1:7443"), None);
        assert_eq!(parse_quic_endpoint("quic://sem-porta"), None);
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
