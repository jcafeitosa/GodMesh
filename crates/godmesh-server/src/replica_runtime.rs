//! Orquestra uma réplica M4-A2 sobre uma sessão QUIC já promovida.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_federation::m4a_wire::{
    self as wire, Ack, AckClass, Batch, ErrorCode, ErrorMessage, Message, Pull, WireError,
};
use godmesh_federation::node::{
    OperationalBiStream, OperationalSession, OperationalSessionHandler, SessionError, SessionLimits,
};
use godmesh_server::replica_operational::{
    EffectError, ReceiverState, ReplicaConfig, ReplicaService, Role, SessionPeer,
};
use godmesh_server::replication::{CopyPage, Error as ReplicationError, Limits};
use tokio::{sync::Mutex, task::JoinSet, time::Instant};

pub struct ReplicaRuntime {
    service: Arc<ReplicaService>,
    receiver: Mutex<()>,
    next_request: AtomicU64,
}

impl ReplicaRuntime {
    pub fn new(service: ReplicaService) -> Self {
        Self {
            service: Arc::new(service),
            receiver: Mutex::new(()),
            next_request: AtomicU64::new(0),
        }
    }

    pub fn session_limits(&self) -> SessionLimits {
        let config = self.service.config();
        SessionLimits {
            max_frame_bytes: config.max_frame_bytes,
            max_streams_per_connection: config.max_streams_per_connection,
            attempt_timeout: config.attempt_timeout,
        }
    }

    async fn serve(&self, session: &OperationalSession, remote: &Remote) {
        let mut tasks = JoinSet::new();
        loop {
            let mut stream = tokio::select! {
                biased;
                _ = session.cancelled() => break,
                _ = tasks.join_next(), if !tasks.is_empty() => continue,
                result = session.accept_bi() => match result {
                    Ok(stream) => stream,
                    Err(SessionError::Busy | SessionError::Timeout) => continue,
                    Err(_) => break,
                },
            };
            let service = Arc::clone(&self.service);
            let remote = remote.clone();
            tasks.spawn(async move {
                let deadline = stream.deadline();
                let _ = tokio::select! {
                    result = serve_pull(&service, &remote, &mut stream) => result,
                    loss = recheck(&service, &remote, deadline) => Err(loss),
                };
            });
        }
        tasks.shutdown().await;
    }

