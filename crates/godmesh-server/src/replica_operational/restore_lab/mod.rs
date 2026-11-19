//! Estado de restore exclusivo do laboratório. A montagem administrativa e o
//! código de boot são responsáveis pela fonte e pelo fence físicos da fixture.
mod controls;
mod local;
mod operations;
mod store;

pub use controls::{CutObservation, M3Tuple, OriginObservation, observe_cut, observe_origin};
pub use local::{AdmissionGate, LocalPhase, LocalRestore, allocation_id};
pub use operations::{BackupArtifact, PromotionCandidate, capture_gap};
pub use store::{
    ExternalObservation, ExternalPhase, ExternalStore, GenerationLogins, OpeningOrigin,
    observe_opening_origin,
};

use godmesh_federation::restore_evidence::{
    RestoreError, RestoreKey, RestorePromotionV1, VerifiedHighWater,
};

use super::{
    EffectError, ReplicaService, SessionPeer, db, replication, sql_budget, with_sql_deadline,
};

pub const EXTERNAL_DDL: &str = include_str!("external.sql");
pub const LOCAL_DDL: &str = include_str!("local.sql");

#[derive(Debug)]
pub enum Error {
    Incomplete,
    Scope,
    Conflict,
    Retired,
    Timeout,
    Evidence(RestoreError),
    Snapshot(super::snapshot_harness::SnapshotEffectError),
    Effect(EffectError),
}

impl From<EffectError> for Error {
    fn from(error: EffectError) -> Self {
        match error {
            EffectError::Timeout => Self::Timeout,
            other => Self::Effect(other),
        }
    }
}
impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        EffectError::from(error).into()
    }
}
impl From<replication::Error> for Error {
    fn from(error: replication::Error) -> Self {
        EffectError::from(error).into()
    }
}
impl From<RestoreError> for Error {
    fn from(error: RestoreError) -> Self {
        Self::Evidence(error)
    }
}
impl From<super::snapshot_harness::SnapshotEffectError> for Error {
    fn from(error: super::snapshot_harness::SnapshotEffectError) -> Self {
        Self::Snapshot(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Effect(EffectError::CommitUnknown(_)) => f.write_str("restore_commit_unknown"),
            _ => write!(f, "restore_history_incomplete: {self:?}"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key: RestoreKey,
    pub generation: u64,
    pub target_db_id: String,
    pub high_water_sha256: [u8; 32],
    pub fence_sha256: [u8; 32],
}

impl Binding {
    pub fn from_high_water(mark: &VerifiedHighWater) -> Result<Self, Error> {
        let body = mark.high_water();
        let value = Self {
            key: body.key.clone(),
            generation: body.target_generation,
            target_db_id: body.target_db_id.clone(),
            high_water_sha256: mark.digest(),
            fence_sha256: body.fence_sha256,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), Error> {
        let max = i64::MAX as u64;
        let text = |s: &str| {
            !s.trim().is_empty() && s.len() <= godmesh_federation::manifest::MAX_FIELD_LEN
        };
        let k = &self.key;
        if k.fixture_id.len() != 32
            || !k
                .fixture_id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !text(&self.target_db_id)
            || !text(&k.cluster)
            || !text(&k.tenant_id)
            || !text(&k.stream_id)
            || !godmesh_federation::identity::is_node_id(&k.local_node)
            || !godmesh_federation::identity::is_node_id(&k.origin_node)
            || k.local_node == k.origin_node
            || k.partition != 0
            || !(1..=max).contains(&k.authority_epoch)
            || !(1..=max).contains(&self.generation)
        {
            return Err(Error::Scope);
        }
        Ok(())
    }

    fn matches_promotion(&self, body: &RestorePromotionV1) -> bool {
        self.key == body.key
            && self.generation == body.target_generation
            && body.fenced_generation.checked_add(1) == Some(self.generation)
            && self.target_db_id == body.target_db_id
            && self.high_water_sha256 == body.high_water_sha256
            && self.fence_sha256 == body.fence_sha256
    }
}

fn window(p: &RestorePromotionV1, now: i64) -> bool {
    p.not_before <= now && now < p.expires_at
}

fn check_key(svc: &ReplicaService, key: &RestoreKey, peer: SessionPeer<'_>) -> Result<(), Error> {
    let cfg = svc.config();
    let ns = &cfg.namespace;
    let ids = match cfg.role {
        super::Role::Origin => (peer.node_id, svc.local_pin().node_id.as_str()),
        super::Role::Receiver => (svc.local_pin().node_id.as_str(), peer.node_id),
    };
    if key.cluster != cfg.cluster
        || key.local_node != ids.0
        || key.origin_node != ids.1
        || key.origin_node != ns.origin_node
        || key.tenant_id != ns.tenant_id
        || key.stream_id != ns.stream_id
        || i32::try_from(key.partition) != Ok(ns.partition)
        || i64::try_from(key.authority_epoch) != Ok(cfg.authority_epoch)
    {
        return Err(Error::Scope);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
