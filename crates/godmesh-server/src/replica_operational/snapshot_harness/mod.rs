//! Exclusivo dos ensaios em bancos criados pela fixture. O binário não contém
//! um ponto de entrada para aplicar snapshots.
mod intent;
mod receipt;
mod reconcile;
#[cfg(test)]
mod tests;

pub use intent::AttemptId;
pub use reconcile::{HarnessHistory, Recovery};

use godmesh_federation::{
    NodeIdentity,
    snapshot::{self, SnapshotError, SnapshotManifest, SnapshotPolicy, VerifiedSnapshot},
};
use sha2::{Digest, Sha256};

use super::{
    EffectError, Instant, ReceiverState, ReplicaService, Role, SessionPeer,
    replication::{Error as ReplicaError, Limits, Replication, snapshot_read},
};

#[derive(Debug)]
pub enum SnapshotEffectError {
    Effect(EffectError),
    Codec(SnapshotError),
    Scope,
    Window,
    LimitExceeded,
    ReceiptPolicy,
    ReceiptExpired,
    ReceiptMissing,
    ReceiptInconsistent,
    SnapshotConflict,
    CheckpointMismatch,
    Eligibility,
    IntentPending,
    AttemptMismatch,
    IntentExpired,
    IntentPolicy,
    IntentInconsistent,
}

