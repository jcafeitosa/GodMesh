//! Transporte HTTPS dedicado do perfil M3-B2. A autorização durável pertence ao router.
use std::convert::Infallible;
use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::uri::{Authority, PathAndQuery};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode, Version, header};
use axum::response::{IntoResponse, Response};
use godmesh_federation::transport::{self, NodeTls};
use godmesh_federation::{NodeIdentity, node_id_for};
use hyper::body::{Body as _, Incoming};
use hyper::server::conn::http1;
use hyper::service::{Service, service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout, timeout_at};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::{TlsAcceptor, TlsConnector, server::TlsStream};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_HEADER_FIELDS: usize = 64;
const FRAME_ROUTE: &str = "/v1/federation/config";

/// Identidade comprovada por mTLS e endereço da conexão TCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub node_id: String,
    pub public_key: [u8; 32],
    pub remote_addr: SocketAddr,
}

impl PeerIdentity {
    fn from_tls(
        certificates: Option<&[CertificateDer<'_>]>,
        alpn: Option<&[u8]>,
        remote_addr: SocketAddr,
    ) -> Result<Self, HttpsError> {
        if alpn != Some(transport::ALPN_HTTP1) {
            return Err(HttpsError::Tls);
        }
        let public_key = certificates
            .and_then(|certs| certs.first())
            .and_then(transport::certificate_public_key)
            .ok_or(HttpsError::Tls)?;
        Ok(Self {
            node_id: node_id_for(&public_key),
            public_key,
            remote_addr,
        })
    }
}

/// Limites explícitos do transporte. A cota de conexões inclui os handshakes.
#[derive(Debug, Clone, Copy)]
pub struct HttpsLimits {
    pub max_handshakes: usize,
    pub max_connections: usize,
    pub handshake_timeout: Duration,
    pub header_timeout: Duration,
    pub max_body_bytes: usize,
    /// Prazo de cada fase de corpo. No servidor também limita a escrita.
    pub body_timeout: Duration,
    pub pull_timeout: Duration,
}

impl HttpsLimits {
    fn validate(self) -> Result<(), HttpsError> {
        let now = Instant::now();
        if !(1..=Semaphore::MAX_PERMITS).contains(&self.max_handshakes)
            || !(1..=Semaphore::MAX_PERMITS).contains(&self.max_connections)
            || self.max_body_bytes == 0
            || [
                self.handshake_timeout,
                self.header_timeout,
                self.body_timeout,
                self.pull_timeout,
            ]
            .into_iter()
            .any(|duration| duration.is_zero() || now.checked_add(duration).is_none())
        {
            return Err(HttpsError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Falhas sanitizadas. `Io` e `Timeout` são elegíveis ao retry do chamador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpsError {
    InvalidConfiguration,
    InvalidRequest,
    Tls,
    Io,
    Timeout,
    Protocol,
    BodyTooLarge,
    InvalidBody,
}

impl fmt::Display for HttpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration => "configuração HTTPS inválida",
            Self::InvalidRequest => "pedido HTTPS inválido",
            Self::Tls => "sessão mTLS recusada",
            Self::Io => "falha de I/O HTTPS",
            Self::Timeout => "prazo HTTPS excedido",
            Self::Protocol => "resposta HTTP fora do perfil",
            Self::BodyTooLarge => "corpo acima do teto HTTPS",
            Self::InvalidBody => "corpo HTTP incompleto ou inválido",
        })
    }
}

impl std::error::Error for HttpsError {}

/// Serve um router dedicado até o sinal de shutdown ou o fechamento do canal.
/// O contexto mTLS e o endereço TCP são instalados antes da chamada ao router.
///
/// # Errors
/// Configuração inválida, material TLS inconsistente ou falha no listener.
pub async fn serve(
    listener: TcpListener,
    router: Router,
    identity: &NodeIdentity,
    client_pins: Vec<[u8; 32]>,
    limits: HttpsLimits,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), HttpsError> {
    limits.validate()?;
    let tls = NodeTls::self_issued(identity).map_err(|_| HttpsError::Tls)?;
    let config = transport::http1_server_tls(&tls, client_pins).map_err(|_| HttpsError::Tls)?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let handshakes = Arc::new(Semaphore::new(limits.max_handshakes));
    let connections = Arc::new(Semaphore::new(limits.max_connections));
    let mut tasks = JoinSet::new();
    let result = loop {
        if *shutdown.borrow_and_update() {
            break Ok(());
        }
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() {
                    break Ok(());
                }
            }
            _ = tasks.join_next(), if !tasks.is_empty() => {}
            accepted = listener.accept() => {
                let Ok((tcp, addr)) = accepted else {
                    break Err(HttpsError::Io);
                };
                let deadline = Instant::now() + limits.handshake_timeout;
                let Ok(connection_slot) = Arc::clone(&connections).try_acquire_owned() else {
                    continue;
                };
                let Ok(handshake_slot) = Arc::clone(&handshakes).try_acquire_owned() else {
                    continue;
                };
                let acceptor = acceptor.clone();
                let router = router.clone();
                tasks.spawn(async move {
                    let _connection_slot = connection_slot;
                    let tls = timeout_at(deadline, acceptor.accept(tcp)).await;
                    drop(handshake_slot);
                    let Ok(Ok(tls)) = tls else {
                        return;
                    };
                    let (_, session) = tls.get_ref();
                    let Ok(peer) = PeerIdentity::from_tls(session.peer_certificates(), session.alpn_protocol(), addr) else {
                        return;
                    };
                    serve_connection(tls, router, peer, limits).await;
                });
            }
        }
    };
    drop(listener);
    tasks.shutdown().await;
    result
}

