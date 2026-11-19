use super::*;
use tokio::sync::{mpsc, oneshot, watch};

struct Fake {
    starts: mpsc::UnboundedSender<Start>,
}
struct Start {
    id: String,
    addr: SocketAddr,
    finish: oneshot::Sender<bool>,
    cancelled: oneshot::Receiver<()>,
}
struct Attempt {
    finish: oneshot::Receiver<bool>,
    cancel: Option<oneshot::Sender<()>>,
}
impl Pending for Attempt {
    fn cancel(&mut self) {
        if let Some(send) = self.cancel.take() {
            let _ = send.send(());
        }
    }
    async fn finish(&mut self) -> bool {
        (&mut self.finish).await.unwrap_or(false)
    }
}
impl Driver for Fake {
    type Attempt = Attempt;
    fn publish(&self, _: &[PeerRecord]) {}
    fn eligible_now(&self, _: &Candidate) -> bool {
        true
    }
    fn start(&self, peer: &Candidate, addr: SocketAddr) -> Option<Attempt> {
        let (finish, rx) = oneshot::channel();
        let (cancel, cancelled) = oneshot::channel();
        self.starts
            .send(Start {
                id: peer.id.clone(),
                addr,
                finish,
                cancelled,
            })
            .ok()?;
        Some(Attempt {
            finish: rx,
            cancel: Some(cancel),
        })
    }
}

#[tokio::test]
async fn duas_vagas_fifo_mesmo_com_refresh_e_par_pendente() {
    let (send, mut starts) = mpsc::unbounded_channel();
    let (_stop, shutdown) = watch::channel(false);
    let driver = Fake { starts: send };
    let mut scheduler = Scheduler::new(&driver, "local");
    scheduler.refreshed(vec![record("B", 1), record("C", 2), record("D", 3)]);
    scheduler.dispatch(&shutdown);
    let b = starts.try_recv().unwrap();
    let c = starts.try_recv().unwrap();
    assert_eq!((&b.id[..], &c.id[..]), ("B", "C"));
    assert!(starts.try_recv().is_err());
    scheduler.refreshed(vec![
        record("B", 1),
        record("C", 2),
        record("D", 3),
        record("A", 4),
    ]);
    scheduler.dispatch(&shutdown);
    assert!(starts.try_recv().is_err());
    c.finish.send(true).unwrap();
    scheduler.reap_one().await;
    scheduler.dispatch(&shutdown);
    let d = starts.try_recv().unwrap();
    assert_eq!((d.id.as_str(), d.addr.port()), ("D", 3));
    assert!(starts.try_recv().is_err());
    // B ainda está pendente quando C e D obtiveram suas vagas.
    assert!(!b.finish.is_closed());
}

fn record(id: &str, port: u16) -> PeerRecord {
    use godmesh_federation::peer::TrustState;
    PeerRecord {
        peer_id: id.to_owned(),
        node_id: id.to_owned(),
        public_key: "11".repeat(32),
        cluster: "fixture".into(),
        region: "lab".into(),
        trust_state: TrustState::Authorized,
        endpoints: vec![format!("quic://127.0.0.1:{port}")],
        software_version: "1".into(),
        capabilities: Vec::new(),
        epoch: 1,
        last_seen_at: 0,
    }
}

#[tokio::test]
async fn fallback_sequencial_e_troca_de_registro_aguardam_recolhimento() {
    let (send, mut starts) = mpsc::unbounded_channel();
    let (_stop, shutdown) = watch::channel(false);
    let driver = Fake { starts: send };
    let mut schedule = Scheduler::new(&driver, "local");
    let mut b = record("B", 101);
    b.endpoints.insert(1, "https://127.0.0.1:999".into());
    b.endpoints.push("quic://127.0.0.1:102".into());
    schedule.refreshed(vec![b, record("C", 103)]);
    schedule.dispatch(&shutdown);
    let first = starts.try_recv().unwrap();
    let c = starts.try_recv().unwrap();
    assert_eq!((first.id.as_str(), first.addr.port()), ("B", 101));
    assert_eq!(c.id, "C");
    first.finish.send(false).unwrap();
    schedule.reap_one().await;
    schedule.dispatch(&shutdown);
    let second = starts.try_recv().unwrap();
    assert_eq!((second.id.as_str(), second.addr.port()), ("B", 102));

    let mut revoked = record("C", 103);
    revoked.trust_state = godmesh_federation::peer::TrustState::Revoked;
    schedule.refreshed(vec![record("B", 105), revoked, record("D", 104)]);
    second.cancelled.await.unwrap();
    c.cancelled.await.unwrap();
    schedule.dispatch(&shutdown);
    assert!(starts.try_recv().is_err());
    second.finish.send(false).unwrap();
    schedule.reap_one().await;
    schedule.dispatch(&shutdown);
    let d = starts.try_recv().unwrap();
    assert_eq!((d.id.as_str(), d.addr.port()), ("D", 104));
    c.finish.send(false).unwrap();
    schedule.reap_one().await;
    schedule.dispatch(&shutdown);
    let new = starts.try_recv().unwrap();
    assert_eq!((new.id.as_str(), new.addr.port()), ("B", 105));
}

