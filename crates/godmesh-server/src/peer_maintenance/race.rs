//! Barreiras do mantenedor na publicação de um refresh com QUIC real.
use super::{Candidate, Driver, PeerRecord, Scheduler, SocketAddr, transport::Node};
use godmesh_federation::{
    NodeIdentity,
    link::LinkState,
    node::{
        DialAttempt, DialTarget, FederationConfig, FederationNode, LinkAuthorizationError,
        LinkAuthorizer, LinkIdentity, LinkPermit, OperationalSession, OperationalSessionHandler,
        SessionLimits,
    },
    peer::PeerRegistry,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    runtime::Handle,
    sync::{Notify, mpsc, watch},
    task::block_in_place,
    time::{Instant, timeout},
};

struct Gate {
    changes: watch::Sender<u64>,
    calls: AtomicUsize,
    pause: bool,
    entered: Notify,
    release: Notify,
}

impl Gate {
    fn new(pause: bool) -> Self {
        Self {
            changes: watch::channel(0).0,
            calls: AtomicUsize::new(0),
            pause,
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}

impl LinkAuthorizer for Gate {
    fn authorize<'a>(
        &'a self,
        _: &'a LinkIdentity,
        _: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            if self.pause && self.calls.fetch_add(1, Ordering::Relaxed) == 1 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(LinkPermit::new(Box::new(()), None))
        })
    }

    fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }
}

struct SessionSink(mpsc::UnboundedSender<OperationalSession>);

impl OperationalSessionHandler for SessionSink {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let _ = self.0.send(session);
        })
    }
}

struct AtPublication {
    node: Node,
    probe: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl Driver for AtPublication {
    type Attempt = DialAttempt;
    fn publish(&self, records: &[PeerRecord]) {
        self.node.publish(records);
        if let Some(probe) = self.probe.borrow_mut().take() {
            probe();
        }
    }
    fn eligible_now(&self, candidate: &Candidate) -> bool {
        self.node.eligible_now(candidate)
    }
    fn start(&self, candidate: &Candidate, addr: SocketAddr) -> Option<DialAttempt> {
        self.node.start(candidate, addr)
    }
}

struct Pair {
    caller: Arc<FederationNode>,
    remote: FederationNode,
    gate: Arc<Gate>,
    record: PeerRecord,
    caller_sessions: mpsc::UnboundedReceiver<OperationalSession>,
    remote_sessions: mpsc::UnboundedReceiver<OperationalSession>,
}

impl Pair {
    fn new(pause: bool) -> Self {
        let config = FederationConfig::new("fixture", "lab", "1.0.0");
        let a = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let b = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let gate = Arc::new(Gate::new(pause));
        let bind = |identity, opposite: &NodeIdentity, authorizer: Arc<Gate>| {
            let registry = Arc::new(PeerRegistry::new());
            registry.insert(PeerRecord::authorized_from_key(
                &opposite.public_key_bytes(),
                &config.cluster,
            ));
            let (send, recv) = mpsc::unbounded_channel();
            let node = FederationNode::bind_with_session_handler(
                identity,
                config.clone(),
                registry,
                ([127, 0, 0, 1], 0).into(),
                authorizer,
                SessionLimits {
                    attempt_timeout: Duration::from_secs(2),
                    max_frame_bytes: 4096.try_into().unwrap(),
                    max_streams_per_connection: 4.try_into().unwrap(),
                },
                Arc::new(SessionSink(send)),
            )
            .unwrap();
            (node, recv)
        };
        let (caller, caller_sessions) = bind(a.clone(), &b, gate.clone());
        let (remote, remote_sessions) = bind(b.clone(), &a, Arc::new(Gate::new(false)));
        let mut record = PeerRecord::authorized_from_key(&b.public_key_bytes(), &config.cluster);
        record.endpoints = vec![format!("quic://{}", remote.local_addr().unwrap())];
        Self {
            caller: Arc::new(caller),
            remote,
            gate,
            record,
            caller_sessions,
            remote_sessions,
        }
    }

