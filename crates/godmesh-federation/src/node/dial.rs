//! Discagem pinada e ciclo de vida da tentativa.
use std::fmt;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use quinn::{Connecting, Connection};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

#[cfg(feature = "relay-harness")]
use super::session::RelayValidity;
use super::{LinkError, NodeShared, close_with_rejection, establish_pinned};
use crate::handshake::Rejection;
use crate::identity::node_id_for;
use crate::link::{LinkSnapshot, LinkState, OperationalValidity, PeerLinks};
use crate::relay_fallback::{FallbackError, ReapedDirect};
use crate::transport;

/// Alvo selecionado de um único registro do par.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialTarget {
    pub node_id: String,
    pub public_key: [u8; 32],
    pub addr: SocketAddr,
}

/// Resultado da disputa entre cancelamento e promoção.
pub enum DialCancellation {
    Cancelled,
    AlreadyPromoted(PromotedDial),
    Finished,
}

/// Falha de uma tentativa pinada.
#[derive(Debug)]
pub enum DialError {
    InvalidTarget,
    PinMismatch,
    Cancelled,
    Finished,
    Link(LinkError),
}

impl fmt::Display for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget => f.write_str("alvo de discagem inválido"),
            Self::PinMismatch => f.write_str("chave TLS difere do pin da discagem"),
            Self::Cancelled => f.write_str("discagem cancelada"),
            Self::Finished => f.write_str("tentativa já recolhida"),
            Self::Link(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DialError {}

/// Capacidade de invalidar somente a conexão promovida pela tentativa.
#[derive(Clone)]
pub struct PromotedDial {
    connection: Connection,
    node_id: String,
    links: Arc<PeerLinks>,
    snapshot: LinkSnapshot,
}

impl PromotedDial {
    /// Fotografia no ponto da promoção, ou `Unknown` se obtida pelo
    /// cancelamento de uma discagem relay cuja rota já ficou inválida.
    pub fn snapshot(&self) -> &LinkSnapshot {
        &self.snapshot
    }

    /// Fecha a conexão original e retira somente o estado dessa conexão.
    pub fn invalidate(&self) {
        close_with_rejection(&self.connection, Rejection::Revoked);
        self.links.close_connection(&self.node_id, &self.connection);
    }
}

/// Tarefa de discagem em voo. Descartar cancela apenas antes da promoção.
pub struct DialAttempt {
    binding: (Arc<()>, DialTarget),
    state: Arc<AttemptState>,
    task: Option<JoinHandle<Result<PromotedDial, DialError>>>,
}

impl DialAttempt {
    pub(super) fn spawn(
        shared: NodeShared,
        target: DialTarget,
        connecting: Connecting,
        state: Arc<AttemptState>,
    ) -> Self {
        let binding = (Arc::new(()), target.clone());
        let guard = TaskGuard(Arc::clone(&state));
        let task = tokio::spawn(async move { guard.run(shared, target, connecting).await });
        Self {
            binding,
            state,
            task: Some(task),
        }
    }

    /// Cancela enquanto pendente. Se a promoção já venceu, devolve a
    /// capacidade de invalidar a conexão exata sob a política do chamador.
    pub fn cancel(&self) -> DialCancellation {
        if let Some(result) = self.state.while_relay_current(|| self.state.cancel()) {
            return result;
        }
        match self.state.cancel() {
            DialCancellation::AlreadyPromoted(mut promoted) => {
                promoted.snapshot.state = LinkState::Unknown;
                promoted.snapshot.rtt_ms = None;
                DialCancellation::AlreadyPromoted(promoted)
            }
            result => result,
        }
    }

    pub(crate) fn fallback_binding(&self) -> (&Arc<()>, &DialTarget) {
        (&self.binding.0, &self.binding.1)
    }

    /// Cancela e recolhe a direta para a arbitragem de fallback da fixture.
    ///
    /// # Errors
    /// Recusa uma direta promovida, já recolhida ou com resultado distinto
    /// de cancelamento observado antes de TLS. Nenhum erro Quinn vira prova.
    pub async fn reap_for_fallback(&mut self) -> Result<ReapedDirect, FallbackError> {
        let cancellation = self.cancel();
        let result = self.wait().await;
        match (cancellation, result) {
            (DialCancellation::AlreadyPromoted(_), _) | (_, Ok(_)) => {
                Err(FallbackError::DirectPromoted)
            }
            (DialCancellation::Cancelled, Err(DialError::Cancelled))
                if self
                    .state
                    .cancelled_while_connecting
                    .load(Ordering::Relaxed) =>
            {
                Ok(ReapedDirect::new(
                    Arc::clone(&self.binding.0),
                    self.binding.1.clone(),
                ))
            }
            (_, Err(DialError::Cancelled | DialError::Finished)) => {
                Err(FallbackError::DirectNotCancelled)
            }
            (_, Err(error)) => Err(FallbackError::DirectFailed(error)),
        }
    }

    /// Recolhe a tarefa. Descartar apenas esta espera permite aguardá-la de novo.
    /// Uma discagem relay só entrega a promoção enquanto a rota ainda é válida.
    pub async fn wait(&mut self) -> Result<PromotedDial, DialError> {
        let task = self.task.as_mut().ok_or(DialError::Finished)?;
        let result = task.await;
        self.task = None;
        let promoted = result.map_err(|_| {
            DialError::Link(LinkError::Connect("tarefa de discagem interrompida".into()))
        })??;
        self.state
            .while_relay_current(|| promoted)
            .ok_or(DialError::Cancelled)
    }
}

impl Drop for DialAttempt {
    fn drop(&mut self) {
        self.state.cancel();
    }
}

enum Phase {
    Pending(Option<Connection>),
    Cancelled,
    Promoted(PromotedDial),
    Finished,
}

pub(super) struct AttemptState {
    phase: Mutex<Phase>,
    cancelled: Notify,
    cancelled_while_connecting: AtomicBool,
    #[cfg(feature = "relay-harness")]
    relay: Option<std::sync::Weak<RelayValidity>>,
}

impl AttemptState {
    pub(super) fn new() -> Self {
        Self {
            phase: Mutex::new(Phase::Pending(None)),
            cancelled: Notify::new(),
            cancelled_while_connecting: AtomicBool::new(false),
            #[cfg(feature = "relay-harness")]
            relay: None,
        }
    }

    #[cfg(feature = "relay-harness")]
    pub(super) fn for_relay(route: &Arc<RelayValidity>) -> Self {
        Self {
            relay: Some(Arc::downgrade(route)),
            ..Self::new()
        }
    }

    pub(super) fn relay_outer(&self) -> Option<Arc<OperationalValidity>> {
        #[cfg(feature = "relay-harness")]
        if let Some(route) = self.relay.as_ref().and_then(std::sync::Weak::upgrade) {
            return Some(route.outer().clone());
        }
        None
    }

    #[cfg(feature = "relay-harness")]
    pub(super) fn invalidate_relay(&self) {
        if let DialCancellation::AlreadyPromoted(promoted) = self.cancel() {
            promoted.invalidate();
        }
    }

    #[cfg(feature = "relay-harness")]
    pub(super) fn still_tracked(&self) -> bool {
        match &*self.lock() {
            Phase::Pending(_) => true,
            Phase::Promoted(promoted) => promoted.connection.close_reason().is_none(),
            _ => false,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Phase> {
        self.phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn cancel(&self) -> DialCancellation {
        let mut phase = self.lock();
        match &*phase {
            Phase::Pending(connection) => {
                if let Some(connection) = connection {
                    cancel_connection(connection);
                }
                *phase = Phase::Cancelled;
                self.cancelled.notify_one();
                DialCancellation::Cancelled
            }
            Phase::Cancelled => DialCancellation::Cancelled,
            Phase::Promoted(promoted) => DialCancellation::AlreadyPromoted(promoted.clone()),
            Phase::Finished => DialCancellation::Finished,
        }
    }

    fn attach(&self, connection: &Connection) -> Result<(), DialError> {
        let mut phase = self.lock();
        if let Phase::Pending(slot) = &mut *phase {
            *slot = Some(connection.clone());
            Ok(())
        } else {
            cancel_connection(connection);
            Err(DialError::Cancelled)
        }
    }

    pub(super) fn promote(
        &self,
        shared: &NodeShared,
        connection: &Connection,
        node_id: &str,
        publish: impl FnOnce() -> Result<LinkSnapshot, Rejection>,
    ) -> Result<LinkSnapshot, Rejection> {
        self.while_relay_current(|| self.publish(shared, connection, node_id, publish))
            .unwrap_or_else(|| {
                self.cancel();
                Err(Rejection::Transport)
            })
    }

    fn while_relay_current<T>(&self, operation: impl FnOnce() -> T) -> Option<T> {
        #[cfg(feature = "relay-harness")]
        if let Some(relay) = &self.relay {
            return relay.upgrade()?.while_active(operation);
        }
        Some(operation())
    }

    fn publish(
        &self,
        shared: &NodeShared,
        connection: &Connection,
        node_id: &str,
        publish: impl FnOnce() -> Result<LinkSnapshot, Rejection>,
    ) -> Result<LinkSnapshot, Rejection> {
        let mut phase = self.lock();
        if !matches!(&*phase, Phase::Pending(Some(pending)) if pending.stable_id() == connection.stable_id())
            || connection.close_reason().is_some()
        {
            return Err(Rejection::Transport);
        }
        let snapshot = publish()?;
        *phase = Phase::Promoted(PromotedDial {
            connection: connection.clone(),
            node_id: node_id.to_owned(),
            links: Arc::clone(&shared.links),
            snapshot: snapshot.clone(),
        });
        Ok(snapshot)
    }
}

struct TaskGuard(Arc<AttemptState>);

impl TaskGuard {
    async fn run(
        self,
        shared: NodeShared,
        target: DialTarget,
        connecting: Connecting,
    ) -> Result<PromotedDial, DialError> {
        let result = self.connect(&shared, &target, connecting).await;
        self.complete(result, || shared.links.mark_rejected(&target.node_id))
    }

    fn complete(
        &self,
        result: Result<(), DialError>,
        mark_rejected: impl FnOnce(),
    ) -> Result<PromotedDial, DialError> {
        let mut phase = self.0.lock();
        match &*phase {
            Phase::Cancelled => Err(result.err().unwrap_or(DialError::Cancelled)),
            Phase::Promoted(promoted) => Ok(promoted.clone()),
            Phase::Pending(connection) => {
                if let Some(connection) = connection {
                    cancel_connection(connection);
                }
                if matches!(&result, Err(DialError::Link(LinkError::Rejected { .. }))) {
                    mark_rejected();
                }
                *phase = Phase::Finished;
                Err(result.err().unwrap_or(DialError::Finished))
            }
            Phase::Finished => Err(DialError::Finished),
        }
    }

    async fn connect(
        &self,
        shared: &NodeShared,
        target: &DialTarget,
        connecting: Connecting,
    ) -> Result<(), DialError> {
        let connection = tokio::select! {
            biased;
            // Se o TLS já concluiu, obtenha a conexão para attach fechá-la
            // explicitamente quando o cancelamento já venceu.
            result = connecting => result.map_err(|error| DialError::Link(LinkError::Connect(error.to_string())))?,
            _ = self.0.cancelled.notified() => {
                self.0.cancelled_while_connecting.store(true, Ordering::Relaxed);
                return Err(DialError::Cancelled);
            }
        };
        self.0.attach(&connection)?;
        if transport::peer_public_key(&connection) != Some(target.public_key) {
            close_with_rejection(&connection, Rejection::KeyMismatch);
            return Err(DialError::PinMismatch);
        }
        establish_pinned(shared, &connection, &self.0)
            .await
            .map(|_| ())
            .map_err(|error| {
                // O fechamento da conexão anexada por cancel() interrompe o
                // handshake de aplicação. Classifique aqui, antes de recolher.
                if error.rejection == Rejection::Transport
                    && matches!(*self.0.lock(), Phase::Cancelled)
                {
                    DialError::Cancelled
                } else {
                    DialError::Link(super::handshake_link_error(error))
                }
            })
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let mut phase = self.0.lock();
        if let Phase::Pending(connection) = &*phase {
            if let Some(connection) = connection {
                cancel_connection(connection);
            }
            *phase = Phase::Finished;
        }
    }
}

pub(super) fn validate_target(target: &DialTarget) -> Result<(), DialError> {
    if node_id_for(&target.public_key) != target.node_id {
        return Err(DialError::InvalidTarget);
    }
    Ok(())
}

fn cancel_connection(connection: &Connection) {
    if connection.close_reason().is_none() {
        connection.close(0u32.into(), b"discagem cancelada");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completed_failure_survives_cancellation_before_reaping() {
        for (case, error) in [
            DialError::PinMismatch,
            DialError::Link(LinkError::Connect("TLS".into())),
            DialError::Link(LinkError::Rejected {
                code: 7,
                reason: "authority",
            }),
        ]
        .into_iter()
        .enumerate()
        {
            let state = Arc::new(AttemptState::new());
            let ready = Arc::new(tokio::sync::Barrier::new(2));
            let collect = Arc::new(tokio::sync::Barrier::new(2));
            let task = {
                let guard = TaskGuard(Arc::clone(&state));
                let ready = Arc::clone(&ready);
                let collect = Arc::clone(&collect);
                tokio::spawn(async move {
                    let result = Err(error);
                    ready.wait().await;
                    collect.wait().await;
                    guard.complete(result, || panic!("cancelamento não atualiza o enlace"))
                })
            };
            let mut attempt = DialAttempt {
                binding: (
                    Arc::new(()),
                    DialTarget {
                        node_id: "test".into(),
                        public_key: [0; 32],
                        addr: "127.0.0.1:1".parse().unwrap(),
                    },
                ),
                state,
                task: Some(task),
            };
            ready.wait().await;
            assert!(matches!(attempt.cancel(), DialCancellation::Cancelled));
            collect.wait().await;
            let Err(FallbackError::DirectFailed(actual)) = attempt.reap_for_fallback().await else {
                panic!("resultado concluído perdido no recolhimento");
            };
            assert!(matches!(
                (case, actual),
                (0, DialError::PinMismatch)
                    | (1, DialError::Link(LinkError::Connect(_)))
                    | (2, DialError::Link(LinkError::Rejected { code: 7, .. }))
            ));
        }
    }
}