async fn serve_connection(
    tls: TlsStream<TcpStream>,
    router: Router,
    peer: PeerIdentity,
    limits: HttpsLimits,
) {
    let (write_deadline, mut deadline) = watch::channel(None);
    let router = TowerToHyperService::new(router);
    let service = service_fn(move |request: Request<Incoming>| {
        let peer = peer.clone();
        let router = router.clone();
        let write_deadline = write_deadline.clone();
        async move {
            let response = match bounded_request(request, peer, limits).await {
                Ok(request) => match router.call(request).await {
                    Ok(response) => response,
                    Err(never) => match never {},
                },
                Err(error) => reject_request(error),
            };
            // O prazo de escrita começa após o router produzir a resposta.
            write_deadline.send_replace(Some(Instant::now() + limits.body_timeout));
            Ok::<_, Infallible>(response)
        }
    });
    let connection = http1::Builder::new()
        .timer(TokioTimer::new())
        .keep_alive(false)
        .max_headers(MAX_HEADER_FIELDS)
        .max_header_size(MAX_HEADER_BYTES)
        .header_read_timeout(limits.header_timeout)
        .serve_connection(TokioIo::new(tls), service);
    tokio::pin!(connection);
    let write_until = tokio::select! {
        _ = &mut connection => return,
        value = deadline.wait_for(Option::is_some) => value.ok().and_then(|value| *value),
    };
    if let Some(write_until) = write_until {
        let _ = timeout_at(write_until, connection).await;
    }
}