    async fn receive(&self, session: &OperationalSession, remote: &Remote, delay: Duration) {
        // Sessões paralelas disputam a única tarefa deste namespace. O guard
        // fica com o callback da conexão que iniciou o pull.
        let _one_pull = tokio::select! {
            biased;
            _ = session.cancelled() => return,
            guard = self.receiver.lock() => guard,
        };
        loop {
            let cycle = tokio::select! {
                biased;
                _ = session.cancelled() => return,
                result = self.pull_cycle(session, remote) => result,
            };
            match cycle {
                Ok(Cycle::More) => tokio::task::yield_now().await,
                Ok(Cycle::Suspended) => {
                    eprintln!("event=m4_history_suspended origin={}", remote.node_id);
                    return;
                }
                Ok(Cycle::Idle) | Err(_) => {
                    tokio::select! {
                        biased;
                        _ = session.cancelled() => return,
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
            }
        }
    }

    async fn pull_cycle(
        &self,
        session: &OperationalSession,
        remote: &Remote,
    ) -> Result<Cycle, AttemptError> {
        let config = self.service.config();
        for attempt in 1..=config.max_attempts.get() {
            let deadline = Instant::now() + config.attempt_timeout;
            let result = tokio::select! {
                result = self.pull_once(session, remote, deadline) => result,
                loss = recheck(&self.service, remote, deadline) => Err(loss),
            };
            match result {
                Err(error) if error.retryable() && attempt < config.max_attempts.get() => {
                    tokio::time::sleep(config.retry_interval).await;
                }
                result => return result,
            }
        }
        unreachable!("o orçamento de tentativas M4 é positivo")
    }

    async fn pull_once(
        &self,
        session: &OperationalSession,
        remote: &Remote,
        deadline: Instant,
    ) -> Result<Cycle, AttemptError> {
        let after_seq = match self.service.checkpoint(remote.peer(), deadline).await? {
            ReceiverState::Ready { after_seq } => after_seq,
            ReceiverState::SuspendedHistory { .. } => return Ok(Cycle::Suspended),
        };
        let request = self
            .next_request
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| AttemptError::Protocol)?;
        let config = self.service.config();
        let pull = Pull {
            request_id: format!("{request:032x}"),
            namespace: namespace(config),
            destination: session.local().node_id.clone(),
            after_seq,
            receiver_limits: limits(config),
        };
        let body = wire::encode_body(&Message::Pull(pull.clone()), config.max_frame_bytes)?;
        let mut stream = io(deadline, session.open_bi(deadline)).await?;
        io(deadline, stream.write_frame(&body)).await?;
        let response = io(deadline, stream.read_frame()).await?;
        io(deadline, stream.expect_fin()).await?;

        match wire::decode_body(&response, config.max_frame_bytes)? {
            Message::Batch(batch) => {
                wire::validate_batch(&pull, &batch, response.len())?;
                if batch.events.is_empty() {
                    finish(&mut stream, deadline)?;
                    return Ok(Cycle::Idle);
                }
                let frames = batch
                    .events
                    .iter()
                    .map(|frame| frame.to_vec())
                    .collect::<Vec<_>>();
                let applied = self
                    .service
                    .apply_copy(
                        remote.peer(),
                        local_limits(batch.effective_limits)?,
                        &frames,
                        deadline,
                    )
                    .await?;
                let ack = Message::Ack(Ack {
                    request_id: pull.request_id,
                    namespace: pull.namespace,
                    destination: pull.destination,
                    through_seq: applied.last,
                    ack_class: AckClass::PersistedLocal,
                });
                let body = wire::encode_body(&ack, batch.effective_limits.frame_bytes)?;
                io(deadline, stream.write_frame(&body)).await?;
                finish(&mut stream, deadline)?;
                Ok(if batch.has_more {
                    Cycle::More
                } else {
                    Cycle::Idle
                })
            }
            Message::Error(error) => {
                wire::validate_error(&pull, &error)?;
                if error.error == ErrorCode::HistoryUnavailable {
                    let state = self
                        .service
                        .suspend_history(remote.peer(), after_seq, deadline)
                        .await?;
                    finish(&mut stream, deadline)?;
                    return Ok(match state {
                        ReceiverState::Ready { .. } => Cycle::More,
                        ReceiverState::SuspendedHistory { .. } => Cycle::Suspended,
                    });
                }
                finish(&mut stream, deadline)?;
                Err(AttemptError::Remote(error.error))
            }
            _ => Err(AttemptError::Protocol),
        }
    }
}

impl OperationalSessionHandler for ReplicaRuntime {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            if session.negotiated_alpn() != "godmesh/1" {
                return;
            }
            let remote = Remote {
                node_id: session.remote().node_id.clone(),
                key: hex::encode(session.remote().public_key),
            };
            let config = self.service.config();
            match config.role {
                Role::Origin
                    if config.destination_pins.iter().any(|pin| {
                        pin.node_id == remote.node_id && pin.public_key == remote.key
                    }) =>
                {
                    self.serve(&session, &remote).await
                }
                Role::Receiver
                    if config.origin_pin.node_id == remote.node_id
                        && config.origin_pin.public_key == remote.key =>
                {
                    if let Some(interval) = config.pull_interval {
                        self.receive(&session, &remote, interval).await;
                    }
                }
                _ => {}
            }
        })
    }
}

#[derive(Clone)]
struct Remote {
    node_id: String,
    key: String,
}

impl Remote {
    fn peer(&self) -> SessionPeer<'_> {
        SessionPeer {
            node_id: &self.node_id,
            public_key: &self.key,
            protocol: 1,
        }
    }
}

enum Cycle {
    More,
    Idle,
    Suspended,
}

enum AttemptError {
    Transport(SessionError),
    Effect(EffectError),
    Remote(ErrorCode),
    Timeout,
    Protocol,
}

