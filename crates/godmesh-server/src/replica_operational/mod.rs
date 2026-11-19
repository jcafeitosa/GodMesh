//! Efeitos M4-A2 após autenticação e correlação na fronteira da sessão.
//! Cada método abre e termina sua transação antes de entregar o resultado.
mod db;
mod link;
mod resume;
mod settings;

pub use link::{authorize_link_in_tx, authorize_link_until};
pub use settings::{ConfigError, Pin, ReplicaConfig, Role};

use std::{fmt, future::Future, sync::Arc, time::Duration};

use godmesh_federation::NodeIdentity;
use sqlx::Connection;
use tokio::time::{Instant, timeout, timeout_at};

use crate::{
    config_bundle::{BundleService, Failure, Fence, WriteError},
    portable_join::{ClusterGuard, PortableJoin, PortableJoinError},
    replication::{
        self, Applied, CopyAuthority, CopyPage, Limits, NodeContext, Replication, Series,
    },
    sql_budget,
};

/// Aplica o deadline às consultas que revalidam o orçamento SQL na transação.
/// O chamador controla o prazo externo e a limpeza da transação.
pub async fn with_sql_deadline<F: Future>(deadline: Instant, future: F) -> F::Output {
    sql_budget::DEADLINE.scope(deadline, future).await
}

#[derive(Clone, Copy, Debug)]
pub struct SessionPeer<'a> {
    pub node_id: &'a str,
    pub public_key: &'a str,
    pub protocol: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverState {
    Ready { after_seq: i64 },
    SuspendedHistory { blocked_next_seq: i64 },
}

#[derive(Debug)]
pub enum EffectError {
    Authority,
    Protocol,
    Fence,
    Replication(replication::Error),
    Storage(sqlx::Error),
    M1c(PortableJoinError),
    M3(WriteError),
    Timeout,
    CommitUnknown(Option<sqlx::Error>),
    ResumeInconsistent,
    StalePull,
    SuspendedHistory { blocked_next_seq: i64 },
}

impl EffectError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Timeout => true,
            Self::Storage(error) => match error {
                sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut => true,
                sqlx::Error::Database(error) => error.code().is_some_and(|code| {
                    code.starts_with("08")
                        || matches!(
                            code.as_ref(),
                            "40001" | "40P01" | "57P01" | "57P02" | "57P03"
                        )
                }),
                _ => false,
            },
            _ => false,
        }
    }
}

impl From<WriteError> for EffectError {
    fn from(value: WriteError) -> Self {
        match value {
            WriteError::Transient(Failure::Storage(error)) => error.into(),
            WriteError::Transient(Failure::Authority(error)) => error.into(),
            other => Self::M3(other),
        }
    }
}

impl From<sqlx::Error> for EffectError {
    fn from(error: sqlx::Error) -> Self {
        if matches!(&error, sqlx::Error::Io(error) if error.kind() == std::io::ErrorKind::TimedOut)
            || error
                .as_database_error()
                .and_then(|err| err.code())
                .is_some_and(|code| matches!(code.as_ref(), "57014" | "55P03"))
        {
            Self::Timeout
        } else {
            Self::Storage(error)
        }
    }
}
impl From<replication::Error> for EffectError {
    fn from(value: replication::Error) -> Self {
        match value {
            replication::Error::Storage(error) => error.into(),
            other => Self::Replication(other),
        }
    }
}
impl From<PortableJoinError> for EffectError {
    fn from(value: PortableJoinError) -> Self {
        match value {
            PortableJoinError::Storage(error) => error.into(),
            other => Self::M1c(other),
        }
    }
}
impl fmt::Display for EffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for EffectError {}

#[derive(Debug)]
pub enum PreflightError {
    Mismatch,
    Unavailable(EffectError),
}
impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch => f.write_str("m4_preflight_mismatch"),
            Self::Unavailable(_) => f.write_str("m4_preflight_unavailable"),
        }
    }
}
impl std::error::Error for PreflightError {}

