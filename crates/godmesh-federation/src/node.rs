//! Nó de federação: endpoint QUIC, aceitação, discagem e heartbeat.
//!
//! Um `FederationNode` é a metade "runtime" da fatia: quem chama `bind` ganha um
//! endpoint que só aceita pares registrados, e quem chama `dial` faz handshake
//! verificado e mantém o enlace vivo com heartbeat.
//!
//! Heartbeat (protocolo desta versão, deliberadamente mínimo):
//!
//! - frame de 12 bytes em stream unidirecional: `GMH1` + contador `u64` BE;
//! - enviado a cada [`FederationConfig::heartbeat_interval`] (padrão 5 s);
//! - ausência por [`FederationConfig::missed_heartbeats`] intervalos (padrão 3,
//!   ou seja 15 s) marca o par `unknown` — nunca `ready`;
//! - heartbeat **não** autoriza nem muda `trust_state`: é evidência de vida.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quinn::{Connection, Endpoint};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::handshake::{self, AcceptedPeer, HandshakeError, Rejection};
use crate::identity::{NodeIdentity, PUBLIC_KEY_LEN, node_id_for};
use crate::link::{LinkSnapshot, LinkState, LinkValidity, PeerLinks};
use crate::manifest::SignedManifest;
use crate::peer::PeerRegistry;
use crate::transport::{self, TransportError};
use crate::{now_unix, transport::ALPN_V1_TEXT};

mod session;

pub use session::{
    OperationalBiStream, OperationalSession, OperationalSessionHandler, SessionError, SessionId,
    SessionLimits,
};

#[cfg(feature = "snapshot-harness")]
pub use session::{SnapshotSessionLimits, StreamOpening};

/// Intervalo de heartbeat padrão: 5 s.
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Heartbeats perdidos tolerados antes de `unknown`: 3 (15 s no padrão).
pub const DEFAULT_MISSED_HEARTBEATS: u32 = 3;
/// Janela de `issued_at` do manifesto: 60 s para trás **e** para frente.
pub const DEFAULT_MANIFEST_WINDOW_SECONDS: i64 = 60;
/// Magic do frame de heartbeat.
pub const HEARTBEAT_MAGIC: [u8; 4] = *b"GMH1";
/// Tamanho exato do frame de heartbeat (magic + contador).
pub const HEARTBEAT_FRAME_LEN: usize = 12;

const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuração do nó local.
#[derive(Debug, Clone)]
pub struct FederationConfig {
    /// Cluster do nó. Par de outro cluster não entra.
    pub cluster: String,
    /// Região declarada. O contrato de manifesto exige região não vazia; é
    /// declaração do nó, não permissão.
    pub region: String,
    /// Versão de software anunciada no manifesto.
    pub software_version: String,
    /// Geração da identidade de federação declarada no manifesto.
    pub epoch: u64,
    /// Intervalo de envio de heartbeat.
    pub heartbeat_interval: Duration,
    /// Quantos intervalos de silêncio tolerar antes de `unknown`.
    pub missed_heartbeats: u32,
    /// Janela aceita em torno de `issued_at` do manifesto (segundos).
    pub manifest_window_seconds: i64,
}

impl FederationConfig {
    /// Configuração com os padrões documentados (5 s / 3 / 60 s).
    ///
    /// O manifesto **não** carrega estado de confiança: quem decide se este nó
    /// entra é o registro do par que o recebe, não a declaração do próprio nó.
    #[must_use]
    pub fn new(
        cluster: impl Into<String>,
        region: impl Into<String>,
        software_version: impl Into<String>,
    ) -> Self {
        Self {
            cluster: cluster.into(),
            region: region.into(),
            software_version: software_version.into(),
            epoch: 1,
            heartbeat_interval: DEFAULT_HEARTBEAT_INTERVAL,
            missed_heartbeats: DEFAULT_MISSED_HEARTBEATS,
            manifest_window_seconds: DEFAULT_MANIFEST_WINDOW_SECONDS,
        }
    }

    /// Ajusta o heartbeat (usado por teste e por ambiente com rede pior).
    #[must_use]
    pub fn with_heartbeat(mut self, interval: Duration, missed_heartbeats: u32) -> Self {
        self.heartbeat_interval = interval;
        self.missed_heartbeats = missed_heartbeats.max(1);
        self
    }

