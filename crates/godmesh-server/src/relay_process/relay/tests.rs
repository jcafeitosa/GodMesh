use super::*;
use std::time::Duration;

#[test]
fn request_id_fica_reservado_ate_a_tarefa_recolher() {
    let session = RequestIds::default();
    let other = RequestIds::default();
    let pending = session.claim("id").unwrap();
    assert!(session.claim("id").is_none());
    assert!(session.claim("id").is_none());
    assert!(other.claim("id").is_some());
    drop(pending);
    assert!(session.claim("id").is_some());
}

#[tokio::test]
async fn socket_entrega_apenas_ao_alvo_conectado() {
    tokio::time::timeout(Duration::from_secs(2), async {
        for addr in ["127.0.0.1:0", "[::1]:0"] {
            let target = UdpSocket::bind(addr).await.unwrap();
            let relay = connect_destination(target.local_addr().unwrap())
                .await
                .unwrap();
            assert_eq!(relay.peer_addr().unwrap(), target.local_addr().unwrap());
            relay.send(b"d1").await.unwrap();
            let mut packet = [0; 2];
            let (_, relay_addr) = target.recv_from(&mut packet).await.unwrap();
            assert_eq!(&packet, b"d1");
            let other = UdpSocket::bind(addr).await.unwrap();
            other.send_to(b"no", relay_addr).await.unwrap();
            target.send_to(b"ok", relay_addr).await.unwrap();
            let mut reader = UdpReader::default();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), reader.receive(&relay))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap(),
                b"ok"
            );
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn recusa_correlacionada_usa_fin_e_id_duplicado_reseta_stream() {
    use godmesh_federation::{
        NodeIdentity,
        node::{
            FederationConfig, FederationNode, LinkAuthorizationError, LinkAuthorizer, LinkPermit,
            OperationalSessionHandler, SessionLimits,
        },
        peer::{PeerRecord, PeerRegistry},
        relay_wire::validate_reply,
    };
    use std::{future::Future, pin::Pin};
    use tokio::{sync::mpsc, time::timeout};

    struct Allow(watch::Sender<u64>);

    impl LinkAuthorizer for Allow {
        fn authorize<'a>(
            &'a self,
            _: &'a LinkIdentity,
            _: &'a LinkIdentity,
        ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
        {
            Box::pin(async { Ok(LinkPermit::new(Box::new(()), None)) })
        }

        fn subscribe(&self) -> watch::Receiver<u64> {
            self.0.subscribe()
        }
    }

    struct Sessions(mpsc::UnboundedSender<OperationalSession>);

    impl OperationalSessionHandler for Sessions {
        fn run(
            &self,
            session: OperationalSession,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(async move { self.0.send(session).unwrap() })
        }
    }

    let config = FederationConfig::new("relay-test", "lab", "0.1.0");
    let e = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let r = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let bind = |identity, peer: &NodeIdentity| {
        let registry = Arc::new(PeerRegistry::new());
        registry.insert(PeerRecord::authorized_from_key(
            &peer.public_key_bytes(),
            &config.cluster,
        ));
        let (send, recv) = mpsc::unbounded_channel();
        let node = FederationNode::bind_with_session_handler(
            identity,
            config.clone(),
            registry,
            ([127, 0, 0, 1], 0).into(),
            Arc::new(Allow(watch::channel(0).0)),
            SessionLimits {
                attempt_timeout: OPEN_TIMEOUT,
                max_frame_bytes: CONTROL_LIMIT,
                max_streams_per_connection: 1.try_into().unwrap(),
            },
            Arc::new(Sessions(send)),
        )
        .unwrap();
        (node, recv)
    };
    let (e_node, mut e_sessions) = bind(e.clone(), &r);
    let (r_node, mut r_sessions) = bind(r, &e);

    timeout(Duration::from_secs(5), async {
        e_node.dial(r_node.local_addr().unwrap()).await.unwrap();
        let e_session = e_sessions.recv().await.unwrap();
        let r_session = r_sessions.recv().await.unwrap();
        let request = Open {
            request_id: "0123456789abcdef0123456789abcdef".into(),
            route_id: "d1".into(),
        };
        let mut caller = e_session
            .open_relay(Instant::now() + OPEN_TIMEOUT, CONTROL_LIMIT)
            .await
            .unwrap();
        let control = encode_control(&Control::Open(request.clone()), CONTROL_LIMIT).unwrap();
        caller.write_control(&control).await.unwrap();
        let mut relay = r_session
            .accept_relay(OPEN_TIMEOUT, CONTROL_LIMIT)
            .await
            .unwrap();
        let request_ids = RequestIds::default();
        let (received, pending) = read_request(&mut relay, &request_ids).await.unwrap();
        refuse(&mut relay, received, ErrorCode::QuotaExceeded).await;
        drop(relay);
        let reply = decode_control(&caller.read_control().await.unwrap(), CONTROL_LIMIT).unwrap();
        assert_eq!(validate_reply(&request, &reply, DATAGRAM_LIMIT), Ok(()));
        assert!(matches!(
            reply,
            Control::Refuse(Refuse {
                error: ErrorCode::QuotaExceeded,
                ..
            })
        ));
        caller.expect_fin().await.unwrap();
        drop(caller);

        let mut caller = e_session
            .open_relay(Instant::now() + OPEN_TIMEOUT, CONTROL_LIMIT)
            .await
            .unwrap();
        caller.write_control(&control).await.unwrap();
        let mut relay = r_session
            .accept_relay(OPEN_TIMEOUT, CONTROL_LIMIT)
            .await
            .unwrap();
        assert!(read_request(&mut relay, &request_ids).await.is_none());
        drop(relay);
        assert_eq!(caller.read_control().await, Err(SessionError::Transport));
        drop(pending);
    })
    .await
    .unwrap();
    e_node.shutdown();
    r_node.shutdown();
}

#[tokio::test]
async fn socket_recusa_o_proprio_endpoint_apos_normalizar() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let local = socket.local_addr().unwrap();
    let mapped = SocketAddr::new(
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped()),
        local.port(),
    );
    let error = attach_destination(socket, mapped).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

