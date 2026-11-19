use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, Extension};
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::{get, put};
use godmesh_federation::NodeIdentity;
use godmesh_federation::transport::{self, NodeTls};
use godmesh_server::config_https::{HttpsClient, HttpsError, HttpsLimits, PeerIdentity, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::rustls::{self, pki_types::ServerName};
use tokio_rustls::{TlsConnector, client::TlsStream};

const PATH: &str = "/v1/federation/config";

fn limits() -> HttpsLimits {
    HttpsLimits {
        max_handshakes: 1,
        max_connections: 2,
        handshake_timeout: Duration::from_millis(400),
        header_timeout: Duration::from_millis(250),
        max_body_bytes: 16,
        body_timeout: Duration::from_millis(300),
        pull_timeout: Duration::from_secs(2),
    }
}

async fn read(
    Extension(peer): Extension<PeerIdentity>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> (StatusCode, HeaderMap, &'static str) {
    assert_eq!(addr, peer.remote_addr);
    let etag = "\"frame\"";
    let mut response = HeaderMap::new();
    response.insert(header::ETAG, etag.parse().unwrap());
    if headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|v| v == etag)
    {
        return (StatusCode::NOT_MODIFIED, response, "");
    }
    (StatusCode::OK, response, "frame")
}

#[tokio::test]
async fn get_autentica_as_duas_pontas_e_recusa_pins_divergentes() {
    let node = NodeIdentity::generate_in_memory().unwrap();
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let outsider = NodeIdentity::generate_in_memory().unwrap();
    let pin = node.public_key_bytes();
    let caller_key = caller.public_key_bytes();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, quit) = watch::channel(false);
    let router = Router::new().route(
        PATH,
        get(
            move |Extension(peer): Extension<PeerIdentity>,
                  ConnectInfo(remote): ConnectInfo<SocketAddr>,
                  headers: HeaderMap| async move {
                assert_eq!(peer.public_key, caller_key);
                assert_eq!(peer.node_id, godmesh_federation::node_id_for(&caller_key));
                read(Extension(peer), ConnectInfo(remote), headers).await
            },
        ),
    );
    let task = tokio::spawn(async move {
        serve(listener, router, &node, vec![caller_key], limits(), quit).await
    });
    let client = HttpsClient::new(&caller, limits()).unwrap();
    let authority = addr.to_string().parse().unwrap();
    let target = PATH.parse().unwrap();
    assert!(matches!(
        client.get(addr, [1; 32], &authority, &target, None).await,
        Err(HttpsError::Tls)
    ));
    let other = HttpsClient::new(&outsider, limits()).unwrap();
    assert!(matches!(
        other.get(addr, pin, &authority, &target, None).await,
        Err(HttpsError::Tls)
    ));
    let ok = client
        .get(addr, pin, &authority, &target, None)
        .await
        .unwrap();
    assert_eq!(ok.peer.node_id, godmesh_federation::node_id_for(&pin));
    assert_eq!(ok.peer.remote_addr, addr);
    assert_eq!(ok.peer.public_key, pin);
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(ok.body, b"frame");
    let cached = client
        .get(addr, pin, &authority, &target, ok.headers.get(header::ETAG))
        .await
        .unwrap();
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);
    assert!(cached.body.is_empty());
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn handshake_ocupado_expira_e_libera_vaga() {
    let node = NodeIdentity::generate_in_memory().unwrap();
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let pin = node.public_key_bytes();
    let caller_key = caller.public_key_bytes();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, quit) = watch::channel(false);
    let task = tokio::spawn(async move {
        serve(
            listener,
            Router::new().route(PATH, get(|| async { "ok" })),
            &node,
            vec![caller_key],
            limits(),
            quit,
        )
        .await
    });
    let mut holding = TcpStream::connect(addr).await.unwrap();
    let mut rejected = TcpStream::connect(addr).await.unwrap();
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(200), rejected.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), holding.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let client = HttpsClient::new(&caller, limits()).unwrap();
    let response = client
        .get(
            addr,
            pin,
            &addr.to_string().parse().unwrap(),
            &PATH.parse().unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

fn client_config(identity: &NodeIdentity, pin: [u8; 32]) -> rustls::ClientConfig {
    let tls = NodeTls::self_issued(identity).unwrap();
    transport::http1_client_tls(&tls, pin).unwrap()
}

async fn raw_tls(
    addr: SocketAddr,
    config: rustls::ClientConfig,
) -> std::io::Result<TlsStream<TcpStream>> {
    TlsConnector::from(Arc::new(config))
        .connect(
            ServerName::try_from(transport::CONNECT_SERVER_NAME).unwrap(),
            TcpStream::connect(addr).await?,
        )
        .await
}

async fn read_http(stream: &mut TlsStream<TcpStream>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let result = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut bytes))
        .await
        .expect("conexão HTTP terminou no prazo");
    if let Err(error) = result {
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }
    bytes
}

