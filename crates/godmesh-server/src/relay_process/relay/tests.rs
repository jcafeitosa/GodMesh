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