// Composição da ponte R (`run`/`serve`) com QUIC real e a autoridade PostgreSQL
// descartável do fixture M4-A2. Cobre a barreira de token da SPEC M5 §5.2.
use godmesh_federation::{
    NodeIdentity,
    node::{
        FederationConfig, FederationNode, LinkAuthorizationError, LinkAuthorizer, LinkPermit,
        OperationalSessionHandler, SessionLimits,
    },
    peer::{PeerRecord, PeerRegistry},
    relay_wire::validate_reply,
};
use godmesh_server::{
    federation::JoinWindow,
    portable_join::PortableJoin,
    relay_lab::{Direction, Registry, Route, RouteInstance},
};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicI64, Ordering},
};
use tokio::{sync::mpsc, time::sleep};

use godmesh_server as server;

#[allow(dead_code)]
#[path = "../../../tests/common/replica_fixture.rs"]
mod fixture;

const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";
const ROUTE_ID: &str = "d1";
const OPEN_CLOCK_SECS: i64 = 110;
// Mesmo prazo de expiração usado pelo teste de autoridade do relay_lab.
const EXPIRED_CLOCK_SECS: i64 = 500;
const PAUSE: Duration = Duration::from_millis(100);
const QUIET: Duration = Duration::from_millis(200);
// Prazo de 100 ms do I/O de cada envio autorizado (SPEC M5 §5.2).
const SEND_DEADLINE: Duration = Duration::from_millis(100);
const WAIT: Duration = Duration::from_secs(2);
const CASE: Duration = Duration::from_secs(20);

struct Composition {
    fixture: fixture::ReceiverFixture,
    e: Arc<NodeIdentity>,
    admission: Arc<RelayAdmission>,
    clock: Arc<AtomicI64>,
    e_session: OperationalSession,
    e_node: FederationNode,
    r_node: FederationNode,
    dst: UdpSocket,
}