impl From<EffectError> for SnapshotEffectError {
    fn from(error: EffectError) -> Self {
        Self::Effect(error)
    }
}
impl From<sqlx::Error> for SnapshotEffectError {
    fn from(error: sqlx::Error) -> Self {
        EffectError::from(error).into()
    }
}
impl From<ReplicaError> for SnapshotEffectError {
    fn from(error: ReplicaError) -> Self {
        match error {
            ReplicaError::LimitExceeded => Self::LimitExceeded,
            other => EffectError::from(other).into(),
        }
    }
}
impl From<SnapshotError> for SnapshotEffectError {
    fn from(error: SnapshotError) -> Self {
        match error {
            SnapshotError::TooManyEvents
            | SnapshotError::ManifestTooLarge
            | SnapshotError::EventTooLarge
            | SnapshotError::PackageTooLarge => Self::LimitExceeded,
            other => Self::Codec(other),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub not_before: i64,
    pub expires_at: i64,
}

impl Window {
    fn check(self, now: i64) -> Result<(), SnapshotEffectError> {
        if self.not_before <= now && now < self.expires_at {
            Ok(())
        } else {
            Err(SnapshotEffectError::Window)
        }
    }
}

fn deadline_check(deadline: Instant) -> Result<(), SnapshotEffectError> {
    if Instant::now() >= deadline {
        Err(EffectError::Timeout.into())
    } else {
        Ok(())
    }
}

fn check_scope(
    svc: &ReplicaService,
    peer: SessionPeer<'_>,
    policy: &SnapshotPolicy,
    role: Role,
) -> Result<(), SnapshotEffectError> {
    let local = svc.local_pin();
    let (own, other) = match role {
        Role::Origin => (&policy.origin, &policy.destination),
        Role::Receiver => (&policy.destination, &policy.origin),
    };
    let ns = &svc.config().namespace;
    if own.node_id != local.node_id
        || own.public_key != local.public_key
        || other.node_id != peer.node_id
        || other.public_key != peer.public_key
        || policy.cluster != svc.config().cluster
        || policy.tenant_id != ns.tenant_id
        || policy.stream_id != ns.stream_id
        || i64::from(policy.partition) != i64::from(ns.partition)
        || i64::try_from(policy.authority_epoch) != Ok(svc.config().authority_epoch)
    {
        return Err(SnapshotEffectError::Scope);
    }
    svc.require_role(role)?;
    Ok(())
}

fn snapshot_limits(policy: &SnapshotPolicy, bytes: usize) -> Result<Limits, SnapshotEffectError> {
    Ok(Limits::new(
        policy.limits.max_events.get() as usize,
        policy.limits.max_event_bytes.get() as usize,
        bytes,
    )?)
}

fn manifest_size(
    manifest: &SnapshotManifest,
    policy: &SnapshotPolicy,
) -> Result<usize, SnapshotEffectError> {
    let json = serde_json::to_vec(manifest).map_err(|_| SnapshotError::MalformedManifest)?;
    if json.len() > policy.limits.max_manifest_bytes.get() as usize {
        return Err(SnapshotEffectError::LimitExceeded);
    }
    json.len()
        .checked_add(4)
        .ok_or(SnapshotEffectError::LimitExceeded)
}

pub async fn capture(
    svc: &ReplicaService,
    signer: &NodeIdentity,
    peer: SessionPeer<'_>,
    base: i64,
    window: Window,
    policy: &SnapshotPolicy,
    deadline: Instant,
) -> Result<Vec<u8>, SnapshotEffectError> {
    check_scope(svc, peer, policy, Role::Origin)?;
    if signer.node_id() != policy.origin.node_id
        || signer.public_key_hex() != policy.origin.public_key
    {
        return Err(SnapshotEffectError::Scope);
    }
    svc.transact_with(deadline, async |guard, now| {
        svc.gates(guard, peer, now).await?;
        window.check(now)?;
        let (class, _, cut) = snapshot_read::scope(guard, svc.series()).await?;
        if class != policy.retention_class {
            return Err(SnapshotEffectError::Scope);
        }
        let count = cut
            .checked_sub(base)
            .filter(|n| base >= 0 && *n > 0)
            .ok_or(ReplicaError::SequenceGap)?;
        let mut manifest = SnapshotManifest {
            profile: 1,
            cluster: policy.cluster.clone(),
            origin_node: policy.origin.node_id.clone(),
            destination_node: policy.destination.node_id.clone(),
            tenant_id: policy.tenant_id.clone(),
            stream_id: policy.stream_id.clone(),
            partition: policy.partition,
            authority_epoch: policy.authority_epoch,
            base_seq: base as u64,
            cut_seq: cut as u64,
            event_count: u32::try_from(count).unwrap_or(u32::MAX),
            not_before: window.not_before,
            expires_at: window.expires_at,
            events_digest: "0".repeat(64),
            signature: "0".repeat(128),
        };
        // O número de dígitos do u32::MAX também cobre qualquer corte que
        // ultrapasse a representação B1. A seleção ainda decide a primeira falha.
        let overhead = manifest_size(&manifest, policy)?;
        let budget = policy
            .limits
            .max_package_bytes
            .get()
            .checked_sub(overhead)
            .filter(|n| *n > 0)
            .ok_or(SnapshotEffectError::LimitExceeded)?;
        let authority = svc.authority(peer, snapshot_limits(policy, budget)?)?;
        let frames =
            snapshot_read::select_closed(guard, svc.series(), authority, base, cut, now).await?;
        let events: Vec<_> = frames.iter().map(Vec::as_slice).collect();
        if i64::try_from(events.len()) != Ok(count) {
            return Err(SnapshotEffectError::LimitExceeded);
        }
        manifest.events_digest = hex::encode(snapshot::events_digest(&events)?);
        manifest.canonical_bytes()?;
        let exact = events
            .iter()
            .try_fold(manifest_size(&manifest, policy)?, |len, bytes| {
                len.checked_add(4)
                    .and_then(|n| n.checked_add(bytes.len()))
                    .ok_or(SnapshotEffectError::LimitExceeded)
            })?;
        if exact > policy.limits.max_package_bytes.get() {
            return Err(SnapshotEffectError::LimitExceeded);
        }
        deadline_check(deadline)?;
        manifest.sign(signer)?;
        let package = snapshot::encode_package(&manifest, &events, policy)?;
        if package.len() != exact {
            return Err(SnapshotEffectError::LimitExceeded);
        }
        Ok(package)
    })
    .await
}

/// Só a fixture que criou o banco e conhece seu histórico constrói este tipo.
/// Sua visibilidade fica dentro do módulo que só existe nos testes.
pub struct EligibleReceiver(std::sync::Arc<ReplicaService>, std::num::NonZeroU64);

#[derive(Debug, PartialEq, Eq)]
pub struct PersistedLocal {
    pub first: i64,
    pub through: i64,
    pub replay: bool,
}

impl EligibleReceiver {
    pub async fn apply(
        &self,
        peer: SessionPeer<'_>,
        attempt: AttemptId,
        package: &[u8],
        policy: &SnapshotPolicy,
        deadline: Instant,
    ) -> Result<PersistedLocal, SnapshotEffectError> {
        self.apply_inner(
            peer,
            package,
            policy,
            deadline,
            Some(attempt),
            snapshot::verify_snapshot,
        )
        .await
    }

    #[cfg(test)]
    async fn apply_b2a(
        &self,
        peer: SessionPeer<'_>,
        package: &[u8],
        policy: &SnapshotPolicy,
        deadline: Instant,
    ) -> Result<PersistedLocal, SnapshotEffectError> {
        self.apply_inner(
            peer,
            package,
            policy,
            deadline,
            None,
            snapshot::verify_snapshot,
        )
        .await
    }

    async fn apply_inner<'a>(
        &self,
        peer: SessionPeer<'_>,
        package: &'a [u8],
        policy: &SnapshotPolicy,
        deadline: Instant,
        attempt: Option<AttemptId>,
        verify: impl FnOnce(&'a [u8], &SnapshotPolicy) -> Result<VerifiedSnapshot<'a>, SnapshotError>,
    ) -> Result<PersistedLocal, SnapshotEffectError> {
        let svc = &self.0;
        check_scope(svc, peer, policy, Role::Receiver)?;
        deadline_check(deadline)?;
        let verified = verify(package, policy)?;
        deadline_check(deadline)?;
        let facts = package_facts(package, &verified)?;
        let base = facts.receipt.base;
        let cut = facts.cut;
        let first = base + 1;
        let events = verified.events();
        let window = facts.window;
        deadline_check(deadline)?;
        let authority = svc.authority(
            peer,
            snapshot_limits(policy, policy.limits.max_package_bytes.get())?,
        )?;
        let local = &svc.local_pin().node_id;
        svc.transact_with(deadline, async |guard, now| {
            svc.gates(guard, peer, now).await?;
            let state = if attempt.is_some() {
                svc.receiver_state_in_tx(guard, authority, now).await?
            } else {
                svc.check_receiver(guard, authority, now).await?
            };
            let prefix = ready(state)?;
            window.check(now)?;
            let (class, region, _) = snapshot_read::scope(guard, svc.series()).await?;
            if class != policy.retention_class {
                return Err(SnapshotEffectError::Scope);
            }
            if let Some(id) = attempt {
                let active =
                    super::intent_marker::pending(guard.connection(), local, svc.series()).await?;
                if active != Some(id.get()) {
                    return Err(SnapshotEffectError::AttemptMismatch);
                }
                let detail = intent::load(guard.connection(), local, svc.series(), id)
                    .await?
                    .ok_or(SnapshotEffectError::IntentInconsistent)?;
                if detail.facts != facts {
                    return Err(SnapshotEffectError::SnapshotConflict);
                }
                reconcile::check_detail(
                    guard.connection(),
                    local,
                    svc.series(),
                    &region,
                    &detail,
                    now,
                )
                .await?;
                if now >= detail.expires_at {
                    return Err(SnapshotEffectError::IntentExpired);
                }
            }
            let current =
                receipt::policy(guard.connection(), local, svc.series(), &region, None).await?;
            if let Some(stored) =
                receipt::load(guard.connection(), local, svc.series(), cut).await?
            {
                if stored.identity != facts.receipt {
                    return Err(SnapshotEffectError::SnapshotConflict);
                }
                if attempt.is_some() {
                    return Err(SnapshotEffectError::CheckpointMismatch);
                }
                reconcile::check_receipt(
                    guard.connection(),
                    local,
                    svc.series(),
                    &region,
                    &stored,
                    window,
                    now,
                )
                .await?;
                if prefix != cut {
                    return Err(SnapshotEffectError::CheckpointMismatch);
                }
                let expiry = snapshot_read::exact_local_expiry(
                    guard,
                    svc.series(),
                    authority.origin,
                    base,
                    events,
                    now,
                )
                .await
                .map_err(|error| match error {
                    ReplicaError::ProofConflict => SnapshotEffectError::SnapshotConflict,
                    other => other.into(),
                })?;
                if expiry != stored.proof_expires_at {
                    return Err(SnapshotEffectError::ReceiptInconsistent);
                }
                return Ok(PersistedLocal {
                    first,
                    through: cut,
                    replay: true,
                });
            }
            if prefix == cut {
                return Err(SnapshotEffectError::ReceiptMissing);
            }
            if prefix != base {
                return Err(SnapshotEffectError::CheckpointMismatch);
            }
            let frames: Vec<_> = events.iter().map(|event| event.to_vec()).collect();
            deadline_check(deadline)?;
            let applied =
                Replication::apply_copy_in_tx(guard, svc.series(), authority, &frames, || now)
                    .await?;
            if (
                applied.first,
                applied.last,
                applied.prefix,
                applied.inserted,
            ) != (first, cut, cut, events.len())
            {
                return Err(SnapshotEffectError::CheckpointMismatch);
            }
            let proof_expires_at = snapshot_read::exact_local_expiry(
                guard,
                svc.series(),
                authority.origin,
                base,
                events,
                now,
            )
            .await?;
            let expires_at =
                receipt_expiry(now, current.seconds, window.expires_at, proof_expires_at)?;
            receipt::insert(
                guard.connection(),
                local,
                svc.series(),
                cut,
                &receipt::Receipt {
                    identity: facts.receipt,
                    policy_id: current.id,
                    policy_version: current.version,
                    applied_at: now,
                    proof_expires_at,
                    expires_at,
                    jurisdiction: region,
                },
            )
            .await?;
            Ok(PersistedLocal {
                first,
                through: cut,
                replay: false,
            })
        })
        .await
    }
}

fn ready(state: ReceiverState) -> Result<i64, SnapshotEffectError> {
    match state {
        ReceiverState::Ready { after_seq } => Ok(after_seq),
        ReceiverState::SuspendedHistory { blocked_next_seq } => {
            Err(EffectError::SuspendedHistory { blocked_next_seq }.into())
        }
    }
}

fn package_facts(
    package: &[u8],
    verified: &VerifiedSnapshot<'_>,
) -> Result<intent::Facts, SnapshotEffectError> {
    let manifest = verified.manifest();
    let mut events = [0; 32];
    hex::decode_to_slice(&manifest.events_digest, &mut events)
        .map_err(|_| SnapshotEffectError::IntentInconsistent)?;
    Ok(intent::Facts {
        cut: manifest.cut_seq as i64,
        receipt: receipt::Identity {
            base: manifest.base_seq as i64,
            count: i64::from(manifest.event_count),
            manifest: Sha256::digest(manifest.canonical_bytes()?).into(),
            package: Sha256::digest(package).into(),
        },
        events,
        window: Window {
            not_before: manifest.not_before,
            expires_at: manifest.expires_at,
        },
    })
}

fn receipt_expiry(
    applied_at: i64,
    duration: i64,
    manifest: i64,
    proof: i64,
) -> Result<i64, SnapshotEffectError> {
    applied_at
        .checked_add(duration)
        .map(|ttl| ttl.min(manifest).min(proof))
        .filter(|expiry| *expiry > applied_at)
        .ok_or(SnapshotEffectError::ReceiptInconsistent)
}