async fn start(
    router: Router,
    limits: HttpsLimits,
    client_pin: [u8; 32],
) -> (
    SocketAddr,
    [u8; 32],
    watch::Sender<bool>,
    tokio::task::JoinHandle<Result<(), HttpsError>>,
) {
    let node = NodeIdentity::generate_in_memory().unwrap();
    let pin = node.public_key_bytes();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, quit) = watch::channel(false);
    let task = tokio::spawn(async move {
        serve(listener, router, &node, vec![client_pin], limits, quit).await
    });
    (addr, pin, stop, task)
}

#[derive(Debug)]
struct NoClientCertificate;

impl rustls::client::ResolvesClientCert for NoClientCertificate {
    fn resolve(
        &self,
        _roots: &[&[u8]],
        _schemes: &[rustls::SignatureScheme],
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        None
    }

    fn has_certs(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn exige_certificado_e_usa_identidade_da_sessao() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let key = caller.public_key_bytes();
    let router = Router::new().route(
        PATH,
        get(
            move |Extension(peer): Extension<PeerIdentity>, headers: HeaderMap| async move {
                assert_eq!(peer.node_id, godmesh_federation::node_id_for(&key));
                assert_eq!(headers["x-node-id"], "claimed-peer");
                "authenticated"
            },
        ),
    );
    let (addr, pin, stop, task) = start(router, limits(), key).await;
    let mut certless = client_config(&caller, pin);
    certless.client_auth_cert_resolver = Arc::new(NoClientCertificate);
    if let Ok(mut stream) = raw_tls(addr, certless).await {
        let mut buffer = [0];
        let denied = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buffer))
            .await
            .unwrap();
        assert!(denied.is_err(), "TLS sem certificado precisa emitir alerta");
    }
    let mut stream = raw_tls(addr, client_config(&caller, pin)).await.unwrap();
    assert_eq!(
        stream.get_ref().1.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_3)
    );
    assert_eq!(
        stream.get_ref().1.alpn_protocol(),
        Some(b"http/1.1".as_slice())
    );
    stream.write_all(
        b"GET /v1/federation/config HTTP/1.1\r\nHost: fixture\r\nX-Node-Id: claimed-peer\r\n\r\n",
    ).await.unwrap();
    let response = read_http(&mut stream).await;
    assert!(response.starts_with(b"HTTP/1.1 200 "));
    assert!(response.ends_with(b"authenticated"));
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn titulo_incompleto_ocupa_conexao_ate_o_prazo() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let mut limits = limits();
    limits.max_connections = 1;
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let router = Router::new().route(
        PATH,
        get(move || async move {
            observed.fetch_add(1, Ordering::Relaxed);
            "ok"
        }),
    );
    let (addr, pin, stop, task) = start(router, limits, caller.public_key_bytes()).await;
    let mut holding = raw_tls(addr, client_config(&caller, pin)).await.unwrap();
    holding
        .write_all(b"GET /v1/federation/config HTTP/1.1\r\nHost: fixture\r\n")
        .await
        .unwrap();
    let mut rejected = TcpStream::connect(addr).await.unwrap();
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(200), rejected.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let response = read_http(&mut holding).await;
    assert!(!response.starts_with(b"HTTP/1.1 200 "));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    let client = HttpsClient::new(&caller, limits).unwrap();
    let ok = client
        .get(
            addr,
            pin,
            &addr.to_string().parse().unwrap(),
            &PATH.parse().unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(ok.body, b"ok");
    assert_eq!(count.load(Ordering::Relaxed), 1);
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn limita_corpo_streaming_e_preserva_o_limite_exato() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let router = Router::new().route(PATH, put(|body: axum::body::Bytes| async move { body }));
    let (addr, pin, stop, task) = start(router, limits(), caller.public_key_bytes()).await;
    for (chunks, expected) in [
        (
            b"10\r\n1234567890123456\r\n0\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 ".as_slice(),
        ),
        (b"10\r\n1234567890123456\r\n1\r\na\r\n", b"HTTP/1.1 413 "),
        (b"5\r\nabc", b"HTTP/1.1 408 "),
    ] {
        let mut stream = raw_tls(addr, client_config(&caller, pin)).await.unwrap();
        stream.write_all(b"PUT /v1/federation/config HTTP/1.1\r\nHost: fixture\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await.unwrap();
        stream.write_all(chunks).await.unwrap();
        let response = read_http(&mut stream).await;
        assert!(response.starts_with(expected), "status inesperado");
    }
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn get_limita_enquadramento_corpo_e_prazo_total_sem_redirecionar() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let publisher = NodeIdentity::generate_in_memory().unwrap();
    let pin = publisher.public_key_bytes();
    let tls = NodeTls::self_issued(&publisher).unwrap();
    let server =
        Arc::new(transport::http1_server_tls(&tls, vec![caller.public_key_bytes()]).unwrap());
    let cases: &[(&[u8], Option<HttpsError>)] = &[
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 16\r\n\r\n1234567890123456",
            None,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\n\r\n",
            Some(HttpsError::BodyTooLarge),
        ),
        (
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 17\r\n\r\n",
            Some(HttpsError::InvalidBody),
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\na",
            Some(HttpsError::InvalidBody),
        ),
        (b"HTTP/1.1 200 OK\r\n\r\n", Some(HttpsError::InvalidBody)),
        (
            b"HTTP/1.1 302 Found\r\nLocation: /outra-rota\r\nContent-Length: 0\r\n\r\n",
            None,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n",
            Some(HttpsError::Timeout),
        ),
    ];
    for (index, &(response, expected)) in cases.iter().enumerate() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(server.clone());
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            // Nenhuma outra tentativa de conexão é atendida.
            drop(listener);
            let mut tls = acceptor.accept(tcp).await.unwrap();
            let mut bytes = [0; 1024];
            let read = tls.read(&mut bytes).await.unwrap();
            assert!(bytes[..read].starts_with(b"GET /v1/federation/config HTTP/1.1\r\n"));
            tls.write_all(response).await.unwrap();
            tls.flush().await.unwrap();
            if expected == Some(HttpsError::Timeout) {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let _ = tls.shutdown().await;
        });
        let mut limits = limits();
        if expected == Some(HttpsError::Timeout) {
            limits.pull_timeout = Duration::from_millis(150);
            limits.body_timeout = Duration::from_secs(2);
        }
        let client = HttpsClient::new(&caller, limits).unwrap();
        let result = client
            .get(
                addr,
                pin,
                &addr.to_string().parse().unwrap(),
                &PATH.parse().unwrap(),
                None,
            )
            .await;
        if let Some(error) = expected {
            assert_eq!(result.err(), Some(error), "cenário {index}");
        } else {
            let response = result.unwrap();
            if index == 0 {
                assert_eq!(response.body, b"1234567890123456");
            } else {
                assert_eq!(response.status, StatusCode::FOUND);
            }
        }
        task.await.unwrap();
    }
}