impl AttemptError {
    fn retryable(&self) -> bool {
        match self {
            Self::Transport(
                SessionError::Busy | SessionError::Transport | SessionError::Timeout,
            )
            | Self::Remote(ErrorCode::StorageUnavailable)
            | Self::Timeout => true,
            Self::Effect(error) => error.is_retryable(),
            _ => false,
        }
    }
}

impl From<SessionError> for AttemptError {
    fn from(error: SessionError) -> Self {
        Self::Transport(error)
    }
}

impl From<WireError> for AttemptError {
    fn from(_: WireError) -> Self {
        Self::Protocol
    }
}

impl From<EffectError> for AttemptError {
    fn from(error: EffectError) -> Self {
        Self::Effect(error)
    }
}

async fn io<T>(
    deadline: Instant,
    operation: impl Future<Output = Result<T, SessionError>>,
) -> Result<T, AttemptError> {
    if Instant::now() >= deadline {
        return Err(AttemptError::Timeout);
    }
    Ok(tokio::time::timeout_at(deadline, operation)
        .await
        .map_err(|_| AttemptError::Timeout)??)
}

fn finish(stream: &mut OperationalBiStream, deadline: Instant) -> Result<(), AttemptError> {
    if Instant::now() >= deadline {
        return Err(AttemptError::Timeout);
    }
    Ok(stream.finish_send()?)
}

async fn recheck(service: &ReplicaService, remote: &Remote, deadline: Instant) -> AttemptError {
    let interval = service.config().acl_recheck_interval;
    loop {
        tokio::time::sleep(interval).await;
        if let Err(error) = service.recheck(remote.peer(), deadline).await {
            return error.into();
        }
    }
}

fn namespace(config: &ReplicaConfig) -> wire::Namespace {
    wire::Namespace {
        origin_node: config.namespace.origin_node.clone(),
        tenant_id: config.namespace.tenant_id.clone(),
        stream_id: config.namespace.stream_id.clone(),
        partition: config.namespace.partition,
        authority_epoch: config.authority_epoch,
    }
}

fn limits(config: &ReplicaConfig) -> wire::Limits {
    wire::Limits {
        frame_bytes: config.max_frame_bytes,
        envelope_bytes: config.max_envelope_bytes,
        event_count: config.max_events,
    }
}

fn local_limits(values: wire::Limits) -> Result<Limits, EffectError> {
    let frames = usize::try_from(values.frame_bytes.get())
        .map_err(|_| EffectError::Replication(ReplicationError::LimitExceeded))?;
    Limits::new(
        values.event_count.get(),
        values.envelope_bytes.get(),
        frames,
    )
    .map_err(EffectError::Replication)
}

struct PreparedPage {
    body: Vec<u8>,
    through_seq: Option<i64>,
}

fn prepare_page(
    pull: &Pull,
    source: wire::Limits,
    page: CopyPage,
) -> Result<PreparedPage, EffectError> {
    let effective = source.component_min(pull.receiver_limits);
    let encode = |count: usize| {
        wire::encode_body(
            &Message::Batch(Batch {
                request_id: pull.request_id.clone(),
                namespace: pull.namespace.clone(),
                destination: pull.destination.clone(),
                after_seq: pull.after_seq,
                source_limits: source,
                effective_limits: effective,
                events: page.frames[..count].iter().map(Vec::as_slice).collect(),
                has_more: page.has_more || count < page.frames.len(),
            }),
            effective.frame_bytes,
        )
    };
    if page.frames.is_empty() {
        let body = encode(0).map_err(|_| EffectError::Protocol)?;
        return Ok(PreparedPage {
            body,
            through_seq: None,
        });
    }

    // O cabeçalho varia entre true e false em um byte. Cada item
    // adicional é maior que isso, então o tamanho cresce com o prefixo.
    let mut lower = 1;
    let mut upper = page.frames.len();
    let mut best = None;
    while lower <= upper {
        let count = lower + (upper - lower) / 2;
        match encode(count) {
            Ok(body) => {
                best = Some((count, body));
                lower = count + 1;
            }
            Err(WireError::LimitExceeded) => upper = count - 1,
            Err(_) => return Err(EffectError::Protocol),
        }
    }
    let (count, body) = best.ok_or(EffectError::Replication(ReplicationError::LimitExceeded))?;
    let last = i64::try_from(count)
        .ok()
        .and_then(|count| pull.after_seq.checked_add(count))
        .ok_or(EffectError::Replication(ReplicationError::SequenceGap))?;
    Ok(PreparedPage {
        body,
        through_seq: Some(last),
    })
}

