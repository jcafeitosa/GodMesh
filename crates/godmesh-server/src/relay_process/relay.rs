//! Lado R do encaminhamento QUIC/UDP da fixture M5.

use std::{
    collections::HashSet,
    fmt,
    io::{self, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    num::NonZeroU32,
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::Duration,
};

use godmesh_federation::{
    node::{
        LinkIdentity, OperationalSession, RelayCancellation, RelayOpening, RelayRecv, RelaySend,
        SessionError,
    },
    relay_wire::{Accept, Control, ErrorCode, Open, Refuse, decode_control, encode_control},
};
use godmesh_server::relay_lab::{
    Direction, Error as AdmissionError, MAX_CONTROL_BYTES, RelayAdmission, RouteInstance,
    SendCredit, Usage, normalize_endpoint,
};
use tokio::{
    net::UdpSocket,
    sync::watch,
    task::JoinSet,
    time::{Instant, timeout_at},
};

use super::udp::{DATAGRAM_LIMIT, UdpReader};

const CONTROL_LIMIT: NonZeroU32 = NonZeroU32::new(MAX_CONTROL_BYTES as u32).unwrap();
const OPEN_TIMEOUT: Duration = Duration::from_secs(2);
// Guarda independente da espera de crédito de envio e das rechecagens de autoridade.
const QUIC_RECEIVE_TIMEOUT: Duration = Duration::from_secs(60);

/// Atende as streams relay de uma única sessão autenticada E↔R.
pub(super) async fn run(
    session: OperationalSession,
    admission: Arc<RelayAdmission>,
    m3_changes: Option<watch::Receiver<u64>>,
) {
    log_sink();
    let request_ids = Arc::new(RequestIds::default());
    let mut routes = JoinSet::new();
    'session: loop {
        // Recolher uma rota não descarta um aceite pendente de outra stream.
        let accept = session.accept_relay(OPEN_TIMEOUT, CONTROL_LIMIT);
        tokio::pin!(accept);
        let result = loop {
            tokio::select! {
                biased;
                _ = session.cancelled() => break 'session,
                _ = routes.join_next(), if !routes.is_empty() => {},
                result = &mut accept => break result,
            }
        };
        match result {
            Ok(opening) => {
                routes.spawn(serve(
                    opening,
                    session.remote().clone(),
                    Arc::clone(&admission),
                    Arc::clone(&request_ids),
                    m3_changes.clone(),
                ));
            }
            Err(SessionError::Busy | SessionError::Timeout) => {}
            Err(_) => break,
        }
    }
    // O fechamento da sessão também cancela o I/O das rotas. Juntar sem abortar
    // para que cada uma possa devolver sua reserva após recolher o transporte.
    while routes.join_next().await.is_some() {}
}

#[derive(Default)]
struct RequestIds(Mutex<HashSet<String>>);

impl RequestIds {
    fn claim(&self, id: &str) -> Option<RequestId<'_>> {
        self.0
            .lock()
            .unwrap()
            .insert(id.to_owned())
            .then(|| RequestId {
                session: self,
                id: id.to_owned(),
            })
    }
}

struct RequestId<'a> {
    session: &'a RequestIds,
    id: String,
}

impl Drop for RequestId<'_> {
    fn drop(&mut self) {
        self.session.0.lock().unwrap().remove(&self.id);
    }
}

async fn serve(
    mut opening: RelayOpening,
    peer: LinkIdentity,
    admission: Arc<RelayAdmission>,
    request_ids: Arc<RequestIds>,
    m3_changes: Option<watch::Receiver<u64>>,
) {
    let Some((request, _request_id)) = read_request(&mut opening, &request_ids).await else {
        // O descarte reseta a stream quando falta correlação inequívoca.
        return;
    };
    let mut token = [0; 16];
    if getrandom::fill(&mut token).is_err() {
        refuse(&mut opening, request, ErrorCode::RouteUnavailable).await;
        return;
    }

    let opened_at = opening.deadline() - OPEN_TIMEOUT;
    let instance = match admission.open(&request.route_id, &peer, opened_at).await {
        Ok(instance) => instance,
        Err(error) => {
            refuse(&mut opening, request, refusal_code(error)).await;
            return;
        }
    };

    let (reason, udp_drops) = match confirm(&mut opening, &instance, request, token).await {
        Ok(next_check) => match opening.into_forwarding(DATAGRAM_LIMIT) {
            Ok((send, recv, cancellation)) => {
                Forwarding {
                    send,
                    recv,
                    cancellation,
                }
                .run(&instance, next_check, m3_changes)
                .await
            }
            Err(error) => (error.into(), 0),
        },
        Err(reason) => {
            drop(opening);
            (reason, 0)
        }
    };

    instance.cancel();
    let route_id = instance.route().id.clone();
    let destination = &instance.route().destination.node_id;
    let outbound = admission.usage(Some(destination), Direction::ToDestination);
    let inbound = admission.usage(Some(destination), Direction::FromDestination);
    instance.recollect();
    log_line(format_args!(
        "event=relay_route_cancel route={route_id} reason={}",
        reason.as_str()
    ));
    log_usage(&route_id, "to_destination", outbound, 0);
    log_usage(&route_id, "from_destination", inbound, udp_drops);
}