    /// Ajusta a janela de `issued_at` (segundos).
    #[must_use]
    pub fn with_manifest_window(mut self, seconds: i64) -> Self {
        self.manifest_window_seconds = seconds;
        self
    }

    /// Silêncio tolerado antes de `unknown`.
    #[must_use]
    pub fn freshness(&self) -> Duration {
        self.heartbeat_interval
            .saturating_mul(self.missed_heartbeats.max(1))
    }

    /// Manifesto assinado deste nó, com `issued_at` = agora.
    ///
    /// É o **mesmo** [`SignedManifest`] canônico que o ingresso por API
    /// (`POST /v1/federation/join-requests`) aceita: um formato, um verificador.
    /// `node_id` e `public_key` são preenchidos por [`SignedManifest::sign`] a
    /// partir da chave que assina — o chamador não escolhe identidade.
    #[must_use]
    pub fn signed_manifest(&self, identity: &NodeIdentity) -> SignedManifest {
        let mut manifest = SignedManifest {
            node_id: String::new(),
            public_key: String::new(),
            cluster: self.cluster.clone(),
            region: self.region.clone(),
            software_version: self.software_version.clone(),
            alpn: ALPN_V1_TEXT.to_owned(),
            capabilities: Vec::new(),
            endpoints: Vec::new(),
            epoch: self.epoch,
            issued_at: now_unix(),
            signature: String::new(),
        };
        manifest.sign(identity);
        manifest
    }
}

/// Identidade pública derivada da chave usada no mTLS, com o cluster local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkIdentity {
    /// Identificador canônico derivado da chave.
    pub node_id: String,
    /// Chave pública Ed25519.
    pub public_key: [u8; PUBLIC_KEY_LEN],
    /// Cluster no qual a autorização é solicitada.
    pub cluster: String,
}

/// Resultado negativo da autoridade operacional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAuthorizationError {
    /// Autoridade ou vigência insuficiente.
    Denied,
    /// A autoridade durável não pôde ser consultada.
    Unavailable,
    /// A consulta atingiu um limite de prazo da tentativa.
    Timeout,
}

/// Autorização que mantém a proteção até o fim da atualização local.
#[must_use]
pub struct LinkPermit {
    _protection: Box<dyn Send>,
    expires_at: Option<Instant>,
}

impl LinkPermit {
    /// A proteção é liberada no drop. O prazo monotônico é exclusivo.
    pub fn new(protection: Box<dyn Send>, expires_at: Option<Instant>) -> Self {
        Self {
            _protection: protection,
            expires_at,
        }
    }
}

/// Autoridade operacional para os dois nós de um enlace QUIC.
pub trait LinkAuthorizer: Send + Sync {
    /// Consulta o estado durável e retorna com a proteção ainda adquirida.
    ///
    /// O nó cancela a consulta ao invalidar o enlace ou após cinco segundos.
    /// A implementação deve liberar recursos se o futuro for descartado.
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>;

    /// Autoriza uma stream dentro do prazo absoluto de admissão. O transporte
    /// limita a espera. Implementações com SQL podem usar o mesmo deadline
    /// para limitar locks e instruções e devem limpar a transação ao cancelar.
    /// Retorne [LinkAuthorizationError::Timeout] quando esses limites expirarem.
    fn authorize_until<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
        _deadline: tokio::time::Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        self.authorize(local, remote)
    }

    /// Assina as alterações de confiança. Publique assim que a mudança durável
    /// for confirmada. Qualquer notificação ou o fechamento do canal invalida
    /// o enlace. O nó registra a versão corrente antes da primeira consulta.
    fn subscribe(&self) -> watch::Receiver<u64>;
}