#[tokio::test]
async fn shutdown_encerra_conexao_autenticada_e_handshake_em_andamento() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let mut limits = limits();
    limits.handshake_timeout = Duration::from_secs(5);
    limits.header_timeout = Duration::from_secs(5);
    let (addr, pin, stop, task) = start(Router::new(), limits, caller.public_key_bytes()).await;
    let mut authenticated = raw_tls(addr, client_config(&caller, pin)).await.unwrap();
    let mut pending = TcpStream::connect(addr).await.unwrap();
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(read_http(&mut authenticated).await.is_empty());
    let mut byte = [0];
    let received = tokio::time::timeout(Duration::from_secs(1), pending.read(&mut byte))
        .await
        .unwrap();
    assert!(received.is_err() || received.unwrap() == 0);
}

#[tokio::test]
async fn get_distingue_interrupcao_de_rede_e_excedente_depois_do_frame() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let publisher = NodeIdentity::generate_in_memory().unwrap();
    let pin = publisher.public_key_bytes();
    let tls = NodeTls::self_issued(&publisher).unwrap();
    let config =
        Arc::new(transport::http1_server_tls(&tls, vec![caller.public_key_bytes()]).unwrap());
    for (head, suffix, close_notify, expected) in [
        (b"".as_slice(), None, false, HttpsError::Io),
        (
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 5\r\n\r\na".as_slice(),
            None,
            false,
            HttpsError::InvalidBody,
        ),
        (
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 5\r\n\r\na".as_slice(),
            None,
            true,
            HttpsError::InvalidBody,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\na".as_slice(),
            None,
            false,
            HttpsError::Io,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\na".as_slice(),
            Some(b"b"),
            true,
            HttpsError::InvalidBody,
        ),
        (
            b"HTTP/1.1 304 Not Modified\r\n\r\n".as_slice(),
            Some(b"b"),
            true,
            HttpsError::InvalidBody,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(config.clone());
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut stream = acceptor.accept(tcp).await.unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            stream.write_all(head).await.unwrap();
            stream.flush().await.unwrap();
            if let Some(suffix) = suffix {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = stream.write_all(suffix).await;
            }
            if close_notify {
                let _ = stream.shutdown().await;
            }
        });
        let client = HttpsClient::new(&caller, limits()).unwrap();
        let result = client
            .get(
                addr,
                pin,
                &addr.to_string().parse().unwrap(),
                &PATH.parse().unwrap(),
                None,
            )
            .await;
        task.await.unwrap();
        assert_eq!(result.err(), Some(expected));
    }
}

