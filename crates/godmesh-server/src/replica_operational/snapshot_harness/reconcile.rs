use std::{num::NonZeroU64, sync::Arc};

use sqlx::PgConnection;

use crate::{replication::Series, sql_budget};

use super::super::intent_marker;
use super::{
    AttemptId, Digest, EligibleReceiver, Instant, ReplicaService, Role, SessionPeer, Sha256,
    SnapshotEffectError, SnapshotManifest, SnapshotPolicy, Window, check_scope, deadline_check,
    intent, package_facts, ready, receipt, receipt_expiry, snapshot, snapshot_limits,
    snapshot_read,
};

/// Evidência externa de um banco criado e acompanhado pelo harness.
/// O entrypoint de teste vincula a geração e os pins à instância antes do boot.
pub struct HarnessHistory {
    pub database: String,
    pub generation: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    Idle { after_seq: i64 },
    Applied { after_seq: i64 },
    NoEffect { after_seq: i64 },
    Blocked { attempt: AttemptId },
}

impl EligibleReceiver {
    pub async fn new(
        service: Arc<ReplicaService>,
        history: HarnessHistory,
        deadline: Instant,
    ) -> Result<Self, SnapshotEffectError> {
        if service.config().role != Role::Receiver || history.database.trim().is_empty() {
            return Err(SnapshotEffectError::Eligibility);
        }
        service
            .transact_with(deadline, async |guard, _| {
                sql_budget::refresh(guard.connection()).await?;
                let actual: String = sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(guard.connection())
                    .await?;
                if actual != history.database {
                    return Err(SnapshotEffectError::Eligibility);
                }
                let _ = snapshot_read::scope(guard, service.series()).await?;
                Ok(())
            })
            .await?;
        Ok(Self(service, history.generation))
    }

    pub fn generation(&self) -> NonZeroU64 {
        self.1
    }

    pub async fn checkpoint(
        &self,
        peer: SessionPeer<'_>,
        deadline: Instant,
    ) -> Result<i64, SnapshotEffectError> {
        ready(self.0.checkpoint(peer, deadline).await?)
    }

    pub async fn intend(
        &self,
        peer: SessionPeer<'_>,
        package: &[u8],
        policy: &SnapshotPolicy,
        deadline: Instant,
    ) -> Result<AttemptId, SnapshotEffectError> {
        let svc = &self.0;
        check_scope(svc, peer, policy, Role::Receiver)?;
        deadline_check(deadline)?;
        let verified = snapshot::verify_snapshot(package, policy)?;
        let facts = package_facts(package, &verified)?;
        deadline_check(deadline)?;
        let authority = svc.authority(
            peer,
            snapshot_limits(policy, policy.limits.max_package_bytes.get())?,
        )?;
        let local = &svc.local_pin().node_id;
        svc.transact_with(deadline, async |guard, now| {
            svc.gates(guard, peer, now).await?;
            let prefix = ready(svc.receiver_state_in_tx(guard, authority, now).await?)?;
            let (class, region, _) = snapshot_read::scope(guard, svc.series()).await?;
            if class != policy.retention_class {
                return Err(SnapshotEffectError::Scope);
            }
            if intent_marker::pending(guard.connection(), local, svc.series())
                .await?
                .is_some()
            {
                return Err(SnapshotEffectError::IntentPending);
            }
            if prefix != facts.receipt.base {
                return Err(SnapshotEffectError::CheckpointMismatch);
            }
            facts.window.check(now)?;
            receipt::policy(guard.connection(), local, svc.series(), &region, None).await?;
            let current =
                intent::policy(guard.connection(), local, svc.series(), &region, None).await?;
            let detail = intent::Detail {
                expires_at: intent::expiry(now, current.seconds, facts.window)?,
                facts,
                policy_id: current.id,
                policy_version: current.version,
                jurisdiction: region,
                created_at: now,
            };
            let token = intent::new_marker(guard.connection(), local, svc.series()).await?;
            intent::insert(guard.connection(), local, svc.series(), token, &detail).await?;
            Ok(token)
        })
        .await
    }