/// Falha ao discar para um par.
#[derive(Debug)]
pub enum LinkError {
    /// Transporte não abriu a conexão (endereço, TLS, timeout).
    Connect(String),
    /// O par recusou o handshake, com código de fechamento e motivo sanitizado.
    Rejected {
        /// Código de fechamento QUIC.
        code: u32,
        /// Motivo fixo deste crate.
        reason: &'static str,
    },
    /// Configuração local inválida (ALPN/nome de conexão).
    Transport(TransportError),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "conexão não estabelecida: {e}"),
            Self::Rejected { code, reason } => {
                write!(f, "par recusou o handshake: {reason} (codigo {code:#010X})")
            }
            Self::Transport(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LinkError {}

#[derive(Clone)]
struct NodeShared {
    identity: Arc<NodeIdentity>,
    config: Arc<FederationConfig>,
    registry: Arc<PeerRegistry>,
    links: Arc<PeerLinks>,
    authorizer: Option<Arc<dyn LinkAuthorizer>>,
    session_handler: Option<SessionHandlerConfig>,
}

#[derive(Clone)]
struct SessionHandlerConfig {
    limits: SessionLimits,
    handler: Arc<dyn OperationalSessionHandler>,
    #[cfg(feature = "snapshot-harness")]
    snapshot: Option<SnapshotSessionLimits>,
}

/// Nó de federação com endpoint QUIC ativo.
pub struct FederationNode {
    shared: NodeShared,
    endpoint: Endpoint,
    client_config: quinn::ClientConfig,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl FederationNode {
    /// Abre o endpoint, sobe o laço de aceitação e o monitor de expiração.
    ///
    /// # Errors
    /// [`TransportError`] se o material TLS, a configuração QUIC ou o socket
    /// falharem. Nenhum endpoint é deixado meio-aberto.
    pub fn bind(
        identity: NodeIdentity,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
    ) -> Result<Self, TransportError> {
        Self::bind_with_authorizer(Arc::new(identity), config, registry, addr, None)
    }

    /// Abre o endpoint com identidade compartilhada e autorização opcional.
    ///
    /// # Errors
    /// Os mesmos de [Self::bind].
    pub fn bind_with_authorizer(
        identity: Arc<NodeIdentity>,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
        authorizer: Option<Arc<dyn LinkAuthorizer>>,
    ) -> Result<Self, TransportError> {
        Self::bind_inner(identity, config, registry, addr, authorizer, None)
    }

    /// Abre o endpoint com dispatcher para conexões operacionais autorizadas.
    ///
    /// # Errors
    /// Limites de sessão inválidos ou as mesmas falhas de [Self::bind].
    pub fn bind_with_session_handler(
        identity: Arc<NodeIdentity>,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
        authorizer: Arc<dyn LinkAuthorizer>,
        limits: SessionLimits,
        handler: Arc<dyn OperationalSessionHandler>,
    ) -> Result<Self, TransportError> {
        Self::bind_session(
            identity,
            config,
            registry,
            addr,
            authorizer,
            SessionHandlerConfig {
                limits,
                handler,
                #[cfg(feature = "snapshot-harness")]
                snapshot: None,
            },
        )
    }

    /// Habilita o escape de snapshot para o dispatcher da fixture de laboratório.
    /// O harness deve validar o contexto externo da fixture antes do bind.
    ///
    /// # Errors
    /// Limites incompatíveis ou as falhas de [Self::bind_with_session_handler].
    #[cfg(feature = "snapshot-harness")]
    pub fn bind_with_snapshot_session_handler(
        identity: Arc<NodeIdentity>,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
        authorizer: Arc<dyn LinkAuthorizer>,
        limits: SnapshotSessionLimits,
        handler: Arc<dyn OperationalSessionHandler>,
    ) -> Result<Self, TransportError> {
        if limits.max_session_frame_bytes < limits.a2.max_frame_bytes
            || usize::try_from(limits.max_session_frame_bytes.get()).is_err()
        {
            return Err(TransportError::Quic(
                "teto de snapshot incompatível".to_owned(),
            ));
        }
        Self::bind_session(
            identity,
            config,
            registry,
            addr,
            authorizer,
            SessionHandlerConfig {
                limits: limits.a2,
                handler,
                snapshot: Some(limits),
            },
        )
    }

    fn bind_session(
        identity: Arc<NodeIdentity>,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
        authorizer: Arc<dyn LinkAuthorizer>,
        session_handler: SessionHandlerConfig,
    ) -> Result<Self, TransportError> {
        let limits = session_handler.limits;
        if limits.max_streams_per_connection.get() > tokio::sync::Semaphore::MAX_PERMITS
            || usize::try_from(limits.max_frame_bytes.get()).is_err()
            || limits.attempt_timeout.is_zero()
            || tokio::time::Instant::now()
                .checked_add(limits.attempt_timeout)
                .is_none()
        {
            return Err(TransportError::Quic(
                "limites de sessão inválidos".to_owned(),
            ));
        }
        Self::bind_inner(
            identity,
            config,
            registry,
            addr,
            Some(authorizer),
            Some(session_handler),
        )
    }

    fn bind_inner(
        identity: Arc<NodeIdentity>,
        config: FederationConfig,
        registry: Arc<PeerRegistry>,
        addr: SocketAddr,
        authorizer: Option<Arc<dyn LinkAuthorizer>>,
        session_handler: Option<SessionHandlerConfig>,
    ) -> Result<Self, TransportError> {
        let tls = transport::NodeTls::self_issued(&identity)?;
        let server = transport::server_config(Arc::clone(&registry), &tls)?;
        let client = transport::client_config(Arc::clone(&registry), &tls)?;
        let endpoint =
            transport::bind_server(addr, server).map_err(|e| TransportError::Io(e.to_string()))?;
        let freshness = config.freshness();
        let shared = NodeShared {
            identity,
            config: Arc::new(config),
            registry,
            links: Arc::new(PeerLinks::new(freshness)),
            authorizer,
            session_handler,
        };
        let node = Self {
            shared,
            endpoint,
            client_config: client,
            tasks: Mutex::new(Vec::new()),
        };
        node.start_accept_loop();
        node.start_expiry_monitor();
        Ok(node)
    }

    /// Endereço efetivo do endpoint (porta resolvida quando se pede `:0`).
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.endpoint.local_addr().ok()
    }

    /// Registro de enlaces observados (compartilhável com o serviço de
    /// diagnóstico).
    #[must_use]
    pub fn links(&self) -> Arc<PeerLinks> {
        Arc::clone(&self.shared.links)
    }

    /// Allowlist de pares em uso por este nó.
    ///
    /// É o **mesmo** `Arc` que o verificador de certificado lê, então quem
    /// recarrega a partir do banco (`PeerRegistry::replace`) muda a decisão do
    /// TLS sem recriar o endpoint.
    #[must_use]
    pub fn registry(&self) -> Arc<PeerRegistry> {
        Arc::clone(&self.shared.registry)
    }

    /// Identidade deste nó.
    #[must_use]
    pub fn identity(&self) -> &NodeIdentity {
        &self.shared.identity
    }

    /// Configuração deste nó.
    #[must_use]
    pub fn config(&self) -> &FederationConfig {
        &self.shared.config
    }

    /// Abre conexão verificada com `peer` e mantém o enlace com heartbeat.
    ///
    /// Devolve a fotografia do enlace como observada agora — `ready` só porque o
    /// handshake passou e o ALPN é o esperado.
    ///
    /// # Errors
    /// [`LinkError::Rejected`] quando o par recusa (com código) e
    /// [`LinkError::Connect`] quando a conexão não abre.
    pub async fn dial(&self, peer: SocketAddr) -> Result<LinkSnapshot, LinkError> {
        let connection = self
            .endpoint
            .connect_with(
                self.client_config.clone(),
                peer,
                transport::CONNECT_SERVER_NAME,
            )
            .map_err(|e| LinkError::Connect(e.to_string()))?
            .await
            .map_err(|e| LinkError::Connect(e.to_string()))?;

        match establish(&self.shared, &connection, true).await {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                match error.rejection {
                    // Sem veredito do par: TLS/alerta recusou antes do protocolo.
                    Rejection::Transport => Err(LinkError::Connect(
                        "handshake TLS recusado antes do protocolo".to_owned(),
                    )),
                    rejection => Err(LinkError::Rejected {
                        code: rejection.code(),
                        reason: rejection.reason(),
                    }),
                }
            }
        }
    }

    /// Fecha o endpoint e aborta as tarefas deste nó.
    pub fn shutdown(&self) {
        self.endpoint.close(0u32.into(), b"shutdown");
        if let Ok(mut tasks) = self.tasks.lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
    }

    fn start_accept_loop(&self) {
        let endpoint = self.endpoint.clone();
        let shared = self.shared.clone();
        let handle = tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let shared = shared.clone();
                tokio::spawn(async move {
                    let Ok(connection) = incoming.await else {
                        return;
                    };
                    let _ = establish(&shared, &connection, false).await;
                });
            }
        });
        self.push_task(handle);
    }

    fn start_expiry_monitor(&self) {
        let shared = self.shared.clone();
        let interval = self.shared.config.heartbeat_interval;
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                // Só log/telemetria derivam daqui; o estado já mudou em `expire`.
                let _ = shared.links.expire(Instant::now());
            }
        });
        self.push_task(handle);
    }

    fn push_task(&self, handle: JoinHandle<()>) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push(handle);
        }
    }
}