#[tokio::test]
async fn refresh_com_vagas_ocupadas_falha_fechado_e_shutdown_ignora_resultado_tardio() {
    use std::time::Duration;
    let (send, mut starts) = mpsc::unbounded_channel();
    let (stop, shutdown) = watch::channel(false);
    let (requests, mut incoming) =
        mpsc::unbounded_channel::<oneshot::Sender<Result<Vec<PeerRecord>, ()>>>();
    let task = tokio::spawn(async move {
        let driver = Fake { starts: send };
        maintain(
            &driver,
            "local",
            Duration::from_millis(10),
            shutdown,
            move |_| {
                let (tx, rx) = oneshot::channel();
                requests.send(tx).unwrap();
                async move { rx.await.unwrap() }
            },
        )
        .await;
    });

    let test = async {
        incoming
            .recv()
            .await
            .unwrap()
            .send(Ok(vec![record("B", 1), record("C", 2), record("D", 3)]))
            .unwrap();
        let b = starts.recv().await.unwrap();
        let c = starts.recv().await.unwrap();
        incoming.recv().await.unwrap().send(Err(())).unwrap();
        let late = incoming.recv().await.unwrap();
        c.finish.send(false).unwrap();
        assert!(starts.try_recv().is_err());
        stop.send(true).unwrap();
        b.cancelled.await.unwrap();
        late.send(Ok(vec![record("late", 7)])).unwrap();
        b.finish.send(false).unwrap();
        task.await.unwrap();
        assert!(starts.recv().await.is_none());
    };
    tokio::time::timeout(Duration::from_secs(2), test)
        .await
        .unwrap();
}

#[tokio::test]
async fn shutdown_do_refresh_sql_descarta_sessao_com_lock_externo_retido() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::time::Duration;
    let Some(url) = std::env::var("GODMESH_TEST_DATABASE_URL").ok() else {
        return;
    };
    let base: PgConnectOptions = url.parse().unwrap();
    let control = PgPoolOptions::new()
        .connect_with(base.clone())
        .await
        .unwrap();
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(version.parse::<u32>().unwrap() / 10_000, 16);
    let name = format!("quic_refresh_{}", std::process::id());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&control)
        .await
        .unwrap();
    let options = base.database(&name);
    let observer = PgPoolOptions::new()
        .connect_with(options.clone())
        .await
        .unwrap();
    migrate_fixture(&observer).await;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(550))
        .connect_with(options)
        .await
        .unwrap();
    let old_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut gate = observer.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE federation_peer IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let (close, stop) = watch::channel(false);
    let (send, mut starts) = mpsc::unbounded_channel();
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        let driver = Fake { starts: send };
        maintain(&driver, "local", Duration::from_secs(1), stop, |stopping| {
            refresh::read(&worker_pool, stopping)
        })
        .await;
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT $2 = ANY(pg_blocking_pids($1))")
                .bind(old_pid)
                .bind(blocker)
                .fetch_one(&observer)
                .await
                .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    close.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(starts.recv().await.is_none());
    let current: (i32, String) =
        sqlx::query_as("SELECT pg_backend_pid(),current_setting('statement_timeout')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(current.0, old_pid);
    assert_eq!(current.1, "0");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT $2 = ANY(pg_blocking_pids($1))")
                .bind(old_pid)
                .bind(blocker)
                .fetch_one(&observer)
                .await
                .unwrap();
            if !waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.rollback().await.unwrap();
    pool.close().await;
    observer.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE {name}")))
        .execute(&control)
        .await
        .unwrap();
}

async fn migrate_fixture(pool: &sqlx::PgPool) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    crate::migrate::migrate(pool, &directory).await.unwrap();
}

#[test]
fn endpoint_do_manifesto_so_vira_socket_em_quic() {
    let esperado: std::net::SocketAddr = "127.0.0.1:7443".parse().expect("endereço");
    assert_eq!(parse_endpoint("quic://127.0.0.1:7443"), Some(esperado));
    assert_eq!(parse_endpoint("  quic://127.0.0.1:7443  "), Some(esperado));
    // Esquema diferente ou endereço ilegível não são "quase certos".
    assert_eq!(parse_endpoint("https://127.0.0.1:7443"), None);
    assert_eq!(parse_endpoint("127.0.0.1:7443"), None);
    assert_eq!(parse_endpoint("quic://sem-porta"), None);
}