fn error_on_wire(error: &EffectError) -> Option<ErrorCode> {
    use EffectError as Effect;
    use ReplicationError as Replica;
    Some(match error {
        Effect::CommitUnknown(_) | Effect::Protocol => return None,
        Effect::Authority | Effect::Fence | Effect::M1c(_) | Effect::M3(_) => {
            ErrorCode::AuthorityBlocked
        }
        Effect::Storage(_) | Effect::Timeout | Effect::Replication(Replica::Storage(_)) => {
            ErrorCode::StorageUnavailable
        }
        Effect::Replication(Replica::Policy) => ErrorCode::ResidenceDenied,
        Effect::Replication(Replica::ProofMissing) => ErrorCode::ProofMissing,
        Effect::Replication(Replica::ProofConflict | Replica::InvalidProof) => {
            ErrorCode::ProofConflict
        }
        Effect::Replication(Replica::SequenceGap) => ErrorCode::SequenceGap,
        Effect::Replication(Replica::HistoryUnavailable | Replica::RetentionExpired) => {
            ErrorCode::HistoryUnavailable
        }
        Effect::Replication(Replica::LimitExceeded) => ErrorCode::LimitExceeded,
        _ => ErrorCode::ScopeDenied,
    })
}

async fn serve_pull(
    service: &ReplicaService,
    remote: &Remote,
    stream: &mut OperationalBiStream,
) -> Result<(), AttemptError> {
    let deadline = stream.deadline();
    let config = service.config();
    let body = io(deadline, stream.read_frame()).await?;
    let Message::Pull(pull) = wire::decode_body(&body, config.max_frame_bytes)? else {
        return Err(AttemptError::Protocol);
    };
    let source = limits(config);
    let effective = source.component_min(pull.receiver_limits);
    let result = if pull.destination != remote.node_id || pull.namespace != namespace(config) {
        Err(EffectError::Replication(ReplicationError::AclDenied))
    } else {
        eprintln!(
            "event=m4_pull destination={} after_seq={}",
            remote.node_id, pull.after_seq
        );
        service
            .prepare_copy(
                remote.peer(),
                pull.after_seq,
                local_limits(effective)?,
                deadline,
                |page| prepare_page(&pull, source, page),
            )
            .await
    };
    match result {
        Ok(page) => {
            service.recheck(remote.peer(), deadline).await?;
            io(deadline, stream.write_frame(&page.body)).await?;
            finish(stream, deadline)?;
            if let Some(last) = page.through_seq {
                let raw = io(deadline, stream.read_frame()).await?;
                io(deadline, stream.expect_fin()).await?;
                let Message::Ack(ack) = wire::decode_body(&raw, effective.frame_bytes)? else {
                    return Err(AttemptError::Protocol);
                };
                wire::validate_ack(&pull, &ack, last)?;
                service.recheck(remote.peer(), deadline).await?;
                eprintln!(
                    "event=m4_ack ack_class=persisted_local destination={} through_seq={last}",
                    remote.node_id
                );
            } else {
                io(deadline, stream.expect_fin()).await?;
            }
        }
        Err(error) => {
            let Some(code) = error_on_wire(&error) else {
                return Err(error.into());
            };
            let message = Message::Error(ErrorMessage {
                request_id: pull.request_id,
                error: code,
            });
            let body = wire::encode_body(&message, effective.frame_bytes)?;
            io(deadline, stream.write_frame(&body)).await?;
            finish(stream, deadline)?;
            io(deadline, stream.expect_fin()).await?;
        }
    }
    Ok(())
}