struct Allow(watch::Sender<u64>);

impl LinkAuthorizer for Allow {
    fn authorize<'a>(
        &'a self,
        _: &'a LinkIdentity,
        _: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async { Ok(LinkPermit::new(Box::new(()), None)) })
    }

    fn subscribe(&self) -> watch::Receiver<u64> {
        self.0.subscribe()
    }
}

struct Sessions(mpsc::UnboundedSender<OperationalSession>);

impl OperationalSessionHandler for Sessions {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move { self.0.send(session).unwrap() })
    }
}

// Lado R real: cada sessão operacional entra no laço de aceite de `relay::run`.
struct Relay(Arc<RelayAdmission>);

impl OperationalSessionHandler for Relay {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let admission = Arc::clone(&self.0);
        Box::pin(async move { super::run(session, admission, None).await })
    }
}

fn bind<H: OperationalSessionHandler + 'static>(
    identity: Arc<NodeIdentity>,
    peer: &NodeIdentity,
    config: &FederationConfig,
    handler: H,
) -> FederationNode {
    let registry = Arc::new(PeerRegistry::new());
    registry.insert(PeerRecord::authorized_from_key(
        &peer.public_key_bytes(),
        &config.cluster,
    ));
    FederationNode::bind_with_session_handler(
        identity,
        config.clone(),
        registry,
        ([127, 0, 0, 1], 0).into(),
        Arc::new(Allow(watch::channel(0).0)),
        SessionLimits {
            attempt_timeout: OPEN_TIMEOUT,
            max_frame_bytes: CONTROL_LIMIT,
            max_streams_per_connection: 4.try_into().unwrap(),
        },
        Arc::new(handler),
    )
    .unwrap()
}

fn pin(id: &NodeIdentity) -> LinkIdentity {
    LinkIdentity {
        node_id: id.node_id().to_owned(),
        public_key: id.public_key_bytes(),
        cluster: fixture::CLUSTER.into(),
    }
}

async fn composition(label: &str) -> Composition {
    let f = fixture::ReceiverFixture::new(label)
        .await
        .expect("composição M5 exige PostgreSQL descartável");
    let e = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let pins = [&*e, &*f.origin, &*f.receiver]
        .map(|id| format!("{}:{}", id.node_id(), id.public_key_hex()))
        .join(";");
    let config = fixture::configured_portable(&pins);
    let portable = Arc::new(PortableJoin::new(
        config.clone(),
        f.pool.clone(),
        f.receiver.clone(),
    ));
    let source = PortableJoin::new(config, f.pool.clone(), e.clone());
    portable
        .import_genesis(&source.local_manifest().unwrap(), JoinWindow::default())
        .await
        .unwrap();

    let dst = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let route = Route {
        id: ROUTE_ID.into(),
        requester: pin(&e),
        destination: pin(&f.origin),
        service: "fixture".into(),
        endpoint: dst.local_addr().unwrap(),
    };
    let clock = Arc::new(AtomicI64::new(OPEN_CLOCK_SECS));
    let admission = Arc::new(
        RelayAdmission::new(
            Registry::new(pin(&f.receiver), vec![route], []).unwrap(),
            portable,
            f.bundle.clone(),
            Instant::now(),
        )
        .unwrap()
        .with_clock({
            let clock = Arc::clone(&clock);
            Arc::new(move || Duration::from_secs(clock.load(Ordering::SeqCst) as u64))
        }),
    );
    admission.set_allowed(ROUTE_ID, true).unwrap();

    let config = FederationConfig::new(fixture::CLUSTER, "relay-test", "0.1.0");
    let (e_tx, mut e_sessions) = mpsc::unbounded_channel();
    let e_node = bind(Arc::clone(&e), &f.receiver, &config, Sessions(e_tx));
    let r_node = bind(
        Arc::clone(&f.receiver),
        &e,
        &config,
        Relay(Arc::clone(&admission)),
    );
    e_node.dial(r_node.local_addr().unwrap()).await.unwrap();
    let e_session = e_sessions.recv().await.unwrap();
    Composition {
        fixture: f,
        e,
        admission,
        clock,
        e_session,
        e_node,
        r_node,
        dst,
    }
}

