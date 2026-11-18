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

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quinn::{Connection, Endpoint};
use tokio::task::JoinHandle;

use crate::handshake::{self, AcceptedPeer, Rejection};
use crate::identity::{NodeIdentity, node_id_for};
use crate::link::{LinkSnapshot, PeerLinks};
use crate::manifest::SignedManifest;
use crate::peer::PeerRegistry;
use crate::transport::{self, TransportError};
use crate::{now_unix, transport::ALPN_V1_TEXT};

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
        let tls = transport::NodeTls::self_issued(&identity)?;
        let server = transport::server_config(Arc::clone(&registry), &tls)?;
        let client = transport::client_config(Arc::clone(&registry), &tls)?;
        let endpoint =
            transport::bind_server(addr, server).map_err(|e| TransportError::Io(e.to_string()))?;
        let freshness = config.freshness();
        let shared = NodeShared {
            identity: Arc::new(identity),
            config: Arc::new(config),
            registry,
            links: Arc::new(PeerLinks::new(freshness)),
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

        match handshake::initiate(
            &connection,
            &self.shared.identity,
            &self.shared.config,
            &self.shared.registry,
        )
        .await
        {
            Ok(accepted) => Ok(self.register_ready(&connection, accepted)),
            Err(error) => {
                self.record_rejection(&connection);
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
                    match handshake::accept(
                        &connection,
                        &shared.identity,
                        &shared.config,
                        &shared.registry,
                    )
                    .await
                    {
                        Ok(accepted) => {
                            register_ready(&shared, &connection, accepted);
                        }
                        Err(_) => {
                            record_rejection(&shared, &connection);
                        }
                    }
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

    fn register_ready(&self, connection: &Connection, accepted: AcceptedPeer) -> LinkSnapshot {
        register_ready(&self.shared, connection, accepted)
    }

    fn record_rejection(&self, connection: &Connection) {
        record_rejection(&self.shared, connection);
    }

    fn push_task(&self, handle: JoinHandle<()>) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push(handle);
        }
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

fn record_rejection(shared: &NodeShared, connection: &Connection) {
    // A identidade do par pode ser ilegível (manifesto inválido); nesse caso não
    // se inventa um `node_id` para registrar — o estado fica sem dono.
    if let Some(key) = transport::peer_public_key(connection) {
        shared.links.mark_rejected(&node_id_for(&key));
    }
}

fn spawn_heartbeat(shared: &NodeShared, connection: &Connection, node_id: String) {
    let sender_connection = connection.clone();
    let interval = shared.config.heartbeat_interval;
    tokio::spawn(async move { send_heartbeats(sender_connection, interval).await });

    let receiver = connection.clone();
    let shared_receiver = shared.clone();
    let receiver_node = node_id.clone();
    tokio::spawn(async move {
        receive_heartbeats(receiver, receiver_node, shared_receiver).await;
    });

    let watcher = connection.clone();
    let links = Arc::clone(&shared.links);
    tokio::spawn(async move {
        watcher.closed().await;
        links.mark_unknown(&node_id);
    });
}

async fn send_heartbeats(connection: Connection, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    let mut counter: u64 = 0;
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = connection.closed() => return,
        }
        counter = counter.wrapping_add(1);
        let mut frame = [0u8; HEARTBEAT_FRAME_LEN];
        frame[..HEARTBEAT_MAGIC.len()].copy_from_slice(&HEARTBEAT_MAGIC);
        frame[HEARTBEAT_MAGIC.len()..].copy_from_slice(&counter.to_be_bytes());
        match connection.open_uni().await {
            Ok(mut send) => {
                if send.write_all(&frame).await.is_err() {
                    return;
                }
                if send.finish().is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

async fn receive_heartbeats(connection: Connection, node_id: String, shared: NodeShared) {
    loop {
        let Ok(mut recv) = connection.accept_uni().await else {
            return;
        };
        let Ok(frame) = recv.read_to_end(HEARTBEAT_FRAME_LEN).await else {
            continue;
        };
        // Frame fora do formato é ignorado, não vira evidência de vida.
        if frame.len() != HEARTBEAT_FRAME_LEN || frame[..HEARTBEAT_MAGIC.len()] != HEARTBEAT_MAGIC {
            continue;
        }
        let seen_at = now_unix();
        shared.links.beat(&node_id, seen_at, Instant::now());
        shared.registry.touch(&node_id, seen_at);
        // Heartbeat não promove estado: se o par foi revogado depois do handshake,
        // quem decide é o registro, não este laço.
        if let Some(record) = shared.registry.record(&node_id)
            && !record.is_authorized()
        {
            connection.close(
                Rejection::Revoked.code().into(),
                Rejection::Revoked.reason().as_bytes(),
            );
            shared.links.mark_unknown(&node_id);
            return;
        }
    }
}