#[tokio::test]
async fn quic_real_c_promove_enquanto_b_ignora_datagrama() {
    use godmesh_federation::{
        NodeIdentity,
        link::LinkState,
        node::{FederationConfig, FederationNode},
        peer::{PeerRegistry, TrustState},
    };
    use std::{sync::Arc, time::Duration};
    let config = FederationConfig::new("fixture", "lab", "1.0.0");
    let source = NodeIdentity::generate_in_memory().unwrap();
    let b_identity = NodeIdentity::generate_in_memory().unwrap();
    let c_identity = NodeIdentity::generate_in_memory().unwrap();
    let bind: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let b_socket = tokio::net::UdpSocket::bind(bind).await.unwrap();
    let peers_c = Arc::new(PeerRegistry::new());
    let make = |key: &NodeIdentity, addr: SocketAddr| {
        let mut record = PeerRecord::from_manifest(&config.signed_manifest(key), 0);
        record.trust_state = TrustState::Authorized;
        record.endpoints = vec![format!("quic://{addr}")];
        record
    };
    peers_c.insert(make(&source, bind));
    let mut peer_c = make(&c_identity, bind);
    let c = FederationNode::bind(c_identity, config.clone(), peers_c, bind).unwrap();
    peer_c.endpoints = vec![format!("quic://{}", c.local_addr().unwrap())];
    let a = Arc::new(
        FederationNode::bind(source, config.clone(), Arc::new(PeerRegistry::new()), bind).unwrap(),
    );
    let peer_b = make(&b_identity, b_socket.local_addr().unwrap());
    let links = a.links();
    let mut policy = vec![peer_b.clone()];
    let (stop, shutdown) = watch::channel(false);
    let (asks, mut next) = mpsc::unbounded_channel::<oneshot::Sender<Vec<PeerRecord>>>();
    let worker = tokio::spawn(async move {
        let driver = super::transport::Node(a.clone());
        maintain(
            &driver,
            a.identity().node_id(),
            Duration::from_millis(10),
            shutdown,
            move |mut stop| {
                let (send, recv) = oneshot::channel();
                asks.send(send).unwrap();
                async move {
                    tokio::select! {
                        _ = stop.wait_for(|stop| *stop) => Err(()),
                        outcome = recv => outcome.map_err(|_| ()),
                    }
                }
            },
        )
        .await;
        a.shutdown();
    });
    let exercise = async {
        next.recv().await.unwrap().send(policy.clone()).unwrap();
        let mut datagram = [0; 2048];
        let bytes = b_socket.recv(&mut datagram).await.unwrap();
        assert!(bytes > 0);
        policy.push(peer_c.clone());
        next.recv().await.unwrap().send(policy).unwrap();
        loop {
            if links
                .snapshot(&peer_c.node_id)
                .is_some_and(|snap| snap.state == LinkState::Ready)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            !links
                .snapshot(&peer_b.node_id)
                .is_some_and(|snap| snap.state == LinkState::Ready)
        );
    };
    tokio::time::timeout(Duration::from_secs(2), exercise)
        .await
        .unwrap();
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    c.shutdown();
}

#[tokio::test]
async fn retry_espera_refresh_e_respeita_chegadas() {
    let (send, mut starts) = mpsc::unbounded_channel();
    let (_stop, shutdown) = watch::channel(false);
    let driver = Fake { starts: send };
    let mut scheduler = Scheduler::new(&driver, "local");
    scheduler.refreshed(vec![record("B", 1), record("C", 2), record("D", 3)]);
    scheduler.dispatch(&shutdown);
    let b = starts.try_recv().unwrap();
    let c = starts.try_recv().unwrap();
    c.finish.send(false).unwrap();
    scheduler.reap_one().await;
    scheduler.dispatch(&shutdown);
    let d = starts.try_recv().unwrap();
    assert_eq!(d.id, "D");
    d.finish.send(true).unwrap();
    scheduler.reap_one().await;
    scheduler.dispatch(&shutdown);
    assert!(starts.try_recv().is_err());
    scheduler.refreshed(vec![record("A", 4), record("B", 1), record("C", 2)]);
    scheduler.dispatch(&shutdown);
    assert_eq!(starts.try_recv().unwrap().id, "C");
    assert!(!b.finish.is_closed());
}