/// Abre a stream, envia o pedido e lê o aceite. Retorna o token de R para E.
async fn open_until_accept(c: &Composition) -> (RelayOpening, [u8; 16]) {
    let request = Open {
        request_id: REQUEST_ID.into(),
        route_id: ROUTE_ID.into(),
    };
    let mut opening = c
        .e_session
        .open_relay(Instant::now() + OPEN_TIMEOUT, CONTROL_LIMIT)
        .await
        .unwrap();
    let control = encode_control(&Control::Open(request.clone()), CONTROL_LIMIT).unwrap();
    opening.write_control(&control).await.unwrap();
    let reply = decode_control(&opening.read_control().await.unwrap(), CONTROL_LIMIT).unwrap();
    assert_eq!(validate_reply(&request, &reply, DATAGRAM_LIMIT), Ok(()));
    let Control::Accept(accept) = reply else {
        panic!("aceite esperado: {reply:?}");
    };
    let mut token = [0; 16];
    hex::decode_to_slice(accept.ready_token, &mut token).unwrap();
    (opening, token)
}

/// Reset ou fechamento do R. `Timeout` (silêncio até o prazo) não conta.
fn interrupted(error: SessionError) -> bool {
    matches!(error, SessionError::Transport | SessionError::Closed)
}

async fn assert_no_udp(dst: &UdpSocket) {
    let mut buffer = [0; 2048];
    assert!(
        tokio::time::timeout(QUIET, dst.recv_from(&mut buffer))
            .await
            .is_err(),
        "datagrama UDP inesperado no destino"
    );
}

/// Espera a reserva anterior ser recolhida pela própria ponte R.
async fn reopen(c: &Composition) -> RouteInstance {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match c.admission.open(ROUTE_ID, &pin(&c.e), Instant::now()).await {
                Ok(instance) => return instance,
                Err(AdmissionError::QuotaExceeded) => sleep(Duration::from_millis(10)).await,
                Err(error) => panic!("vaga não liberada: {error:?}"),
            }
        }
    })
    .await
    .expect("vaga liberada pela ponte R")
}

#[derive(Clone, Copy)]
enum Fault {
    Revoke,
    Expire,
}