async fn establish(
    shared: &NodeShared,
    connection: &Connection,
    initiating: bool,
) -> Result<LinkSnapshot, HandshakeError> {
    let result = if let Some(authorizer) = &shared.authorizer {
        establish_operational(shared, connection, initiating, Arc::clone(authorizer)).await
    } else {
        exchange_manifest(shared, connection, initiating, Some(&shared.registry))
            .await
            .map(|accepted| register_ready(shared, connection, accepted))
    };
    if result.is_err()
        && let Some(key) = transport::peer_public_key(connection)
    {
        shared
            .links
            .record_rejection(&node_id_for(&key), shared.authorizer.is_some());
    }
    result
}

async fn exchange_manifest(
    shared: &NodeShared,
    connection: &Connection,
    initiating: bool,
    registry: Option<&PeerRegistry>,
) -> Result<AcceptedPeer, HandshakeError> {
    let identity = &shared.identity;
    let config = &shared.config;
    if initiating {
        handshake::initiate_with_registry(connection, identity, config, registry).await
    } else {
        handshake::accept_with_registry(connection, identity, config, registry).await
    }
}

struct OperationalLink {
    authorizer: Arc<dyn LinkAuthorizer>,
    local: LinkIdentity,
    remote: LinkIdentity,
    alpn: String,
    validity: LinkValidity,
    deadline: watch::Sender<Option<Instant>>,
}

