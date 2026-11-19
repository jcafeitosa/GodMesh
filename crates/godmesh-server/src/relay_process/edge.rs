use std::{net::SocketAddr, num::NonZeroU64, time::Duration};

use godmesh_federation::{
    node::{
        DialCancellation, DialTarget, FederationNode, OperationalSession, RelayRecv, RelaySend,
    },
    relay_fallback::DirectPathController,
    relay_wire::{self, Control, Open},
};
use tokio::{
    net::UdpSocket,
    time::{Instant, timeout_at},
};

use super::{
    CONTROL_LIMIT, OPEN_TIMEOUT,
    blocked_path::BlockedPath,
    udp::{DATAGRAM_LIMIT, UdpReader},
};

const IO_TIMEOUT: Duration = Duration::from_millis(100);
const FIXTURE_READ_WINDOW: Duration = Duration::from_secs(60);

pub(super) struct Route {
    pub(super) id: String,
    pub(super) direct: DialTarget,
    pub(super) shim: SocketAddr,
    pub(super) generation: NonZeroU64,
}

pub(super) async fn run(
    node: &FederationNode,
    relay: &OperationalSession,
    route: &Route,
) -> Result<(), String> {
    let own_addr = node.local_addr().ok_or("endpoint local QUIC ausente")?;
    let blocked = BlockedPath::bind(route.direct.addr, own_addr)
        .await
        .map_err(report)?;
    let controller = DirectPathController::new(route.direct.clone()).map_err(report)?;
    let mut direct = node.start_dial(route.direct.clone()).map_err(report)?;
    let proof = tokio::select! {
        biased;
        result = direct.wait() => {
            result.map_err(report)?;
            eprintln!("event=relay_direct_promoted route={}", route.id);
            return Ok(());
        }
        result = blocked.observe_drop(Instant::now() + OPEN_TIMEOUT) => result.map_err(report)?,
        _ = relay.cancelled() => return Ok(()),
    };
    if proof.endpoint() != route.direct.addr {
        return Err("endpoint do bloqueio diverge da tentativa".into());
    }
    let evidence = {
        let mut guard = controller.lock();
        guard
            .set_effective(Some(route.generation))
            .map_err(report)?;
        guard
            .confirm_unavailable(&direct, proof.confirmed_at())
            .map_err(report)?
    };
    let mut attempt = direct.reap_for_fallback().await.map_err(report)?;
    eprintln!("event=relay_direct_reaped route={}", route.id);
    let reservation = controller
        .lock()
        .reserve(
            &evidence,
            &mut attempt,
            &route.direct,
            std::time::Instant::now(),
        )
        .map_err(report)?;
    let shim = UdpSocket::bind(route.shim).await.map_err(report)?;
    shim.connect(own_addr).await.map_err(report)?;
    let target = reservation.into_target(shim.local_addr().map_err(report)?);

    let deadline = Instant::now() + OPEN_TIMEOUT;
    eprintln!("event=relay_open_requested route={}", route.id);
    let mut stream = relay
        .open_relay(deadline, CONTROL_LIMIT)
        .await
        .map_err(report)?;
    let mut request_id = [0; 16];
    getrandom::fill(&mut request_id).map_err(report)?;
    let request = Open {
        request_id: hex::encode(request_id),
        route_id: route.id.clone(),
    };
    let body = relay_wire::encode_control(&Control::Open(request.clone()), CONTROL_LIMIT)
        .map_err(report)?;
    stream.write_control(&body).await.map_err(report)?;
    let body = stream.read_control().await.map_err(report)?;
    let reply = relay_wire::decode_control(&body, CONTROL_LIMIT).map_err(report)?;
    relay_wire::validate_reply(&request, &reply, DATAGRAM_LIMIT).map_err(report)?;
    let accept = match reply {
        Control::Accept(accept) => accept,
        Control::Refuse(refusal) => {
            stream.expect_fin().await.map_err(report)?;
            return Err(report(refusal.error));
        }
        Control::Open(_) => return Err("pedido recebido no lugar da resposta".into()),
    };
    stream
        .send_token(accept.token_bytes().map_err(report)?)
        .await
        .map_err(report)?;
    let (mut sender, mut receiver, cancellation) =
        stream.into_forwarding(DATAGRAM_LIMIT).map_err(report)?;
    let forwarding = match node.start_relay_dial(target, &cancellation) {
        Ok(mut inner) => {
            let result = tokio::select! {
                biased;
                _ = relay.cancelled() => Ok(()),
                result = to_relay(&shim, &mut sender) => result,
                result = from_relay(&shim, &mut receiver) => result,
                result = async {
                    inner.wait().await.map_err(report)?;
                    eprintln!("event=relay_inner_promoted route={}", route.id);
                    std::future::pending().await
                } => result,
            };
            cancellation.cancel();
            if let DialCancellation::AlreadyPromoted(link) = inner.cancel() {
                link.invalidate();
            }
            let _ = inner.wait().await;
            result
        }
        Err(error) => Err(report(error)),
    };
    cancellation.cancel();
    drop(sender);
    drop(receiver);
    cancellation.reclaimed().await;
    drop(blocked);
    forwarding
}

async fn to_relay(shim: &UdpSocket, sender: &mut RelaySend) -> Result<(), String> {
    let mut reader = UdpReader::default();
    loop {
        if let Some(body) = reader.receive(shim).await.map_err(report)? {
            sender
                .write_datagram(body, Instant::now() + IO_TIMEOUT)
                .await
                .map_err(report)?;
        }
    }
}

async fn from_relay(shim: &UdpSocket, receiver: &mut RelayRecv) -> Result<(), String> {
    loop {
        let body = receiver
            .read_datagram(Instant::now() + FIXTURE_READ_WINDOW)
            .await
            .map_err(report)?;
        let bytes = timeout_at(Instant::now() + IO_TIMEOUT, shim.send(&body))
            .await
            .map_err(report)?
            .map_err(report)?;
        if bytes != body.len() {
            return Err("envio UDP parcial".into());
        }
    }
}

fn report(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