    /// Resolve somente por evidência durável sob os mesmos locks da escrita.
    /// Nenhuma destas saídas autoriza um ACK numa stream substituta.
    pub async fn recover(
        &self,
        peer: SessionPeer<'_>,
        policy: &SnapshotPolicy,
        deadline: Instant,
    ) -> Result<Recovery, SnapshotEffectError> {
        let svc = &self.0;
        check_scope(svc, peer, policy, Role::Receiver)?;
        let authority = svc.authority(
            peer,
            snapshot_limits(policy, policy.limits.max_package_bytes.get())?,
        )?;
        let local = &svc.local_pin().node_id;
        svc.transact_with(deadline, async |guard, now| {
            svc.gates(guard, peer, now).await?;
            let prefix = ready(svc.receiver_state_in_tx(guard, authority, now).await?)?;
            let Some(raw_token) =
                intent_marker::pending(guard.connection(), local, svc.series()).await?
            else {
                return Ok(Recovery::Idle { after_seq: prefix });
            };
            let token = AttemptId::from_db(raw_token)?;
            let blocked = Recovery::Blocked { attempt: token };
            let Some(detail) = intent::load(guard.connection(), local, svc.series(), token).await?
            else {
                return Ok(blocked);
            };
            let (class, region, _) = snapshot_read::scope(guard, svc.series()).await?;
            if class != policy.retention_class {
                return Err(SnapshotEffectError::Scope);
            }
            check_detail(
                guard.connection(),
                local,
                svc.series(),
                &region,
                &detail,
                now,
            )
            .await?;
            if !matches_manifest(&detail.facts, policy)? {
                return Ok(blocked);
            }
            let facts = &detail.facts;
            let saved = receipt::load(guard.connection(), local, svc.series(), facts.cut).await?;
            let resolution = match saved {
                Some(receipt) if prefix == facts.cut && receipt.identity == facts.receipt => {
                    if facts.window.check(now).is_err() {
                        return Ok(blocked);
                    }
                    receipt::policy(guard.connection(), local, svc.series(), &region, None).await?;
                    match check_receipt(
                        guard.connection(),
                        local,
                        svc.series(),
                        &region,
                        &receipt,
                        facts.window,
                        now,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(
                            SnapshotEffectError::ReceiptExpired
                            | SnapshotEffectError::ReceiptInconsistent,
                        ) => return Ok(blocked),
                        Err(error) => return Err(error),
                    }
                    let (frames, expiry) = match snapshot_read::local_snapshot_proofs(
                        guard,
                        svc.series(),
                        authority,
                        facts.receipt.base,
                        facts.cut,
                        now,
                    )
                    .await
                    {
                        Ok(proofs) => proofs,
                        Err(error) if !matches!(error, crate::replication::Error::Storage(_)) => {
                            return Ok(blocked);
                        }
                        Err(error) => return Err(error.into()),
                    };
                    let events: Vec<_> = frames.iter().map(Vec::as_slice).collect();
                    if expiry != receipt.proof_expires_at
                        || snapshot::events_digest(&events)? != facts.events
                    {
                        return Ok(blocked);
                    }
                    Recovery::Applied { after_seq: prefix }
                }
                None if prefix == facts.receipt.base => {
                    if !snapshot_read::range_unused(
                        guard,
                        svc.series(),
                        facts.receipt.base,
                        facts.cut,
                    )
                    .await?
                    {
                        return Ok(blocked);
                    }
                    Recovery::NoEffect { after_seq: prefix }
                }
                _ => return Ok(blocked),
            };
            intent::resolve(guard.connection(), local, svc.series(), token).await?;
            Ok(resolution)
        })
        .await
    }

    /// Redução de detalhe vencido. O marcador e o token sobrevivem ao commit.
    pub async fn reduce_expired(
        &self,
        peer: SessionPeer<'_>,
        token: AttemptId,
        deadline: Instant,
    ) -> Result<(), SnapshotEffectError> {
        let svc = &self.0;
        let authority = svc.authority(peer, svc.local_limits())?;
        let local = &svc.local_pin().node_id;
        svc.transact_with(deadline, async |guard, now| {
            svc.gates(guard, peer, now).await?;
            ready(svc.receiver_state_in_tx(guard, authority, now).await?)?;
            if intent_marker::pending(guard.connection(), local, svc.series()).await?
                != Some(token.get())
            {
                return Err(SnapshotEffectError::AttemptMismatch);
            }
            let detail = intent::load(guard.connection(), local, svc.series(), token)
                .await?
                .ok_or(SnapshotEffectError::IntentInconsistent)?;
            let (_, region, _) = snapshot_read::scope(guard, svc.series()).await?;
            let historical = intent::policy(
                guard.connection(),
                local,
                svc.series(),
                &region,
                Some(&detail),
            )
            .await?;
            detail.consistent(&historical, &region, now)?;
            if !historical.allow_reduce || now < detail.expires_at {
                return Err(SnapshotEffectError::IntentPolicy);
            }
            intent::reduce(guard.connection(), local, svc.series(), token).await
        })
        .await
    }
}

pub(super) async fn check_detail(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    region: &str,
    detail: &intent::Detail,
    now: i64,
) -> Result<(), SnapshotEffectError> {
    if detail.jurisdiction != region {
        return Err(SnapshotEffectError::IntentInconsistent);
    }
    intent::policy(db, local, series, region, None).await?;
    let historical = intent::policy(db, local, series, region, Some(detail)).await?;
    detail.consistent(&historical, region, now)
}

pub(super) async fn check_receipt(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    region: &str,
    stored: &receipt::Receipt,
    window: Window,
    now: i64,
) -> Result<(), SnapshotEffectError> {
    if stored.jurisdiction != region {
        return Err(SnapshotEffectError::ReceiptInconsistent);
    }
    let historical = receipt::policy(db, local, series, region, Some(stored)).await?;
    let expiry = receipt_expiry(
        stored.applied_at,
        historical.seconds,
        window.expires_at,
        stored.proof_expires_at,
    )?;
    if expiry != stored.expires_at
        || stored.applied_at < window.not_before
        || stored.applied_at > now
    {
        return Err(SnapshotEffectError::ReceiptInconsistent);
    }
    if now >= expiry {
        return Err(SnapshotEffectError::ReceiptExpired);
    }
    Ok(())
}

fn matches_manifest(
    facts: &intent::Facts,
    policy: &SnapshotPolicy,
) -> Result<bool, SnapshotEffectError> {
    let count =
        u32::try_from(facts.receipt.count).map_err(|_| SnapshotEffectError::IntentInconsistent)?;
    let manifest = SnapshotManifest {
        profile: 1,
        cluster: policy.cluster.clone(),
        origin_node: policy.origin.node_id.clone(),
        destination_node: policy.destination.node_id.clone(),
        tenant_id: policy.tenant_id.clone(),
        stream_id: policy.stream_id.clone(),
        partition: policy.partition,
        authority_epoch: policy.authority_epoch,
        base_seq: facts.receipt.base as u64,
        cut_seq: facts.cut as u64,
        event_count: count,
        not_before: facts.window.not_before,
        expires_at: facts.window.expires_at,
        events_digest: hex::encode(facts.events),
        signature: String::new(),
    };
    Ok(<[u8; 32]>::from(Sha256::digest(manifest.canonical_bytes()?)) == facts.receipt.manifest)
}