async fn read_request<'a>(
    opening: &mut RelayOpening,
    request_ids: &'a RequestIds,
) -> Option<(Open, RequestId<'a>)> {
    let body = opening.read_control().await.ok()?;
    let Control::Open(request) = decode_control(&body, CONTROL_LIMIT).ok()? else {
        return None;
    };
    let claim = request_ids.claim(&request.request_id)?;
    Some((request, claim))
}

fn refusal_code(error: AdmissionError) -> ErrorCode {
    match error {
        AdmissionError::RouteDenied => ErrorCode::RouteDenied,
        AdmissionError::AuthorityBlocked => ErrorCode::AuthorityBlocked,
        AdmissionError::QuotaExceeded => ErrorCode::QuotaExceeded,
        _ => ErrorCode::RouteUnavailable,
    }
}

async fn refuse(opening: &mut RelayOpening, request: Open, error: ErrorCode) {
    let control = Control::Refuse(Refuse {
        request_id: request.request_id,
        route_id: request.route_id,
        error,
    });
    if let Ok(body) = encode_control(&control, CONTROL_LIMIT)
        && opening.write_control(&body).await.is_ok()
    {
        let _ = opening.finish_send();
    }
}

async fn confirm(
    opening: &mut RelayOpening,
    instance: &RouteInstance,
    request: Open,
    token: [u8; 16],
) -> Result<Instant, StopReason> {
    let reply = Control::Accept(Accept {
        request_id: request.request_id,
        route_id: request.route_id,
        max_datagram_bytes: DATAGRAM_LIMIT,
        ready_token: hex::encode(token),
    });
    let body = encode_control(&reply, CONTROL_LIMIT).map_err(|_| StopReason::Transport)?;
    let mut cancelled = instance.cancellation();
    tokio::select! {
        biased;
        _ = cancelled.wait_for(|value| *value) => Err(StopReason::Cancelled),
        result = async {
            opening.write_control(&body).await?;
            opening.expect_token(token).await?;
            Ok(instance.confirm_token().await?)
        } => result,
    }
}

struct Forwarding {
    send: RelaySend,
    recv: RelayRecv,
    cancellation: RelayCancellation,
}

impl Forwarding {
    async fn run(
        mut self,
        instance: &RouteInstance,
        next_check: Instant,
        m3_changes: Option<watch::Receiver<u64>>,
    ) -> (StopReason, u64) {
        let mut udp_drops = 0;
        let reason = self
            .drive(instance, next_check, m3_changes, &mut udp_drops)
            .await;
        instance.cancel();
        let cancellation = self.cancellation.clone();
        cancellation.cancel();
        drop(self);
        cancellation.reclaimed().await;
        (reason, udp_drops)
    }

    async fn drive(
        &mut self,
        instance: &RouteInstance,
        next_check: Instant,
        m3_changes: Option<watch::Receiver<u64>>,
        udp_drops: &mut u64,
    ) -> StopReason {
        let mut cancelled = instance.cancellation();
        // Cada ramo possui sua própria espera. Ao sair, as outras futures são
        // descartadas, inclusive seus créditos, antes de recolher a reserva.
        tokio::select! {
            biased;
            _ = cancelled.wait_for(|value| *value) => StopReason::Cancelled,
            _ = self.cancellation.cancelled() => StopReason::Closed,
            result = instance.run_rechecks(next_check, m3_changes) =>
                result.err().map_or(StopReason::Cancelled, StopReason::from),
            result = forward_packets(instance, &mut self.send, &mut self.recv, udp_drops) => result,
        }
    }
}

async fn forward_packets(
    instance: &RouteInstance,
    send: &mut RelaySend,
    recv: &mut RelayRecv,
    udp_drops: &mut u64,
) -> StopReason {
    let Ok(socket) = connect_destination(instance.route().endpoint).await else {
        return StopReason::Udp;
    };
    log_line(format_args!(
        "event=relay_route_forwarding route={}",
        instance.route().id
    ));
    tokio::select! {
        result = receive_quic(instance, recv) => result,
        result = receive_udp(instance, &socket, udp_drops) => result,
        result = send_credits(instance, Egress::Udp(&socket)) => result,
        result = send_credits(instance, Egress::Quic(send)) => result,
    }
}

async fn connect_destination(endpoint: SocketAddr) -> io::Result<UdpSocket> {
    let endpoint = canonical_udp(endpoint)?;
    let loopback = if endpoint.is_ipv4() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    };
    attach_destination(UdpSocket::bind((loopback, 0)).await?, endpoint).await
}

async fn attach_destination(socket: UdpSocket, endpoint: SocketAddr) -> io::Result<UdpSocket> {
    let endpoint = canonical_udp(endpoint)?;
    if canonical_udp(socket.local_addr()?)? == endpoint {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "autoalvo UDP"));
    }
    socket.connect(endpoint).await?;
    Ok(socket)
}