impl OperationalLink {
    fn new(
        shared: &NodeShared,
        connection: &Connection,
        authorizer: Arc<dyn LinkAuthorizer>,
    ) -> Result<Self, Rejection> {
        let alpn = transport::require_alpn(connection).map_err(|_| Rejection::AlpnMismatch)?;
        let remote_key = transport::peer_public_key(connection).ok_or(Rejection::KeyMismatch)?;
        let identity = |public_key| LinkIdentity {
            node_id: node_id_for(&public_key),
            public_key,
            cluster: shared.config.cluster.clone(),
        };
        let mut trust = authorizer.subscribe();
        let _ = *trust.borrow_and_update();
        let (deadline, deadline_rx) = watch::channel(None);
        Ok(Self {
            authorizer,
            local: identity(shared.identity.public_key_bytes()),
            remote: identity(remote_key),
            alpn,
            validity: LinkValidity {
                connection: connection.clone(),
                trust,
                deadline: deadline_rx,
            },
            deadline,
        })
    }

    async fn authorize(&self) -> Result<LinkPermit, Rejection> {
        if !self.validity.is_current(Instant::now()) {
            return Err(Rejection::NotAuthorized);
        }
        let permit = tokio::time::timeout(
            AUTHORIZATION_TIMEOUT,
            self.authorizer.authorize(&self.local, &self.remote),
        )
        .await
        .map_err(|_| Rejection::NotAuthorized)?
        .map_err(|_| Rejection::NotAuthorized)?;
        self.keep_permit(permit)
    }

    fn keep_permit(&self, permit: LinkPermit) -> Result<LinkPermit, Rejection> {
        if let Some(expires_at) = permit.expires_at {
            // Uma nova consulta pode encurtar o enlace. Para estendê-lo,
            // reabra a conexão sob a nova vigência.
            self.deadline.send_if_modified(|deadline| {
                if deadline.is_none_or(|current| expires_at < current) {
                    *deadline = Some(expires_at);
                    return true;
                }
                false
            });
        }
        if !self.validity.is_current(Instant::now()) {
            return Err(Rejection::NotAuthorized);
        }
        Ok(permit)
    }

