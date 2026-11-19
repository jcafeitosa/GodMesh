//! Discagem pinada e ciclo de vida da tentativa.
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use quinn::{Connecting, Connection};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::{LinkError, NodeShared, close_with_rejection, establish_pinned};
use crate::handshake::Rejection;
use crate::identity::node_id_for;
use crate::link::{LinkSnapshot, PeerLinks};
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
    /// Fotografia no ponto da promoção.
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
    state: Arc<AttemptState>,
    task: Option<JoinHandle<Result<PromotedDial, DialError>>>,
}

impl DialAttempt {
    pub(super) fn spawn(shared: NodeShared, target: DialTarget, connecting: Connecting) -> Self {
        let state = Arc::new(AttemptState {
            phase: Mutex::new(Phase::Pending(None)),
            cancelled: Notify::new(),
        });
        let guard = TaskGuard(Arc::clone(&state));
        let task = tokio::spawn(async move { guard.run(shared, target, connecting).await });
        Self {
            state,
            task: Some(task),
        }
    }

    /// Cancela enquanto pendente. Se a promoção já venceu, devolve a
    /// capacidade de invalidar a conexão exata sob a política do chamador.
    pub fn cancel(&self) -> DialCancellation {
        self.state.cancel()
    }

    /// Recolhe a tarefa. Descartar apenas esta espera permite aguardá-la de novo.
    pub async fn wait(&mut self) -> Result<PromotedDial, DialError> {
        let task = self.task.as_mut().ok_or(DialError::Finished)?;
        let result = task.await;
        self.task = None;
        result.map_err(|_| {
            DialError::Link(LinkError::Connect("tarefa de discagem interrompida".into()))
        })?
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
}

impl AttemptState {
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
        let result = self.connect(shared, target, connecting).await;
        let mut phase = self.0.lock();
        match &*phase {
            Phase::Cancelled => Err(DialError::Cancelled),
            Phase::Promoted(promoted) => Ok(promoted.clone()),
            Phase::Pending(connection) => {
                if let Some(connection) = connection {
                    cancel_connection(connection);
                }
                *phase = Phase::Finished;
                Err(result.err().unwrap_or(DialError::Finished))
            }
            Phase::Finished => Err(DialError::Finished),
        }
    }

    async fn connect(
        &self,
        shared: NodeShared,
        target: DialTarget,
        connecting: Connecting,
    ) -> Result<(), DialError> {
        let connection = tokio::select! {
            biased;
            // Se o TLS já concluiu, obtenha a conexão para attach fechá-la
            // explicitamente quando o cancelamento já venceu.
            result = connecting => result.map_err(|error| DialError::Link(LinkError::Connect(error.to_string())))?,
            _ = self.0.cancelled.notified() => return Err(DialError::Cancelled),
        };
        self.0.attach(&connection)?;
        if transport::peer_public_key(&connection) != Some(target.public_key) {
            close_with_rejection(&connection, Rejection::KeyMismatch);
            return Err(DialError::PinMismatch);
        }
        establish_pinned(&shared, &connection, &self.0)
            .await
            .map(|_| ())
            .map_err(|error| DialError::Link(super::handshake_link_error(error)))
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