pub struct ReplicaService {
    config: ReplicaConfig,
    portable: Arc<PortableJoin>,
    m3: Arc<BundleService>,
    local: Pin,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl ReplicaService {
    pub async fn preflight(
        config: ReplicaConfig,
        portable: Arc<PortableJoin>,
        m3: Arc<BundleService>,
        identity: &NodeIdentity,
    ) -> Result<Self, PreflightError> {
        let local = Pin {
            node_id: identity.node_id().to_owned(),
            public_key: identity.public_key_hex(),
        };
        if portable.identity().node_id() != local.node_id
            || portable.identity().public_key_hex() != local.public_key
            || portable.cluster() != config.cluster
            || m3.policy().bundle.cluster != config.cluster
            || match config.role {
                Role::Origin => local != config.origin_pin,
                Role::Receiver => local.node_id == config.origin_pin.node_id,
            }
        {
            return Err(PreflightError::Mismatch);
        }
        let service = Self {
            config,
            portable,
            m3,
            local,
            clock: Arc::new(godmesh_federation::now_unix),
        };
        let deadline = Instant::now() + service.config.attempt_timeout;
        service
            .transact(deadline, async |guard, _| {
                Replication::verify_reservation_in_tx(
                    guard,
                    service.series(),
                    service.local(),
                    service.config.role == Role::Origin,
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(|error| match error {
                EffectError::Replication(
                    replication::Error::NotReserved
                    | replication::Error::Authority
                    | replication::Error::InvalidReservation
                    | replication::Error::ReservationConflict,
                ) => PreflightError::Mismatch,
                other => PreflightError::Unavailable(other),
            })?;
        Ok(service)
    }

    pub fn local_pin(&self) -> &Pin {
        &self.local
    }

    pub fn config(&self) -> &ReplicaConfig {
        &self.config
    }

    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }

    pub async fn recheck(
        &self,
        peer: SessionPeer<'_>,
        deadline: Instant,
    ) -> Result<(), EffectError> {
        let authority = self.authority(peer, self.local_limits())?;
        self.transact(deadline, async |guard, now| {
            self.gates(guard, peer, now).await?;
            Replication::recheck_copy_in_tx(
                guard,
                self.series(),
                authority,
                self.config.role == Role::Origin,
                now,
            )
            .await?;
            Ok(())
        })
        .await
    }

    pub async fn checkpoint(
        &self,
        peer: SessionPeer<'_>,
        deadline: Instant,
    ) -> Result<ReceiverState, EffectError> {
        self.require_role(Role::Receiver)?;
        let authority = self.authority(peer, self.local_limits())?;
        self.transact(deadline, async |guard, now| {
            self.gates(guard, peer, now).await?;
            self.check_receiver(guard, authority, now).await
        })
        .await
    }

    pub async fn prepare_copy<T>(
        &self,
        peer: SessionPeer<'_>,
        after_seq: i64,
        limits: Limits,
        deadline: Instant,
        prepare: impl FnOnce(CopyPage) -> Result<T, EffectError>,
    ) -> Result<T, EffectError> {
        self.require_role(Role::Origin)?;
        let first = after_seq
            .checked_add(1)
            .filter(|n| *n > 0)
            .ok_or(replication::Error::SequenceGap)?;
        let authority = self.authority(peer, self.bound_limits(limits))?;
        self.transact(deadline, async |guard, now| {
            self.gates(guard, peer, now).await?;
            let page =
                Replication::select_copy_in_tx(guard, self.series(), authority, first, || now)
                    .await?;
            prepare(page)
        })
        .await
    }

    pub async fn apply_copy(
        &self,
        peer: SessionPeer<'_>,
        limits: Limits,
        frames: &[Vec<u8>],
        deadline: Instant,
    ) -> Result<Applied, EffectError> {
        self.require_role(Role::Receiver)?;
        let authority = self.authority(peer, self.bound_limits(limits))?;
        self.transact(deadline, async |guard, now| {
            self.gates(guard, peer, now).await?;
            if let ReceiverState::SuspendedHistory { blocked_next_seq } =
                self.check_receiver(guard, authority, now).await?
            {
                return Err(EffectError::SuspendedHistory { blocked_next_seq });
            }

            Ok(
                Replication::apply_copy_in_tx(guard, self.series(), authority, frames, || now)
                    .await?,
            )
        })
        .await
    }

    /// Recebe exclusivamente o contexto de um erro de histórico já correlacionado
    /// pelo adaptador (mesma sessão/stream, request_id e FIN).
    pub async fn suspend_history(
        &self,
        peer: SessionPeer<'_>,
        after_seq: i64,
        deadline: Instant,
    ) -> Result<ReceiverState, EffectError> {
        self.require_role(Role::Receiver)?;
        let authority = self.authority(peer, self.local_limits())?;
        self.transact(deadline, async |guard, now| {
            self.gates(guard, peer, now).await?;
            let state = self.check_receiver(guard, authority, now).await?;
            let next = after_seq
                .checked_add(1)
                .filter(|seq| *seq > 0)
                .ok_or(EffectError::StalePull)?;
            match state {
                ReceiverState::Ready { after_seq: current } if current == after_seq => {
                    resume::insert(guard.connection(), &self.local.node_id, self.series(), next)
                        .await?;
                    Ok(ReceiverState::SuspendedHistory {
                        blocked_next_seq: next,
                    })
                }
                ReceiverState::SuspendedHistory { blocked_next_seq }
                    if blocked_next_seq == next =>
                {
                    Ok(state)
                }
                _ => Err(EffectError::StalePull),
            }
        })
        .await
    }

    async fn check_receiver(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        authority: CopyAuthority<'_>,
        now: i64,
    ) -> Result<ReceiverState, EffectError> {
        Replication::recheck_copy_in_tx(guard, self.series(), authority, false, now).await?;

        let prefix =
            Replication::receiver_checkpoint_in_tx(guard, self.series(), self.local()).await?;

        resume::state(
            guard.connection(),
            &self.local.node_id,
            self.series(),
            prefix,
        )
        .await
    }

    async fn transact<T>(
        &self,
        deadline: Instant,
        effect: impl AsyncFnOnce(&mut ClusterGuard<'_, '_>, i64) -> Result<T, EffectError>,
    ) -> Result<T, EffectError> {
        let mut lease = db::Lease::acquire(self.portable.pool(), deadline).await?;
        let mut tx = timeout_at(
            deadline,
            lease.begin_with("BEGIN ISOLATION LEVEL READ COMMITTED"),
        )
        .await
        .map_err(|_| EffectError::Timeout)??;
        let decision = timeout_at(
            deadline,
            with_sql_deadline(deadline, async {
                let mut guard = self.portable.lock_cluster_guard(&mut tx).await?;
                replication::locked(&mut guard, &self.config.namespace).await?;
                let now = (self.clock)();

                let prepared = effect(&mut guard, now).await?;
                sql_budget::refresh(guard.connection()).await?;

                Ok(prepared)
            }),
        )
        .await
        .unwrap_or(Err(EffectError::Timeout));

        let decision = decision.and_then(|prepared| {
            if Instant::now() >= deadline {
                Err(EffectError::Timeout)
            } else {
                Ok(prepared)
            }
        });
        match decision {
            Ok(prepared) => {
                db::commit(tx, deadline).await?;
                lease.clean();
                Ok(prepared)
            }
            Err(error) => {
                if matches!(
                    timeout(Duration::from_secs(1), tx.rollback()).await,
                    Ok(Ok(()))
                ) {
                    lease.clean();
                }
                Err(error)
            }
        }
    }

    async fn gates(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        peer: SessionPeer<'_>,
        now: i64,
    ) -> Result<(), EffectError> {
        self.portable.authorize_local_at(guard, now).await?;

        self.portable
            .authorize_peer_at(guard, peer.node_id, peer.public_key, now)
            .await?;

        let fence = self
            .m3
            .classify_fence_in_tx(guard, peer.protocol, now)
            .await?;
        if !matches!(fence, Fence::Allowed { .. }) {
            return Err(EffectError::Fence);
        }
        Ok(())
    }

    fn authority<'a>(
        &'a self,
        peer: SessionPeer<'a>,
        limits: Limits,
    ) -> Result<CopyAuthority<'a>, EffectError> {
        let same = |pin: &Pin| pin.node_id == peer.node_id && pin.public_key == peer.public_key;
        let remote = NodeContext {
            node_id: peer.node_id,
            public_key: peer.public_key,
        };
        match self.config.role {
            Role::Origin if self.config.destination_pins.iter().any(same) => Ok(CopyAuthority {
                origin: self.local(),
                destination: remote,
                limits,
            }),
            Role::Receiver if same(&self.config.origin_pin) => Ok(CopyAuthority {
                origin: remote,
                destination: self.local(),
                limits,
            }),
            _ => Err(EffectError::Authority),
        }
    }

    fn series(&self) -> Series<'_> {
        Series {
            namespace: &self.config.namespace,
            epoch: self.config.authority_epoch,
        }
    }

    fn local(&self) -> NodeContext<'_> {
        NodeContext {
            node_id: &self.local.node_id,
            public_key: &self.local.public_key,
        }
    }

    fn require_role(&self, role: Role) -> Result<(), EffectError> {
        if self.config.role != role {
            return Err(EffectError::Authority);
        }
        Ok(())
    }

    fn local_limits(&self) -> Limits {
        Limits::new(
            self.config.max_events.get(),
            self.config.max_envelope_bytes.get(),
            self.config.max_frame_bytes.get() as usize,
        )
        .expect("limites positivos validados")
    }

    fn bound_limits(&self, limits: Limits) -> Limits {
        limits.component_min(self.local_limits())
    }
}
