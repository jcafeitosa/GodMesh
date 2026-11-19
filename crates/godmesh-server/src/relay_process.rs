//! Entrada do ensaio M5 com quatro processos e bases descartáveis.
mod blocked_path;
mod edge;
mod relay;
mod udp;

use std::{
    future::Future,
    io::Read,
    net::SocketAddr,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    pin::Pin,
    str::FromStr,
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};

use godmesh_federation::{
    identity::{NodeIdentity, node_id_for},
    node::{
        DialCancellation, DialTarget, FederationNode, LinkIdentity, OperationalSession,
        OperationalSessionHandler, SessionLimits,
    },
};
use godmesh_server::{
    config_bundle::BundleService,
    portable_join::PortableJoin,
    relay_lab::{Registry, RelayAdmission, Route, normalize_endpoint},
};
use serde::Deserialize;
use sqlx::{PgPool, postgres::PgConnectOptions};
use tokio::{sync::watch, task::JoinHandle, time::Instant};

use crate::replica_runtime::ReplicaRuntime;

const CONTROL_LIMIT: NonZeroU32 = NonZeroU32::new(512).unwrap();
const OPEN_TIMEOUT: Duration = Duration::from_secs(2);
const SERVICE: &str = "m4a.metadata.aceite";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Peer {
    node_id: String,
    public_key: String,
    endpoint: SocketAddr,
}

impl Peer {
    fn pin(&self, cluster: &str) -> Result<LinkIdentity, String> {
        let mut public_key = [0; 32];
        hex::decode_to_slice(&self.public_key, &mut public_key).map_err(report)?;
        if self.public_key != hex::encode(public_key) || self.node_id != node_id_for(&public_key) {
            return Err("pin da fixture M5 inválido".into());
        }
        Ok(LinkIdentity {
            node_id: self.node_id.clone(),
            public_key,
            cluster: cluster.into(),
        })
    }