    async fn invalidated(&self) -> Rejection {
        let mut trust = self.validity.trust.clone();
        let mut deadlines = self.validity.deadline.clone();
        loop {
            let deadline = *deadlines.borrow_and_update();
            tokio::select! {
                biased;
                _ = trust.changed() => return Rejection::Revoked,
                _ = wait_deadline(deadline) => return Rejection::NotAuthorized,
                _ = deadlines.changed() => {}
            }
        }
    }
}

async fn wait_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

async fn establish_operational(
    shared: &NodeShared,
    connection: &Connection,
    initiating: bool,
    authorizer: Arc<dyn LinkAuthorizer>,
) -> Result<LinkSnapshot, HandshakeError> {
    let result = async {
        let link = OperationalLink::new(shared, connection, authorizer)?;
        let snapshot = tokio::select! {
            biased;
            reason = link.invalidated() => return Err(reason),
            _ = connection.closed() => {
                return Err(handshake::peer_verdict(connection).unwrap_or(Rejection::Transport));
            },
            result = async {
                // A autorização precede a primeira operação de protocolo.
                drop(link.authorize().await?);
                let accepted = exchange_manifest(shared, connection, initiating, None)
                    .await
                    .map_err(|error| error.rejection)?;
                let _permit = link.authorize().await?;
                let seen_at = now_unix();
                let mut snapshot = ready_snapshot(connection, accepted, seen_at);
                if !shared.links.mark_operational_ready(
                    &link.remote.node_id, &mut snapshot, link.validity.clone(),
                ) {
                    return Err(Rejection::NotAuthorized);
                }
                shared.registry.touch_operational(&link.remote.node_id, seen_at);
                Ok(snapshot)
            } => result?,
        };
        tokio::spawn(run_operational(shared.clone(), link));
        Ok(snapshot)
    };
    result.await.map_err(|rejection| {
        close_with_rejection(connection, rejection);
        HandshakeError { rejection }
    })
}

async fn run_operational(shared: NodeShared, link: OperationalLink) {
    let link = Arc::new(link);
    let callback = shared.session_handler.clone().map(|config| {
        let link = Arc::clone(&link);
        let links = Arc::clone(&shared.links);
        tokio::spawn(async move {
            if let Some(session) = OperationalSession::new(
                link,
                links,
                config.limits,
                #[cfg(feature = "snapshot-harness")]
                config.snapshot,
            ) {
                config.handler.run(session).await;
            }
        })
    });
    let connection = &link.validity.connection;
    let node_id = &link.remote.node_id;
    let result = tokio::select! {
        biased;
        rejection = link.invalidated() => Err(rejection),
        _ = connection.closed() => Ok(()),
        result = send_heartbeats(connection, shared.config.heartbeat_interval, Some((&link, &shared.links))) => result,
        result = receive_heartbeats(connection, node_id, &shared, Some(&link)) => result,
    };
    if let Err(rejection) = result {
        close_with_rejection(connection, rejection);
    } else if connection.close_reason().is_none() {
        connection.close(0u32.into(), b"enlace encerrado");
    }
    shared.links.close_operational(node_id, connection);
    // O fechamento acorda cancelled(); aguarde a limpeza cooperativa depois
    // de retirar o estado desta conexão, sem bloquear outras sessões.
    if let Some(callback) = callback {
        let _ = callback.await;
    }
}

fn close_with_rejection(connection: &Connection, rejection: Rejection) {
    if connection.close_reason().is_none() {
        connection.close(rejection.code().into(), rejection.reason().as_bytes());
    }
}

fn ready_snapshot(connection: &Connection, accepted: AcceptedPeer, seen_at: i64) -> LinkSnapshot {
    LinkSnapshot {
        state: LinkState::Ready,
        last_seen_at: Some(seen_at),
        rtt_ms: Some(u64::try_from(connection.rtt().as_millis()).unwrap_or(u64::MAX)),
        negotiated_alpn: Some(accepted.alpn),
        peer_version: Some(accepted.software_version),
    }
}