    fn driver(&self) -> AtPublication {
        AtPublication {
            node: Node(self.caller.clone()),
            probe: RefCell::new(None),
        }
    }
}

async fn next_session(
    receiver: &mut mpsc::UnboundedReceiver<OperationalSession>,
) -> OperationalSession {
    timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}

fn at_publication<T>(f: impl Future<Output = T>) -> T {
    block_in_place(|| {
        Handle::current().block_on(async { timeout(Duration::from_secs(2), f).await.unwrap() })
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_removido_apos_tls_cancela_antes_de_publicar() {
    let mut pair = Pair::new(true);
    let driver = pair.driver();
    let mut schedule = Scheduler::new(&driver, pair.caller.identity().node_id());
    let (_stop, shutdown) = watch::channel(false);
    schedule.refreshed(vec![pair.record.clone()]);
    schedule.dispatch(&shutdown);
    timeout(Duration::from_secs(2), pair.gate.entered.notified())
        .await
        .unwrap();
    let remote_session = next_session(&mut pair.remote_sessions).await;
    let mut callbacks = pair.caller_sessions;
    let gate = pair.gate.clone();
    let caller = pair.caller.clone();
    let mut updated = pair.record.clone();
    updated.endpoints.clear(); // A chave e a autoridade continuam válidas.
    let peer = updated.node_id.clone();
    *driver.probe.borrow_mut() = Some(Box::new(move || {
        assert!(
            caller
                .registry()
                .record(&peer)
                .unwrap()
                .endpoints
                .is_empty()
        );
        gate.release.notify_one();
        at_publication(async {
            tokio::select! {
                callback = callbacks.recv() => panic!("callback tardio após publicação: {:?}", callback.map(|s| s.id())),
                () = remote_session.cancelled() => {}
            }
        });
        assert!(callbacks.try_recv().is_err());
        assert_ne!(
            caller.links().snapshot_or_never_seen(&peer).state,
            LinkState::Ready
        );
        assert!(
            caller
                .links()
                .snapshot_or_never_seen(&peer)
                .last_seen_at
                .is_none()
        );
    }));
    schedule.refreshed(vec![updated]);
    timeout(Duration::from_secs(2), schedule.reap_one())
        .await
        .unwrap();
    pair.caller.shutdown();
    pair.remote.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_apos_promocao_invalida_original_e_preserva_substituta() {
    let mut pair = Pair::new(false);
    let driver = pair.driver();
    let mut schedule = Scheduler::new(&driver, pair.caller.identity().node_id());
    let (_stop, shutdown) = watch::channel(false);
    schedule.refreshed(vec![pair.record.clone()]);
    schedule.dispatch(&shutdown);
    let first = next_session(&mut pair.caller_sessions).await;
    let mut replacement = pair
        .caller
        .start_dial(DialTarget {
            node_id: pair.record.node_id.clone(),
            public_key: pair.remote.identity().public_key_bytes(),
            addr: pair.remote.local_addr().unwrap(),
        })
        .unwrap();
    let replacement_permit = replacement.wait().await.unwrap();
    let second = next_session(&mut pair.caller_sessions).await;
    assert_ne!(first.id(), second.id());
    let caller = pair.caller.clone();
    let mut updated = pair.record;
    updated.endpoints.clear();
    let peer = updated.node_id.clone();
    *driver.probe.borrow_mut() = Some(Box::new(move || {
        at_publication(first.cancelled());
        assert!(at_publication(second.open_bi(Instant::now() + Duration::from_secs(1))).is_ok());
        assert_eq!(
            caller.links().snapshot_or_never_seen(&peer).state,
            LinkState::Ready
        );
    }));
    schedule.refreshed(vec![updated]);
    timeout(Duration::from_secs(2), schedule.reap_one())
        .await
        .unwrap();
    replacement_permit.invalidate();
    pair.caller.shutdown();
    pair.remote.shutdown();
}
