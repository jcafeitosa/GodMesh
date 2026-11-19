use godmesh_federation::restore_evidence::{
    ControlState, CopyAcl, CopyPolicy, IntentState, RestoreKey, proofs_digest,
};
use sqlx::{PgConnection, Row};
use tokio::time::Instant;

use crate::{
    config_bundle::Fence,
    portable_join::ClusterGuard,
    replication::{CopyAuthority, Replication, snapshot_read},
};

use super::{
    super::{EffectError, ReceiverState, Role, intent_marker},
    Error, ReplicaService, SessionPeer, check_key, sql_budget,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3Tuple {
    pub epoch: u64,
    pub version: u64,
    pub hash: [u8; 32],
    pub floor: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginObservation(pub(super) OriginWire);

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginWire {
    pub(super) key: RestoreKey,
    pub(super) m3: M3Tuple,
    pub(super) valid_until: i64,
    pub(super) sampled_at: i64,
    revocations: Vec<[u8; 32]>,
    primary: [u8; 32],
    secondary: [u8; 32],
    acl: [u8; 32],
}

pub struct CutObservation {
    pub control: ControlState,
    pub backup_proofs_sha256: [u8; 32],
    pub gap_proofs_sha256: [u8; 32],
    pub cut_at: i64,
}

impl OriginObservation {
    pub fn m3(&self) -> M3Tuple {
        self.0.m3
    }

    pub(super) fn encode(&self) -> Result<String, Error> {
        serde_json::to_string(&self.0).map_err(|_| Error::Incomplete)
    }

    pub(super) fn decode(raw: &str) -> Result<Self, Error> {
        Ok(Self(
            serde_json::from_str(raw).map_err(|_| Error::Incomplete)?,
        ))
    }
}

async fn gates(
    svc: &ReplicaService,
    guard: &mut ClusterGuard<'_, '_>,
    peer: SessionPeer<'_>,
    now: i64,
) -> Result<(M3Tuple, i64), Error> {
    let local = svc
        .portable
        .authorize_local_at(guard, now)
        .await
        .map_err(EffectError::from)?;
    let peer_expiry = svc
        .portable
        .authorize_peer_at(guard, peer.node_id, peer.public_key, now)
        .await
        .map_err(EffectError::from)?;
    let Fence::Allowed {
        version,
        hash,
        floor,
        recheck_at,
        expires_at,
    } = svc
        .m3
        .classify_fence_in_tx(guard, peer.protocol, now)
        .await
        .map_err(EffectError::from)?
    else {
        return Err(Error::Incomplete);
    };
    let valid = local
        .into_iter()
        .chain(peer_expiry)
        .chain([expires_at, recheck_at])
        .min()
        .ok_or(Error::Incomplete)?;
    if now >= valid {
        return Err(Error::Incomplete);
    }
    Ok((
        M3Tuple {
            epoch: svc.m3.policy().bundle.epoch,
            version,
            hash,
            floor,
        },
        valid,
    ))
}

async fn revocations(
    svc: &ReplicaService,
    guard: &mut ClusterGuard<'_, '_>,
) -> Result<Vec<[u8; 32]>, Error> {
    svc.portable
        .restore_revocations(guard)
        .await
        .map_err(EffectError::from)?
        .iter()
        .map(|row| Ok(row.digest()?))
        .collect()
}

pub async fn observe_origin(
    svc: &ReplicaService,
    key: &RestoreKey,
    peer: SessionPeer<'_>,
    deadline: Instant,
) -> Result<OriginObservation, Error> {
    svc.require_role(Role::Origin)?;
    check_key(svc, key, peer)?;
    let authority = svc.authority(peer, svc.local_limits())?;
    svc.transact_with(deadline,async |guard,now| {
        let (m3,valid)=gates(svc,guard,peer,now).await?;
        Replication::recheck_copy_in_tx(guard,svc.series(),authority,true,now).await?;
        let revocations=revocations(svc,guard).await?;
        let db=guard.connection();
        let (acl,secondary)=copy_side(db,key,"export").await?;
        sql_budget::refresh(db).await?;
        let primary = sqlx::query(
            "SELECT p.* FROM federation_replica_namespace n JOIN federation_replica_policy p USING(policy_id,policy_version)
             WHERE (n.origin_node,n.tenant_id,n.stream_id,n.partition,n.authority_epoch,n.local_node)=($1,$2,$3,$4,$5,$1)
             FOR SHARE OF p",
        ).bind(&key.origin_node).bind(&key.tenant_id).bind(&key.stream_id)
            .bind(key.partition as i32).bind(key.authority_epoch as i64).fetch_optional(db).await?
            .ok_or(Error::Incomplete)?;
        Ok(OriginObservation(OriginWire {key:key.clone(),m3,valid_until:valid.min(acl.expires_at),sampled_at:now,revocations,
            primary:policy(&primary)?.digest()?,secondary:secondary.digest()?,acl:acl.digest()?}))
    }).await
}

pub(super) async fn target(
    svc: &ReplicaService,
    guard: &mut ClusterGuard<'_, '_>,
    key: &RestoreKey,
    generation: u64,
    peer: SessionPeer<'_>,
    origin: &OriginObservation,
    now: i64,
) -> Result<(ControlState, i64), Error> {
    if origin.0.key != *key || now < origin.0.sampled_at || now >= origin.0.valid_until {
        return Err(Error::Incomplete);
    }
    let (m3, valid) = gates(svc, guard, peer, now).await?;
    if m3 != origin.0.m3 {
        return Err(Error::Incomplete);
    }
    let authority = svc.authority(peer, svc.local_limits())?;
    let state = svc.receiver_state_in_tx(guard, authority, now).await?;
    let (prefix, blocked_next_seq) = match state {
        ReceiverState::Ready { after_seq } => (after_seq, 0),
        ReceiverState::SuspendedHistory { blocked_next_seq } => (
            blocked_next_seq.checked_sub(1).ok_or(Error::Incomplete)?,
            blocked_next_seq,
        ),
    };
    let (root_node_id, authority_kind, authority_ref) = svc
        .portable
        .restore_local_authority(guard, now)
        .await
        .map_err(EffectError::from)?;
    let destination_revocations = revocations(svc, guard).await?;
    let (intent_state, attempt_id) =
        match intent_marker::pending(guard.connection(), &key.local_node, svc.series()).await? {
            Some(attempt) => (IntentState::Pending, attempt as u64),
            None => (IntentState::Absent, 0),
        };
    let (acl, secondary) = copy_side(guard.connection(), key, "import").await?;
    let cutoff = valid.min(acl.expires_at).min(origin.0.valid_until);
    Ok((
        ControlState {
            key: key.clone(),
            generation,
            root_node_id,
            authority_kind,
            authority_ref,
            origin_revocations: origin.0.revocations.clone(),
            destination_revocations,
            m3_epoch: m3.epoch,
            m3_version: m3.version,
            m3_hash: m3.hash,
            m3_floor: m3.floor,
            primary_policy_sha256: origin.0.primary,
            export_secondary_policy_sha256: origin.0.secondary,
            import_secondary_policy_sha256: secondary.digest()?,
            export_acl_sha256: origin.0.acl,
            import_acl_sha256: acl.digest()?,
            blocked_next_seq: blocked_next_seq as u64,
            intent_state,
            attempt_id,
            prefix: prefix as u64,
        },
        cutoff,
    ))
}

/// Captura no banco de corte pelo operador, após cerca e drenagem externas.
/// A observação de A foi obtida separadamente, sem transação distribuída.
pub async fn observe_cut(
    svc: &ReplicaService,
    key: &RestoreKey,
    generation: u64,
    peer: SessionPeer<'_>,
    origin: &OriginObservation,
    backup_prefix: u64,
    deadline: Instant,
) -> Result<CutObservation, Error> {
    svc.require_role(Role::Receiver)?;
    check_key(svc, key, peer)?;
    let authority = svc.authority(peer, svc.local_limits())?;
    svc.transact_restore(deadline, async |guard, now| {
        let (control, _) = target(svc, guard, key, generation, peer, origin, now).await?;
        if backup_prefix > control.prefix {
            return Err(Error::Incomplete);
        }
        let frames = prefix_frames(svc, guard, authority, control.prefix, now).await?;
        let (backup, gap) =
            frames.split_at(usize::try_from(backup_prefix).map_err(|_| Error::Incomplete)?);
        Ok(CutObservation {
            backup_proofs_sha256: proofs_digest(backup_prefix, backup.iter().map(Vec::as_slice))?,
            gap_proofs_sha256: proofs_digest(
                control.prefix - backup_prefix,
                gap.iter().map(Vec::as_slice),
            )?,
            control,
            cut_at: now,
        })
    })
    .await
}

pub(super) async fn prefix_frames(
    svc: &ReplicaService,
    guard: &mut ClusterGuard<'_, '_>,
    authority: CopyAuthority<'_>,
    prefix: u64,
    now: i64,
) -> Result<Vec<Vec<u8>>, Error> {
    if prefix == 0 {
        return Ok(Vec::new());
    }
    let cut = i64::try_from(prefix).map_err(|_| Error::Incomplete)?;
    Ok(
        snapshot_read::local_snapshot_proofs(guard, svc.series(), authority, 0, cut, now)
            .await?
            .0,
    )
}

async fn copy_side(
    db: &mut PgConnection,
    k: &RestoreKey,
    direction: &str,
) -> Result<(CopyAcl, CopyPolicy), Error> {
    sql_budget::refresh(db).await?;
    let acl=sqlx::query(
        "SELECT * FROM federation_replica_acl
         WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch,destination,direction)=($1,$2,$3,$4,$5,$6,$7)
         FOR SHARE",
    ).bind(&k.origin_node).bind(&k.tenant_id).bind(&k.stream_id).bind(k.partition as i32)
     .bind(k.authority_epoch as i64).bind(&k.local_node).bind(direction)
     .fetch_optional(&mut *db).await?.ok_or(Error::Incomplete)?;
    let rule = CopyAcl {
        origin_node: acl.try_get("origin_node")?,
        tenant_id: acl.try_get("tenant_id")?,
        stream_id: acl.try_get("stream_id")?,
        partition: k.partition,
        authority_epoch: k.authority_epoch,
        destination: acl.try_get("destination")?,
        direction: acl.try_get("direction")?,
        retention_class: acl.try_get("retention_class")?,
        class_version: number(&acl, "class_version")?,
        jurisdiction: acl.try_get("jurisdiction")?,
        copy_kind: acl.try_get("copy_kind")?,
        policy_id: acl.try_get("policy_id")?,
        policy_version: number(&acl, "policy_version")?,
        not_before: acl.try_get("not_before")?,
        expires_at: acl.try_get("expires_at")?,
        revoked: acl.try_get("revoked")?,
    };
    sql_budget::refresh(db).await?;
    let p=sqlx::query("SELECT * FROM federation_replica_policy WHERE policy_id=$1 AND policy_version=$2 FOR SHARE")
        .bind(&rule.policy_id).bind(rule.policy_version as i64).fetch_optional(db).await?.ok_or(Error::Incomplete)?;
    Ok((rule, policy(&p)?))
}

fn number(row: &sqlx::postgres::PgRow, field: &str) -> Result<u64, Error> {
    u64::try_from(row.try_get::<i64, _>(field)?).map_err(|_| Error::Incomplete)
}

fn policy(p: &sqlx::postgres::PgRow) -> Result<CopyPolicy, Error> {
    Ok(CopyPolicy {
        policy_id: p.try_get("policy_id")?,
        policy_version: number(p, "policy_version")?,
        tenant_id: p.try_get("tenant_id")?,
        retention_class: p.try_get("retention_class")?,
        class_version: number(p, "class_version")?,
        jurisdiction: p.try_get("jurisdiction")?,
        copy_node: p.try_get("copy_node")?,
        copy_kind: p.try_get("copy_kind")?,
        purpose: p.try_get("purpose")?,
        trigger_event: p.try_get("trigger_event")?,
        duration_seconds: number(p, "duration_seconds")?,
        allow_discard: p.try_get("allow_discard")?,
        hold_mode: p.try_get("hold_mode")?,
        active: p.try_get("active")?,
    })
}
