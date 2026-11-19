use godmesh_federation::{
    NodeIdentity,
    restore_evidence::{
        IntentState, Reconciliation, VerifiedHighWater, VerifiedPromotion, proofs_digest,
    },
    snapshot::{SnapshotPolicy, verify_snapshot},
};
use tokio::time::Instant;

use crate::portable_join::ClusterGuard;

use super::{
    super::{
        replication::snapshot_read,
        snapshot_harness::{self, CaptureRange, RestoreApplication, Window},
    },
    Binding, Error, ExternalObservation, ExternalPhase, LocalPhase, LocalRestore,
    OriginObservation, ReplicaService, SessionPeer, check_key, controls, local, window,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupArtifact {
    pub bytes: u64,
    pub sha256: [u8; 32],
}

pub struct PromotionCandidate {
    pub(super) binding: Binding,
    pub(super) reconciliation: Reconciliation,
    pub(super) valid_until: i64,
    pub(super) authorization: Option<[u8; 32]>,
    pub(super) origin: OriginObservation,
}

impl PromotionCandidate {
    pub fn reconciliation(&self) -> &Reconciliation {
        &self.reconciliation
    }
}

/// Origem lê exatamente a lacuna atestada, mesmo depois de avançar seu prefixo.
pub async fn capture_gap(
    svc: &ReplicaService,
    signer: &NodeIdentity,
    peer: SessionPeer<'_>,
    high: &VerifiedHighWater,
    window: Window,
    policy: &SnapshotPolicy,
    deadline: Instant,
) -> Result<Vec<u8>, Error> {
    let mark = high.high_water();
    check_key(svc, &mark.key, peer)?;
    enabled_mark(mark)?;
    let range = CaptureRange::Fixed {
        base: mark.backup_prefix as i64,
        cut: mark.cut_prefix as i64,
        digest: mark.gap_proofs_sha256,
    };
    Ok(snapshot_harness::capture_range(svc, signer, peer, range, window, policy, deadline).await?)
}

impl LocalRestore {
    #[allow(
        clippy::too_many_arguments,
        reason = "fronteira explícita de evidências do harness"
    )]
    pub async fn reconcile(
        &self,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        external: &ExternalObservation,
        origin: &OriginObservation,
        artifact: BackupArtifact,
        gap: Option<(&[u8], &SnapshotPolicy)>,
        deadline: Instant,
    ) -> Result<Reconciliation, Error> {
        let mark = evidence.high_water();
        self.check_mark(evidence, external)?;
        enabled_mark(mark)?;
        if external.origin() != Some(origin) {
            return Err(Error::Conflict);
        }
        if artifact.bytes != mark.backup_bytes || artifact.sha256 != mark.backup_sha256 {
            return Err(Error::Incomplete);
        }
        check_key(&self.service, &mark.key, peer)?;
        budget(deadline)?;
        let verified = if let Some((raw, policy)) = gap {
            snapshot_harness::check_scope(
                &self.service,
                peer,
                policy,
                super::super::Role::Receiver,
            )?;
            let verify = verify_snapshot(raw, policy)
                .map_err(snapshot_harness::SnapshotEffectError::from)?;
            if verify.cut() != (mark.backup_prefix, mark.cut_prefix)
                || proofs_digest(mark.gap_proof_count, verify.events().iter().copied())?
                    != mark.gap_proofs_sha256
            {
                return Err(Error::Incomplete);
            }
            Some((raw, policy, verify))
        } else {
            None
        };
        budget(deadline)?;
        let svc = &self.service;
        let authority = svc.authority(peer, svc.local_limits())?;
        svc.transact_restore(deadline, async |guard, now| {
            let row = local::load(guard.connection(), self.binding()).await?;
            if row.phase == LocalPhase::RetiredClosed {
                return Err(Error::Retired);
            }
            if row.phase == LocalPhase::ReconciledClosed {
                return Ok(self
                    .prove(guard, peer, evidence, origin, now, &row)
                    .await?
                    .reconciliation);
            }
            if row.phase != LocalPhase::RestoreClosed {
                return Err(Error::Conflict);
            }
            let (control, _) = controls::target(
                svc,
                guard,
                &mark.key,
                self.binding().generation,
                peer,
                origin,
                now,
            )
            .await?;
            available(&control)?;
            if control.prefix != mark.backup_prefix {
                return Err(Error::Incomplete);
            }
            self.verify_prefix(guard, authority, evidence, control.prefix, now)
                .await?;
            if mark.cut_prefix > mark.backup_prefix {
                let (raw, policy, package) = verified.as_ref().ok_or(Error::Incomplete)?;
                let (class, region, _) = snapshot_read::scope(guard, svc.series()).await?;
                if class != policy.retention_class {
                    return Err(Error::Scope);
                }
                let authority = svc.authority(
                    peer,
                    snapshot_harness::snapshot_limits(
                        policy,
                        policy.limits.max_package_bytes.get(),
                    )?,
                )?;
                snapshot_harness::apply_closed_in_tx(RestoreApplication {
                    svc,
                    guard,
                    authority,
                    raw,
                    verified: package,
                    region,
                    now,
                    deadline,
                })
                .await?;
            } else if verified.is_some() {
                return Err(Error::Incomplete);
            }
            let (control, _) = controls::target(
                svc,
                guard,
                &mark.key,
                self.binding().generation,
                peer,
                origin,
                now,
            )
            .await?;
            available(&control)?;
            if control.prefix != mark.cut_prefix {
                return Err(Error::Incomplete);
            }
            let proofs_sha256 = self
                .verify_prefix(guard, authority, evidence, control.prefix, now)
                .await?;
            let r = reconciliation(self.binding(), &control, proofs_sha256)?;
            local::record_reconciliation(guard.connection(), self.binding(), &r).await?;
            Ok(r)
        })
        .await
    }

    pub async fn promotion_candidate(
        &self,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        origin: &OriginObservation,
        deadline: Instant,
    ) -> Result<PromotionCandidate, Error> {
        self.service
            .transact_restore(deadline, async |guard, now| {
                let row = local::load(guard.connection(), self.binding()).await?;
                self.prove(guard, peer, evidence, origin, now, &row).await
            })
            .await
    }

    pub async fn opening_candidate(
        &self,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        observed: &ExternalObservation,
        deadline: Instant,
    ) -> Result<PromotionCandidate, Error> {
        self.check_mark(evidence, observed)?;
        let origin = observed.opening_origin().ok_or(Error::Incomplete)?;
        let decision = observed.decision.ok_or(Error::Incomplete)?;
        let candidate = self
            .promotion_candidate(peer, evidence, origin, deadline)
            .await?;
        if candidate.authorization != Some(decision.authorization)
            || candidate.reconciliation.digest()? != decision.reconciliation
        {
            return Err(Error::Conflict);
        }
        Ok(candidate)
    }

    /// Persiste a autorização verificada antes do CAS, permanecendo fechado.
    pub async fn prepare_promotion(
        &self,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        promotion: &VerifiedPromotion,
        origin: &OriginObservation,
        deadline: Instant,
    ) -> Result<PromotionCandidate, Error> {
        let p = promotion.promotion();
        if !self.binding().matches_promotion(p) {
            return Err(Error::Conflict);
        }
        self.service
            .transact_restore(deadline, async |guard, now| {
                if !window(p, now) {
                    return Err(Error::Incomplete);
                }
                let row = local::load(guard.connection(), self.binding()).await?;
                let mut candidate = self.prove(guard, peer, evidence, origin, now, &row).await?;
                if candidate.reconciliation.digest()? != p.reconciliation_sha256 {
                    return Err(Error::Conflict);
                }
                local::prepare_authorization(
                    guard.connection(),
                    self.binding(),
                    promotion.digest(),
                )
                .await?;
                candidate.authorization = Some(promotion.digest());
                Ok(candidate)
            })
            .await
    }

    pub async fn open(
        &self,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        observed: &ExternalObservation,
        promotion: &VerifiedPromotion,
        origin: &OriginObservation,
        deadline: Instant,
    ) -> Result<(), Error> {
        self.check_mark(evidence, observed)?;
        let p = promotion.promotion();
        if observed.opening_origin() != Some(origin) {
            return Err(Error::Conflict);
        }
        let decision = observed.decision.ok_or(Error::Incomplete)?;
        if observed.phase() != ExternalPhase::PromotionConsumed
            || !self.binding().matches_promotion(p)
            || decision.authorization != promotion.digest()
            || decision.reconciliation != p.reconciliation_sha256
        {
            return Err(Error::Conflict);
        }
        self.service
            .transact_restore(deadline, async |guard, now| {
                if !window(p, now) {
                    return Err(Error::Incomplete);
                }
                let row = local::load(guard.connection(), self.binding()).await?;
                if row.authorization != Some(decision.authorization) {
                    return Err(Error::Conflict);
                }
                let result = self.prove(guard, peer, evidence, origin, now, &row).await?;
                if result.reconciliation.digest()? != decision.reconciliation {
                    return Err(Error::Conflict);
                }
                local::record_open(guard.connection(), self.binding(), decision.authorization).await
            })
            .await?;
        self.gate.opened();
        Ok(())
    }

    fn check_mark(
        &self,
        evidence: &VerifiedHighWater,
        external: &ExternalObservation,
    ) -> Result<(), Error> {
        if Binding::from_high_water(evidence)? != *self.binding()
            || external.binding() != self.binding()
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    async fn prove(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        peer: SessionPeer<'_>,
        evidence: &VerifiedHighWater,
        origin: &OriginObservation,
        now: i64,
        row: &local::LocalRow,
    ) -> Result<PromotionCandidate, Error> {
        let mark = evidence.high_water();
        enabled_mark(mark)?;
        if row.phase == LocalPhase::RetiredClosed {
            return Err(Error::Retired);
        }
        if row.phase != LocalPhase::ReconciledClosed
            || Binding::from_high_water(evidence)? != *self.binding()
        {
            return Err(Error::Incomplete);
        }
        let svc = &self.service;
        check_key(svc, &mark.key, peer)?;
        let authority = svc.authority(peer, svc.local_limits())?;
        let (control, valid_until) = controls::target(
            svc,
            guard,
            &mark.key,
            self.binding().generation,
            peer,
            origin,
            now,
        )
        .await?;
        available(&control)?;
        if control.prefix != mark.cut_prefix {
            return Err(Error::Incomplete);
        }
        let digest = self
            .verify_prefix(guard, authority, evidence, control.prefix, now)
            .await?;
        let r = reconciliation(self.binding(), &control, digest)?;
        if row.reconciliation != Some(r.digest()?)
            || row.final_prefix != Some(r.final_prefix as i64)
            || row.proofs_sha256 != Some(r.proofs_sha256)
            || row.current_control_sha256 != Some(r.current_control_sha256)
        {
            return Err(Error::Conflict);
        }
        Ok(PromotionCandidate {
            binding: self.binding().clone(),
            reconciliation: r,
            valid_until,
            authorization: row.authorization,
            origin: origin.clone(),
        })
    }

    async fn verify_prefix(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        authority: crate::replication::CopyAuthority<'_>,
        evidence: &VerifiedHighWater,
        prefix: u64,
        now: i64,
    ) -> Result<[u8; 32], Error> {
        let mark = evidence.high_water();
        let frames = controls::prefix_frames(&self.service, guard, authority, prefix, now).await?;
        let i = usize::try_from(mark.backup_prefix).map_err(|_| Error::Incomplete)?;
        let (backup, gap) = frames.split_at_checked(i).ok_or(Error::Incomplete)?;
        if proofs_digest(mark.backup_proof_count, backup.iter().map(Vec::as_slice))?
            != mark.backup_proofs_sha256
            || (prefix == mark.cut_prefix
                && proofs_digest(mark.gap_proof_count, gap.iter().map(Vec::as_slice))?
                    != mark.gap_proofs_sha256)
        {
            return Err(Error::Incomplete);
        }
        Ok(proofs_digest(prefix, frames.iter().map(Vec::as_slice))?)
    }
}

fn enabled_mark(
    mark: &godmesh_federation::restore_evidence::RestoreHighWaterV1,
) -> Result<(), Error> {
    if mark.blocked_next_seq != 0
        || mark.intent_state != IntentState::Absent
        || mark.attempt_id != 0
    {
        return Err(Error::Incomplete);
    }
    Ok(())
}

fn available(control: &godmesh_federation::restore_evidence::ControlState) -> Result<(), Error> {
    if control.blocked_next_seq != 0
        || control.intent_state != IntentState::Absent
        || control.attempt_id != 0
    {
        return Err(Error::Incomplete);
    }
    Ok(())
}

fn reconciliation(
    b: &Binding,
    control: &godmesh_federation::restore_evidence::ControlState,
    proofs: [u8; 32],
) -> Result<Reconciliation, Error> {
    Ok(Reconciliation {
        key: b.key.clone(),
        target_generation: b.generation,
        high_water_sha256: b.high_water_sha256,
        final_prefix: control.prefix,
        proofs_sha256: proofs,
        current_control_sha256: control.digest()?,
        blocked_next_seq: control.blocked_next_seq,
        intent_state: control.intent_state,
        attempt_id: control.attempt_id,
    })
}

fn budget(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}