    fn target(&self, cluster: &str, endpoint: SocketAddr) -> Result<DialTarget, String> {
        let pin = self.pin(cluster)?;
        Ok(DialTarget {
            node_id: pin.node_id,
            public_key: pin.public_key,
            addr: loopback(endpoint)?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteSelection {
    route_id: String,
    destination: Peer,
    direct: SocketAddr,
    shim: SocketAddr,
    allowed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Database {
    name: String,
    port: u16,
    system_identifier: String,
    oid: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    generation: NonZeroU64,
    cluster: String,
    local_node_id: String,
    database: Database,
    edge: Peer,
    relay: Peer,
    routes: [RouteSelection; 2],
}

impl Selection {
    fn read() -> Result<Self, String> {
        let root = std::env::var_os("GODMESH_TEST_TEMP")
            .map(PathBuf::from)
            .ok_or("diretório descartável M5 ausente")?;
        if !root
            .file_name()
            .and_then(|x| x.to_str())
            .is_some_and(|name| name.starts_with("gm-m1c."))
            || !root.join("pg/PG_VERSION").is_file()
        {
            return Err("controle M5 exige a fixture PostgreSQL descartável".into());
        }
        let file = std::env::var_os("GODMESH_TEST_RELAY_CONTEXT")
            .map(PathBuf::from)
            .ok_or("seleção da fixture M5 ausente")?
            .canonicalize()
            .map_err(report)?;
        let root = root.canonicalize().map_err(report)?;
        if file.parent() != Some(root.as_path()) {
            return Err("contexto M5 fora do diretório descartável".into());
        }
        let mut bytes = Vec::new();
        let limit = 16_384;
        std::fs::File::open(&file)
            .map_err(report)?
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(report)?;
        if bytes.len() as u64 > limit {
            return Err("contexto M5 excede o limite".into());
        }
        serde_json::from_slice(&bytes).map_err(report)
    }

    pub(super) fn verify_url(&self, url: &str) -> Result<(), String> {
        let config = PgConnectOptions::from_str(url).map_err(|_| "URL PostgreSQL inválida")?;
        if config.get_host() != "127.0.0.1"
            || config.get_port() != self.database.port
            || config.get_database() != Some(self.database.name.as_str())
            || config.get_username() != "gm_m1c"
            || config.get_socket().is_some()
            || url.contains(['?', '#'])
            || !matches!(
                self.database.name.as_str(),
                "gm_m1c_a" | "gm_m1c_r" | "gm_m1c_b" | "gm_m1c_c"
            )
        {
            return Err("banco fora da alocação M5".into());
        }
        Ok(())
    }

    pub(super) async fn verify_allocation(&self, pool: &PgPool) -> Result<(), String> {
        let (system, oid): (String, i64) = sqlx::query_as(
            "SELECT (pg_control_system()).system_identifier::text, oid::bigint
             FROM pg_database WHERE datname = current_database()",
        )
        .fetch_one(pool)
        .await
        .map_err(report)?;
        if system != self.database.system_identifier || oid != self.database.oid {
            return Err("alocação PostgreSQL M5 divergente".into());
        }
        Ok(())
    }

    pub(super) fn prepare(
        self,
        identity: &NodeIdentity,
        endpoint: SocketAddr,
        portable: Arc<PortableJoin>,
        m3: Arc<BundleService>,
        replica: Option<Arc<ReplicaRuntime>>,
        m3_changes: Option<watch::Receiver<u64>>,
    ) -> Result<Arc<Runtime>, String> {
        let cluster = &self.cluster;
        if identity.node_id() != self.local_node_id || portable.cluster() != cluster {
            return Err("identidade local diverge do contexto M5".into());
        }
        let edge = self.edge.pin(cluster)?;
        let relay = self.relay.pin(cluster)?;
        let edge_addr = loopback(self.edge.endpoint)?;
        let relay_addr = loopback(self.relay.endpoint)?;
        let mut reserved = vec![edge_addr, relay_addr];
        let mut route_entries = Vec::new();
        let mut edge_routes = Vec::new();
        let mut known_peers = vec![(&self.edge, edge_addr), (&self.relay, relay_addr)];
        for selection in &self.routes {
            let destination = selection.destination.pin(cluster)?;
            let dst_addr = loopback(selection.destination.endpoint)?;
            let direct = loopback(selection.direct)?;
            let shim = loopback(selection.shim)?;
            reserved.extend([direct, shim]);
            known_peers.push((&selection.destination, dst_addr));
            route_entries.push(Route {
                id: selection.route_id.clone(),
                requester: edge.clone(),
                destination,
                service: SERVICE.into(),
                endpoint: dst_addr,
            });
            edge_routes.push(edge::Route {
                id: selection.route_id.clone(),
                direct: selection.destination.target(cluster, direct)?,
                shim,
                generation: self.generation,
            });
        }
        let mut unique = std::collections::HashSet::new();
        if reserved.iter().any(|addr| !unique.insert(*addr)) {
            return Err(
                "endpoints de listener, shim e caminho direto M5 devem ser distintos".into(),
            );
        }
        let mut ids = std::collections::HashSet::new();
        if known_peers
            .iter()
            .any(|(peer, _)| !ids.insert(&peer.node_id))
        {
            return Err("os quatro nós M5 precisam de identidades distintas".into());
        }
        let (local, local_addr) = known_peers
            .into_iter()
            .find(|(peer, _)| peer.node_id == self.local_node_id)
            .ok_or("identidade local não pertence à topologia M5")?;
        let bind = loopback(endpoint)?;
        if local.pin(cluster)?.public_key != identity.public_key_bytes() || bind != local_addr {
            return Err("listener M5 diverge do pin ou do endereço do contexto".into());
        }
        let registry = Registry::new(relay.clone(), route_entries, reserved).map_err(report)?;
        let mode = if self.local_node_id == edge.node_id {
            let [first, second] = edge_routes
                .try_into()
                .map_err(|_| "faltam rotas da fixture M5")?;
            Mode::Edge {
                relay,
                target: self.relay.target(cluster, relay_addr)?,
                routes: Box::new([first, second]),
            }
        } else if self.local_node_id == relay.node_id {
            if replica.is_some() {
                return Err("o processo de relay não admite réplica local".into());
            }
            let admission =
                RelayAdmission::new(registry, portable, m3, Instant::now()).map_err(report)?;
            for route in &self.routes {
                admission
                    .set_allowed(&route.route_id, route.allowed)
                    .map_err(report)?;
            }
            Mode::Relay {
                edge,
                admission: Arc::new(admission),
                m3_changes,
            }
        } else {
            Mode::Destination
        };
        if !matches!(mode, Mode::Relay { .. }) && replica.is_none() {
            return Err("E e os destinos exigem réplica M4 da fixture".into());
        }
        Ok(Arc::new(Runtime {
            mode,
            bind,
            node: OnceLock::new(),
            replica,
        }))
    }
}

enum Mode {
    Edge {
        relay: LinkIdentity,
        target: DialTarget,
        routes: Box<[edge::Route; 2]>,
    },
    Relay {
        edge: LinkIdentity,
        admission: Arc<RelayAdmission>,
        m3_changes: Option<watch::Receiver<u64>>,
    },
    Destination,
}

pub(super) struct Runtime {
    mode: Mode,
    bind: SocketAddr,
    node: OnceLock<Weak<FederationNode>>,
    replica: Option<Arc<ReplicaRuntime>>,
}

impl Runtime {
    pub(super) fn bind_addr(&self) -> SocketAddr {
        self.bind
    }

    pub(super) fn limits(&self) -> SessionLimits {
        self.replica.as_deref().map_or(
            SessionLimits {
                max_frame_bytes: CONTROL_LIMIT,
                max_streams_per_connection: NonZeroUsize::new(4).unwrap(),
                attempt_timeout: OPEN_TIMEOUT,
            },
            |replica| {
                let mut limits = replica.session_limits();
                limits.max_streams_per_connection = limits
                    .max_streams_per_connection
                    .max(NonZeroUsize::new(4).unwrap());
                limits
            },
        )
    }

    pub(super) fn attach(&self, node: &Arc<FederationNode>) {
        let _ = self.node.set(Arc::downgrade(node));
    }

    pub(super) fn dial_relay(
        &self,
        node: Arc<FederationNode>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Option<JoinHandle<()>> {
        let Mode::Edge { target, .. } = &self.mode else {
            return None;
        };
        let target = target.clone();
        Some(tokio::spawn(async move {
            let Ok(mut dial) = node.start_dial(target) else {
                return;
            };
            let promoted = tokio::select! {
                _ = async { let _ = shutdown.wait_for(|stopped| *stopped).await; } => false,
                result = dial.wait() => result.is_ok(),
            };
            if promoted {
                let _ = shutdown.wait_for(|stopped| *stopped).await;
            }
            if let DialCancellation::AlreadyPromoted(link) = dial.cancel() {
                link.invalidate();
            }
            let _ = dial.wait().await;
        }))
    }
}

impl OperationalSessionHandler for Runtime {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            match &self.mode {
                Mode::Edge { relay, routes, .. } if same_pin(relay, session.remote()) => {
                    let Some(node) = self.node.get().and_then(Weak::upgrade) else {
                        return;
                    };
                    tokio::join!(
                        edge_route(&node, &session, &routes[0]),
                        edge_route(&node, &session, &routes[1]),
                    );
                }
                Mode::Relay {
                    edge,
                    admission,
                    m3_changes,
                } if same_pin(edge, session.remote()) => {
                    relay::run(session, Arc::clone(admission), m3_changes.clone()).await;
                }
                _ => {
                    if let Some(replica) = &self.replica {
                        replica.run(session).await;
                    }
                }
            }
        })
    }
}

async fn edge_route(node: &FederationNode, session: &OperationalSession, route: &edge::Route) {
    if let Err(error) = edge::run(node, session, route).await {
        eprintln!(
            "event=relay_edge_unavailable route={} reason={error}",
            route.id
        );
    }
}

fn same_pin(expected: &LinkIdentity, remote: &LinkIdentity) -> bool {
    expected.node_id == remote.node_id
        && expected.public_key == remote.public_key
        && expected.cluster == remote.cluster
}

fn loopback(addr: SocketAddr) -> Result<SocketAddr, String> {
    let normalized = normalize_endpoint(addr).map_err(report)?;
    if !normalized.ip().is_loopback() {
        return Err("listener, destino e shim M5 exigem loopback".into());
    }
    Ok(normalized)
}

fn report(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[test]
#[ignore = "executado somente pela fixture M5 E/R/D1/D2"]
fn process() {
    let context = Selection::read().expect("seleção de laboratório M5");
    let mut config =
        crate::Config::from_env(crate::Args::Serve).expect("configuração da fixture M5");
    config.relay_process = Some(context);
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime M5");
    tokio
        .block_on(crate::run(config))
        .expect("processo da fixture M5");
}