async fn token_apos_falha_nao_produz_efeito(fault: Fault, label: &str) {
    tokio::time::timeout(CASE, async {
        let c = composition(label).await;
        let (mut opening, token) = open_until_accept(&c).await;
        // Pausa entre o aceite e o token: nada ainda pode sair para o destino.
        sleep(PAUSE).await;
        assert_no_udp(&c.dst).await;
        match fault {
            Fault::Revoke => {
                // Autoridade durável muda no PostgreSQL sem o commit com broadcast M1C.
                let mut trust = c.admission.trust_changes();
                let before = *trust.borrow_and_update();
                let rows = sqlx::query(
                    "UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1 AND cluster = $2",
                )
                .bind(c.e.node_id())
                .bind(fixture::CLUSTER)
                .execute(&c.fixture.pool)
                .await
                .unwrap()
                .rows_affected();
                assert_eq!(rows, 1, "par E ausente em federation_peer");
                assert!(!trust.has_changed().unwrap(), "revogação não pode emitir broadcast M1C");
                assert_eq!(*trust.borrow(), before);
            }
            Fault::Expire => c.clock.store(EXPIRED_CLOCK_SECS, Ordering::SeqCst),
        }
        // A falha tem de ser observável pela abertura antes do token.
        assert!(matches!(
            c.admission.open(ROUTE_ID, &pin(&c.e), Instant::now()).await,
            Err(AdmissionError::AuthorityBlocked)
        ));
        // E envia o token antes do prazo: R precisa processá-lo e interromper a stream.
        assert!(Instant::now() < opening.deadline());
        opening
            .send_token(token)
            .await
            .expect("token enviado antes do prazo");
        match opening.into_forwarding(DATAGRAM_LIMIT) {
            Ok((mut send, mut recv, _)) => {
                // Silêncio até o prazo (Timeout) reprova: a interrupção vem por reset/fechamento.
                let interruption = recv.read_datagram(Instant::now() + WAIT).await.unwrap_err();
                assert!(
                    interrupted(interruption),
                    "R deve interromper por reset/fechamento: {interruption:?}"
                );
                // Escrita só é tolerada depois que E já observou a interrupção de R.
                let _ = send.write_datagram(b"frame", Instant::now() + SEND_DEADLINE).await;
            }
            Err(interruption) => assert!(
                interrupted(interruption),
                "R deve interromper por reset/fechamento: {interruption:?}"
            ),
        }
        assert_no_udp(&c.dst).await;
        let origin = c.fixture.origin.node_id();
        for direction in [Direction::ToDestination, Direction::FromDestination] {
            let usage = c.admission.usage(Some(origin), direction);
            assert_eq!((usage.debited_frames, usage.debited_bytes), (0, 0));
        }
        match fault {
            Fault::Revoke => {
                // Restaura a linha da disposable DB para liberar a vaga sem broadcast.
                sqlx::query(
                    "UPDATE federation_peer SET trust_state = 'authorized' WHERE node_id = $1 AND cluster = $2",
                )
                .bind(c.e.node_id())
                .bind(fixture::CLUSTER)
                .execute(&c.fixture.pool)
                .await
                .unwrap();
            }
            Fault::Expire => c.clock.store(OPEN_CLOCK_SECS, Ordering::SeqCst),
        }
        let instance = reopen(&c).await;
        instance.cancel();
        instance.recollect();
        c.e_node.shutdown();
        c.r_node.shutdown();
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "exige PostgreSQL 16 descartável (GODMESH_TEST_DATABASE_URL ou DATABASE_URL); executar com --ignored"]
async fn composicao_revogacao_apos_aceite_nao_produz_udp_nem_frame() {
    token_apos_falha_nao_produz_efeito(Fault::Revoke, "relay_comp_revoke").await;
}

#[tokio::test]
#[ignore = "exige PostgreSQL 16 descartável (GODMESH_TEST_DATABASE_URL ou DATABASE_URL); executar com --ignored"]
async fn composicao_validade_vencida_apos_aceite_nao_produz_udp_nem_frame() {
    token_apos_falha_nao_produz_efeito(Fault::Expire, "relay_comp_expire").await;
}

#[tokio::test]
#[ignore = "exige PostgreSQL 16 descartável (GODMESH_TEST_DATABASE_URL ou DATABASE_URL); executar com --ignored"]
async fn composicao_token_com_primeiro_frame_debita_uma_vez() {
    tokio::time::timeout(CASE, async {
        let c = composition("relay_comp_positive").await;
        let (mut opening, token) = open_until_accept(&c).await;
        let frame = b"primeiro-frame".to_vec();
        opening.send_token(token).await.unwrap();
        let (mut send, _recv, _) = opening.into_forwarding(DATAGRAM_LIMIT).unwrap();
        send.write_datagram(&frame, Instant::now() + SEND_DEADLINE)
            .await
            .unwrap();
        let mut buffer = [0; 2048];
        let (len, _) = tokio::time::timeout(WAIT, c.dst.recv_from(&mut buffer))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&buffer[..len], frame.as_slice());
        assert_no_udp(&c.dst).await;
        let usage = c
            .admission
            .usage(Some(c.fixture.origin.node_id()), Direction::ToDestination);
        // Contabiliza o frame com o prefixo de 4 bytes, uma única vez.
        assert_eq!(usage.debited_frames, 1);
        assert_eq!(usage.debited_bytes, frame.len() as u64 + 4);
        c.e_node.shutdown();
        c.r_node.shutdown();
    })
    .await
    .unwrap();
}