#[tokio::test]
async fn escrita_parada_da_resposta_libera_cota_no_prazo_de_corpo() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let mut limits = limits();
    limits.max_connections = 1;
    limits.body_timeout = Duration::from_millis(250);
    let started = Arc::new(tokio::sync::Notify::new());
    let router = Router::new().route(
        PATH,
        get({
            let started = started.clone();
            move |headers: HeaderMap| {
                let started = started.clone();
                async move {
                    if headers.contains_key("x-fixture-large") {
                        started.notify_one();
                        return vec![0; 16 * 1024 * 1024];
                    }
                    b"ok".to_vec()
                }
            }
        }),
    );
    let (addr, pin, stop, task) = start(router, limits, caller.public_key_bytes()).await;
    let mut holding = raw_tls(addr, client_config(&caller, pin)).await.unwrap();
    holding
        .write_all(
            b"GET /v1/federation/config HTTP/1.1\r\nHost: fixture\r\nX-Fixture-Large: yes\r\n\r\n",
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    let mut rejected = TcpStream::connect(addr).await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(180), rejected.read(&mut [0]))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    let client = HttpsClient::new(&caller, limits).unwrap();
    let response = client
        .get(
            addr,
            pin,
            &addr.to_string().parse().unwrap(),
            &PATH.parse().unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    drop(holding);
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn exige_alpn_http1_e_sessoes_tls13_completas() {
    let caller = NodeIdentity::generate_in_memory().unwrap();
    let (addr, pin, stop, task) = start(
        Router::new().route(PATH, get(|| async { "ok" })),
        limits(),
        caller.public_key_bytes(),
    )
    .await;
    for alpn in [vec![], vec![b"h2".to_vec()]] {
        let mut config = client_config(&caller, pin);
        config.alpn_protocols = alpn;
        if let Ok(mut stream) = raw_tls(addr, config).await {
            assert!(read_http(&mut stream).await.is_empty());
        }
    }
    let legacy = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS12])
    .unwrap()
    .with_root_certificates(rustls::RootCertStore::empty())
    .with_no_client_auth();
    let error = raw_tls(addr, legacy).await.expect_err("TLS 1.2 recusado");
    assert!(matches!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<rustls::Error>()),
        Some(rustls::Error::AlertReceived(
            rustls::AlertDescription::ProtocolVersion
        )),
    ));

    let mut resumable = client_config(&caller, pin);
    resumable.resumption = rustls::client::Resumption::in_memory_sessions(4);
    resumable.enable_early_data = true;
    for _ in 0..2 {
        let mut stream = raw_tls(addr, resumable.clone()).await.unwrap();
        assert_eq!(
            stream.get_ref().1.handshake_kind(),
            Some(rustls::HandshakeKind::Full)
        );
        assert!(!stream.get_ref().1.is_early_data_accepted());
        stream
            .write_all(b"GET /v1/federation/config HTTP/1.1\r\nHost: fixture\r\n\r\n")
            .await
            .unwrap();
        let response = read_http(&mut stream).await;
        assert!(response.starts_with(b"HTTP/1.1 200 "));
    }
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}