async fn bounded_request(
    incoming: Request<Incoming>,
    peer: PeerIdentity,
    limits: HttpsLimits,
) -> Result<Request<Body>, HttpsError> {
    let (mut parts, mut body) = incoming.into_parts();
    if parts.version != Version::HTTP_11 || parts.headers.contains_key(header::CONTENT_ENCODING) {
        return Err(HttpsError::InvalidRequest);
    }
    let bytes = timeout(limits.body_timeout, async {
        let mut bytes = Vec::new();
        while let Some(frame) = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await
        {
            let frame = frame.map_err(|_| HttpsError::InvalidBody)?;
            if let Some(data) = frame.data_ref() {
                if data.len() > limits.max_body_bytes - bytes.len() {
                    return Err(HttpsError::BodyTooLarge);
                }
                bytes.extend_from_slice(data);
            }
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| HttpsError::Timeout)??;
    parts.extensions.insert(ConnectInfo(peer.remote_addr));
    parts.extensions.insert(peer);
    Ok(Request::from_parts(parts, Body::from(bytes)))
}

fn reject_request(error: HttpsError) -> Response {
    let (status, code) = match error {
        HttpsError::BodyTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
        HttpsError::Timeout => (StatusCode::REQUEST_TIMEOUT, "request_timeout"),
        _ => (StatusCode::BAD_REQUEST, "invalid_request"),
    };
    (
        status,
        axum::Json(serde_json::json!({
            "error": code,
            "detail": error.to_string(),
            "hint": "verifique o perfil e os limites HTTPS",
            "ack_class": "transport_accepted",
        })),
    )
        .into_response()
}

/// Resultado HTTP já enquadrado, com a identidade autenticada naquela conexão.
/// Os cabeçalhos e o corpo são entregues para a validação semântica do perfil.
pub struct HttpsResponse {
    pub peer: PeerIdentity,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// Cliente do GET do perfil. Cada chamada abre uma sessão TLS e faz uma tentativa.
pub struct HttpsClient {
    tls: NodeTls,
    limits: HttpsLimits,
}

impl HttpsClient {
    /// Prepara a credencial mTLS local.
    ///
    /// # Errors
    /// Limites inválidos ou material TLS inconsistente.
    pub fn new(identity: &NodeIdentity, limits: HttpsLimits) -> Result<Self, HttpsError> {
        limits.validate()?;
        Ok(Self {
            tls: NodeTls::self_issued(identity).map_err(|_| HttpsError::Tls)?,
            limits,
        })
    }

    /// Executa o GET no endereço selecionado pelo chamador, com pin obrigatório.
    ///
    /// # Errors
    /// Falha de configuração, enquadramento HTTP, autenticação, I/O ou prazo.
    pub async fn get(
        &self,
        addr: SocketAddr,
        publisher_pin: [u8; 32],
        authority: &Authority,
        target: &PathAndQuery,
        if_none_match: Option<&HeaderValue>,
    ) -> Result<HttpsResponse, HttpsError> {
        if target.path() != FRAME_ROUTE || authority.as_str().contains('@') {
            return Err(HttpsError::InvalidRequest);
        }
        let conditional = if_none_match
            .map(HeaderValue::to_str)
            .transpose()
            .map_err(|_| HttpsError::InvalidRequest)?;
        let mut request = format!(
            "GET {target} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nAccept: application/json\r\n"
        );
        if let Some(etag) = conditional {
            request.push_str("If-None-Match: ");
            request.push_str(etag);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        if request.len() > MAX_HEADER_BYTES {
            return Err(HttpsError::InvalidRequest);
        }

        timeout(self.limits.pull_timeout, async {
            let config = transport::http1_client_tls(&self.tls, publisher_pin)
                .map_err(|_| HttpsError::Tls)?;
            let connector = TlsConnector::from(Arc::new(config));
            let server_name = ServerName::try_from(transport::CONNECT_SERVER_NAME)
                .map_err(|_| HttpsError::InvalidConfiguration)?;
            let tcp = TcpStream::connect(addr).await.map_err(|_| HttpsError::Io)?;
            let mut tls = timeout(
                self.limits.handshake_timeout,
                connector.connect(server_name, tcp),
            )
            .await
            .map_err(|_| HttpsError::Timeout)?
            .map_err(tls_io)?;
            let (tcp, session) = tls.get_ref();
            let peer = PeerIdentity::from_tls(
                session.peer_certificates(),
                session.alpn_protocol(),
                tcp.peer_addr().map_err(|_| HttpsError::Io)?,
            )?;
            tls.write_all(request.as_bytes()).await.map_err(tls_io)?;
            tls.flush().await.map_err(tls_io)?;
            let (status, headers, buffered) =
                timeout(self.limits.header_timeout, read_headers(&mut tls))
                    .await
                    .map_err(|_| HttpsError::Timeout)??;
            let length = body_length(status, &headers)?;
            if length > self.limits.max_body_bytes {
                return Err(if status == StatusCode::SERVICE_UNAVAILABLE {
                    HttpsError::InvalidBody
                } else {
                    HttpsError::BodyTooLarge
                });
            }
            if buffered.len() > length {
                return Err(HttpsError::InvalidBody);
            }
            let body = timeout(
                self.limits.body_timeout,
                read_body(&mut tls, status, buffered, length),
            )
            .await
            .map_err(|_| HttpsError::Timeout)??;
            Ok(HttpsResponse {
                peer,
                status,
                headers,
                body,
            })
        })
        .await
        .map_err(|_| HttpsError::Timeout)?
    }
}

fn tls_io(error: std::io::Error) -> HttpsError {
    if error.kind() == std::io::ErrorKind::InvalidData {
        HttpsError::Tls
    } else {
        HttpsError::Io
    }
}

async fn read_headers(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<(StatusCode, HeaderMap, Vec<u8>), HttpsError> {
    let mut received = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let available = (MAX_HEADER_BYTES - received.len()).min(chunk.len());
        if available == 0 {
            return Err(HttpsError::Protocol);
        }
        let count = stream.read(&mut chunk[..available]).await.map_err(tls_io)?;
        if count == 0 {
            return Err(HttpsError::Io);
        }
        received.extend_from_slice(&chunk[..count]);
        let mut raw = [httparse::EMPTY_HEADER; MAX_HEADER_FIELDS];
        let mut parsed = httparse::Response::new(&mut raw);
        let httparse::Status::Complete(end) =
            parsed.parse(&received).map_err(|_| HttpsError::Protocol)?
        else {
            continue;
        };
        let status = parsed
            .code
            .and_then(|code| StatusCode::from_u16(code).ok())
            .filter(|status| !status.is_informational())
            .ok_or(HttpsError::Protocol)?;
        if parsed.version != Some(1) {
            return Err(HttpsError::Protocol);
        }
        let mut headers = HeaderMap::new();
        for field in parsed.headers {
            let name =
                HeaderName::from_bytes(field.name.as_bytes()).map_err(|_| HttpsError::Protocol)?;
            let value = HeaderValue::from_bytes(field.value).map_err(|_| HttpsError::Protocol)?;
            headers.append(name, value);
        }
        return Ok((status, headers, received.split_off(end)));
    }
}

fn body_length(status: StatusCode, headers: &HeaderMap) -> Result<usize, HttpsError> {
    if headers.contains_key(header::TRANSFER_ENCODING)
        || headers.contains_key(header::CONTENT_ENCODING)
    {
        return Err(HttpsError::InvalidBody);
    }
    let lengths = headers.get_all(header::CONTENT_LENGTH);
    let mut values = lengths.iter();
    let length = values
        .next()
        .map(|value| {
            let value = value.to_str().map_err(|_| HttpsError::InvalidBody)?;
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(HttpsError::InvalidBody);
            }
            value.parse::<usize>().map_err(|_| HttpsError::InvalidBody)
        })
        .transpose()?;
    if values.next().is_some() {
        return Err(HttpsError::InvalidBody);
    }
    if status == StatusCode::NOT_MODIFIED {
        return if length.is_none_or(|length| length == 0) {
            Ok(0)
        } else {
            Err(HttpsError::InvalidBody)
        };
    }
    length.ok_or(HttpsError::InvalidBody)
}

async fn read_body(
    stream: &mut (impl AsyncRead + Unpin),
    status: StatusCode,
    mut body: Vec<u8>,
    length: usize,
) -> Result<Vec<u8>, HttpsError> {
    let mut chunk = [0; 8192];
    while body.len() < length {
        let remaining = (length - body.len()).min(chunk.len());
        let count = stream
            .read(&mut chunk[..remaining])
            .await
            .map_err(|error| {
                // Sem o corpo completo de um 503, o pull não pode decidir retry
                // pelo catálogo de erros, inclusive se a sessão cair sem close_notify.
                if status == StatusCode::SERVICE_UNAVAILABLE {
                    HttpsError::InvalidBody
                } else {
                    tls_io(error)
                }
            })?;
        if count == 0 {
            return Err(HttpsError::InvalidBody);
        }
        body.extend_from_slice(&chunk[..count]);
    }
    // A tentativa pede Connection: close. Exija o fim da sessão para
    // verificar o mesmo enquadramento mesmo com dados em outra leitura.
    if stream.read(&mut chunk[..1]).await.map_err(tls_io)? != 0 {
        return Err(HttpsError::InvalidBody);
    }
    Ok(body)
}