/// Registra o enlace pronto e sobe heartbeat/recepção/vigia de fechamento.
fn register_ready(
    shared: &NodeShared,
    connection: &Connection,
    accepted: AcceptedPeer,
) -> LinkSnapshot {
    let now = Instant::now();
    let seen_at = now_unix();
    let rtt_ms = Some(u64::try_from(connection.rtt().as_millis()).unwrap_or(u64::MAX));
    shared.links.mark_ready(
        &accepted.node_id,
        seen_at,
        rtt_ms,
        accepted.alpn.clone(),
        accepted.software_version.clone(),
        now,
    );
    shared.registry.touch(&accepted.node_id, seen_at);
    spawn_heartbeat(shared, connection, accepted.node_id.clone());
    shared
        .links
        .snapshot(&accepted.node_id)
        .unwrap_or_else(LinkSnapshot::never_seen)
}

fn spawn_heartbeat(shared: &NodeShared, connection: &Connection, node_id: String) {
    let sender_connection = connection.clone();
    let interval = shared.config.heartbeat_interval;
    tokio::spawn(async move {
        let _ = send_heartbeats(&sender_connection, interval, None).await;
    });

    let receiver = connection.clone();
    let shared_receiver = shared.clone();
    let receiver_node = node_id.clone();
    tokio::spawn(async move {
        let _ = receive_heartbeats(&receiver, &receiver_node, &shared_receiver, None).await;
    });

    let watcher = connection.clone();
    let links = Arc::clone(&shared.links);
    tokio::spawn(async move {
        watcher.closed().await;
        links.mark_unknown(&node_id);
    });
}

async fn send_heartbeats(
    connection: &Connection,
    interval: Duration,
    operational: Option<(&OperationalLink, &PeerLinks)>,
) -> Result<(), Rejection> {
    let mut ticker = tokio::time::interval(interval);
    let mut counter: u64 = 0;
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = connection.closed() => return Ok(()),
        }
        if let Some((link, links)) = operational {
            let _permit = link.authorize().await?;
            if !links.operational_is_ready(&link.remote.node_id, connection) {
                return Ok(());
            }
        }
        counter = counter.wrapping_add(1);
        let mut frame = [0u8; HEARTBEAT_FRAME_LEN];
        frame[..HEARTBEAT_MAGIC.len()].copy_from_slice(&HEARTBEAT_MAGIC);
        frame[HEARTBEAT_MAGIC.len()..].copy_from_slice(&counter.to_be_bytes());
        match connection.open_uni().await {
            Ok(mut send) => {
                if send.write_all(&frame).await.is_err() {
                    return Ok(());
                }
                if send.finish().is_err() {
                    return Ok(());
                }
            }
            Err(_) => return Ok(()),
        }
    }
}

async fn receive_heartbeats(
    connection: &Connection,
    node_id: &str,
    shared: &NodeShared,
    operational: Option<&OperationalLink>,
) -> Result<(), Rejection> {
    loop {
        let Ok(mut recv) = connection.accept_uni().await else {
            return Ok(());
        };
        let Ok(frame) = recv.read_to_end(HEARTBEAT_FRAME_LEN).await else {
            continue;
        };
        // Frame fora do formato é ignorado, não vira evidência de vida.
        if frame.len() != HEARTBEAT_FRAME_LEN || frame[..HEARTBEAT_MAGIC.len()] != HEARTBEAT_MAGIC {
            continue;
        }
        if let Some(link) = operational {
            let _permit = link.authorize().await?;
            let seen_at = now_unix();
            if !shared.links.beat_operational(node_id, connection, seen_at) {
                return Ok(());
            }
            shared.registry.touch_operational(node_id, seen_at);
            continue;
        }
        let seen_at = now_unix();
        shared.links.beat(node_id, seen_at, Instant::now());
        shared.registry.touch(node_id, seen_at);
        // Heartbeat não promove estado: se o par foi revogado depois do handshake,
        // quem decide é o registro, não este laço.
        if let Some(record) = shared.registry.record(node_id)
            && !record.is_authorized()
        {
            connection.close(
                Rejection::Revoked.code().into(),
                Rejection::Revoked.reason().as_bytes(),
            );
            shared.links.mark_unknown(node_id);
            return Ok(());
        }
    }
}