fn canonical_udp(endpoint: SocketAddr) -> io::Result<SocketAddr> {
    normalize_endpoint(endpoint)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "endpoint UDP inválido"))
}

async fn receive_quic(instance: &RouteInstance, recv: &mut RelayRecv) -> StopReason {
    loop {
        let packet = match recv
            .read_datagram(Instant::now() + QUIC_RECEIVE_TIMEOUT)
            .await
        {
            Ok(packet) => packet,
            Err(error) => return error.into(),
        };
        if let Err(error) = enqueue(instance, Direction::ToDestination, packet) {
            return error.into();
        }
        tokio::task::yield_now().await;
    }
}

async fn receive_udp(
    instance: &RouteInstance,
    socket: &UdpSocket,
    udp_drops: &mut u64,
) -> StopReason {
    let mut reader = UdpReader::default();
    loop {
        match reader.receive(socket).await {
            Ok(Some(packet)) => {
                if let Err(error) = enqueue(instance, Direction::FromDestination, packet.to_vec()) {
                    return error.into();
                }
            }
            Ok(None) => *udp_drops += 1,
            Err(_) => return StopReason::Udp,
        }
        tokio::task::yield_now().await;
    }
}

fn enqueue(
    instance: &RouteInstance,
    direction: Direction,
    packet: Vec<u8>,
) -> Result<(), AdmissionError> {
    match instance.enqueue(direction, packet) {
        Err(AdmissionError::QueueFull) => Ok(()),
        result => result,
    }
}

enum Egress<'a> {
    Udp(&'a UdpSocket),
    Quic(&'a mut RelaySend),
}

async fn send_credits(instance: &RouteInstance, mut egress: Egress<'_>) -> StopReason {
    let direction = match egress {
        Egress::Udp(_) => Direction::ToDestination,
        Egress::Quic(_) => Direction::FromDestination,
    };
    loop {
        if let Err(error) = instance.wait_for_packet(direction).await {
            return error.into();
        }
        match instance.next_credit(direction).await {
            Ok(Some(credit)) => {
                let delivered = match &mut egress {
                    Egress::Udp(socket) => send_udp(socket, &credit).await,
                    Egress::Quic(stream) => stream
                        .write_datagram(credit.datagram(), credit.deadline())
                        .await
                        .map_err(StopReason::from),
                };
                if let Err(reason) = delivered {
                    return reason;
                }
            }
            Ok(None) | Err(AdmissionError::RateExceeded) => {}
            Err(error) => return error.into(),
        }
    }
}

async fn send_udp(socket: &UdpSocket, credit: &SendCredit) -> Result<(), StopReason> {
    let deadline = credit.deadline();
    loop {
        timeout_at(deadline, socket.writable())
            .await
            .map_err(|_| StopReason::Deadline)?
            .map_err(|_| StopReason::Udp)?;
        if Instant::now() >= deadline {
            return Err(StopReason::Deadline);
        }
        match socket.try_send(credit.datagram()) {
            Ok(written) if written == credit.datagram().len() => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            _ => return Err(StopReason::Udp),
        }
    }
}

#[derive(Clone, Copy)]
enum StopReason {
    Authority,
    Cancelled,
    Closed,
    Deadline,
    Transport,
    Udp,
}

impl StopReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::Cancelled => "cancelled",
            Self::Closed => "closed",
            Self::Deadline => "deadline",
            Self::Transport => "transport",
            Self::Udp => "udp",
        }
    }
}

impl From<AdmissionError> for StopReason {
    fn from(error: AdmissionError) -> Self {
        match error {
            AdmissionError::Cancelled => Self::Cancelled,
            _ => Self::Authority,
        }
    }
}

impl From<SessionError> for StopReason {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::Closed => Self::Closed,
            SessionError::Timeout => Self::Deadline,
            _ => Self::Transport,
        }
    }
}

fn log_usage(route: &str, direction: &str, usage: Usage, udp_drops: u64) {
    log_line(format_args!(
        "event=relay_route_metrics route={route} direction={direction} debited_frames={} debited_bytes={} queued_frames={} queued_bytes={} queue_drops={} rate_drops={} size_drops={} cancelled_frames={} udp_drops={udp_drops}",
        usage.debited_frames,
        usage.debited_bytes,
        usage.queued_frames,
        usage.queued_bytes,
        usage.queue_drops,
        usage.rate_drops,
        usage.size_drops,
        usage.cancelled_frames,
    ));
}

fn log_line(line: fmt::Arguments<'_>) {
    // A saída da fixture nunca retém o loop de rechecagem nem acumula logs sem limite.
    let _ = log_sink().try_send(line.to_string());
}

fn log_sink() -> &'static mpsc::SyncSender<String> {
    static SINK: OnceLock<mpsc::SyncSender<String>> = OnceLock::new();
    SINK.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<String>(64);
        let _ = std::thread::Builder::new()
            .name("relay-logs".into())
            .spawn(move || {
                for line in receiver {
                    if writeln!(io::stdout().lock(), "{line}").is_err() {
                        break;
                    }
                }
            });
        sender
    })
}
#[cfg(test)]
mod tests;
