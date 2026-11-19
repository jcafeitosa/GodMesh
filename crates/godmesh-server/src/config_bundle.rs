//! Estado durável M3-B1. Operações `_in_tx` preparam efeitos na transação
//! READ COMMITTED que já detém o lock M1C. O chamador é dono do commit.

use godmesh_federation::config_bundle::{
    AuthenticatedBundle, BundleError, BundlePolicy, BundleStatus, NAMESPACE, PROFILE,
    SoftwareVersion, authenticate_bundle,
};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};

use crate::portable_join::{ClusterGuard, PortableJoin, PortableJoinError};

type Tx<'a> = Transaction<'a, Postgres>;
type Hash = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Publisher,
    Receiver,
}

impl Role {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Publisher => "publisher",
            Self::Receiver => "receiver",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServicePolicy {
    pub profile: u64,
    pub bundle: BundlePolicy,
    pub role: Role,
    pub software: SoftwareVersion,
}

#[derive(Debug)]
pub enum Preflight {
    Disabled,
    Enabled(BundleService),
}

impl Preflight {
    /// A ausência de política só é um estado válido após o preflight.
    pub async fn classify_fence_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        protocol: u64,
        now: i64,
    ) -> Result<Fence, WriteError> {
        match self {
            Self::Disabled => Ok(Fence::Disabled),
            Self::Enabled(service) => service.classify_fence_in_tx(guard, protocol, now).await,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Accepted,
    Replay,
    ConflictRecorded,
    ConflictKnown,
    IncompatibleRecorded,
    IncompatibleKnown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Policy,
    WrongRole,
    LockRequired,
    LocalAuthority,
    Codec(BundleError),
    Rollback,
    Expired,
    ConflictOpen,
    NoOpenConflict,
    Incompatible,
    InvalidActor,
}

#[derive(Debug)]
pub enum Failure {
    Storage(sqlx::Error),
    CorruptState,
    Authority(PortableJoinError),
}

#[derive(Debug)]
pub enum WriteError {
    Rejected(Rejection),
    Transient(Failure),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WriteError {}

impl From<Rejection> for WriteError {
    fn from(reason: Rejection) -> Self {
        Self::Rejected(reason)
    }
}

impl From<sqlx::Error> for WriteError {
    fn from(error: sqlx::Error) -> Self {
        Self::Transient(Failure::Storage(error))
    }
}

impl From<PortableJoinError> for WriteError {
    fn from(error: PortableJoinError) -> Self {
        match error {
            PortableJoinError::Storage(error) => error.into(),
            PortableJoinError::Configuration(_) => Rejection::LockRequired.into(),
            PortableJoinError::Encoding(_) => Self::Transient(Failure::Authority(error)),
            _ => Rejection::LocalAuthority.into(),
        }
    }
}

fn corrupt() -> WriteError {
    WriteError::Transient(Failure::CorruptState)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Published {
    Frame {
        frame: Vec<u8>,
        version: u64,
        hash: Hash,
    },
    NotModified {
        version: u64,
        hash: Hash,
    },
    Unavailable,
    BlockedConflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Conflict,
    Recovering,
    Incompatible,
    PendingDue,
    NoEffective,
    Expired,
    BelowFloor,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Activation {
    Activated,
    NoChange,
    Blocked(Block),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Fence {
    Disabled,
    Allowed {
        version: u64,
        hash: Hash,
        floor: u64,
        expires_at: i64,
    },
    Blocked(Block),
}

#[derive(Debug)]
pub struct BundleService {
    pool: PgPool,
    policy: ServicePolicy,
    voters: Vec<String>,
    signers: Vec<String>,
}

type StoredPolicy = (i64, String, String, String, i64, String, Vec<String>);

#[derive(Default)]
struct State {
    accepted: Option<String>,
    incompatible: Option<String>,
    pending: Option<String>,
    effective: Option<String>,
    published: Option<String>,
    conflict: Option<String>,
    recovering: bool,
}

impl State {
    async fn load(tx: &mut Tx<'_>) -> Result<Self, WriteError> {
        let row = sqlx::query(
            "SELECT accepted, incompatible, pending, effective, published, conflict, recovering
               FROM federation_config_state WHERE singleton",
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(corrupt)?;
        Ok(Self {
            accepted: row.try_get("accepted")?,
            incompatible: row.try_get("incompatible")?,
            pending: row.try_get("pending")?,
            effective: row.try_get("effective")?,
            published: row.try_get("published")?,
            conflict: row.try_get("conflict")?,
            recovering: row.try_get("recovering")?,
        })
    }

    async fn save(&self, tx: &mut Tx<'_>) -> Result<(), WriteError> {
        let affected = sqlx::query(
            "UPDATE federation_config_state SET accepted = $1, incompatible = $2, pending = $3,
               effective = $4, published = $5, conflict = $6, recovering = $7 WHERE singleton",
        )
        .bind(&self.accepted)
        .bind(&self.incompatible)
        .bind(&self.pending)
        .bind(&self.effective)
        .bind(&self.published)
        .bind(&self.conflict)
        .bind(self.recovering)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if affected != 1 {
            return Err(corrupt());
        }
        Ok(())
    }
}

struct Artifact {
    hash: String,
    version: i64,
    applies_at: i64,
    expires_at: i64,
    effective_after: i64,
    floor: i64,
    software: SoftwareVersion,
    accepted: bool,
    incompatible: bool,
    resolution: bool,
}

impl Artifact {
    fn from_row(row: &PgRow) -> Result<Self, WriteError> {
        let components: Vec<i64> = row.try_get("software")?;
        let [major, minor, patch] = components.as_slice() else {
            return Err(corrupt());
        };
        let software = SoftwareVersion::new(*major as u64, *minor as u64, *patch as u64)
            .ok_or_else(corrupt)?;
        Ok(Self {
            hash: row.try_get("hash")?,
            version: row.try_get("version")?,
            applies_at: row.try_get("applies_at")?,
            expires_at: row.try_get("expires_at")?,
            effective_after: row.try_get("effective_after")?,
            floor: row.try_get("floor")?,
            software,
            accepted: row.try_get("accepted")?,
            incompatible: row.try_get("incompatible")?,
            resolution: row.try_get("resolution")?,
        })
    }

    fn due(&self, now: i64) -> bool {
        now >= self.applies_at.max(self.effective_after)
    }

    fn digest(&self) -> Result<Hash, WriteError> {
        let mut hash = [0; 32];
        hex::decode_to_slice(&self.hash, &mut hash).map_err(|_| corrupt())?;
        Ok(hash)
    }
}

async fn load_artifact(tx: &mut Tx<'_>, hash: &str) -> Result<Option<Artifact>, WriteError> {
    sqlx::query(
        "SELECT hash, version, applies_at, expires_at, effective_after, floor, software,
                accepted, incompatible, resolution FROM federation_config_frame WHERE hash = $1",
    )
    .bind(hash)
    .fetch_optional(&mut **tx)
    .await?
    .as_ref()
    .map(Artifact::from_row)
    .transpose()
}

async fn reference(tx: &mut Tx<'_>, hash: Option<&str>) -> Result<Option<Artifact>, WriteError> {
    match hash {
        None => Ok(None),
        Some(hash) => Ok(Some(load_artifact(tx, hash).await?.ok_or_else(corrupt)?)),
    }
}

async fn stored_frame(tx: &mut Tx<'_>, hash: &str) -> Result<Vec<u8>, WriteError> {
    sqlx::query_scalar("SELECT frame FROM federation_config_frame WHERE hash = $1")
        .bind(hash)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(corrupt)
}

async fn stored_policy(tx: &mut Tx<'_>) -> Result<Option<StoredPolicy>, WriteError> {
    Ok(sqlx::query_as(
        "SELECT profile, cluster, namespace, scope_id, epoch, role, voters
           FROM federation_config_policy WHERE singleton",
    )
    .fetch_optional(&mut **tx)
    .await?)
}

async fn lock_policy_table(tx: &mut Tx<'_>) -> Result<(), WriteError> {
    sqlx::query("LOCK TABLE federation_config_policy IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl BundleService {
    pub async fn preflight(
        pool: &PgPool,
        portable: Option<&PortableJoin>,
        policy: Option<ServicePolicy>,
    ) -> Result<Preflight, WriteError> {
        let mut tx = pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await?;
        let Some(mut policy) = policy else {
            lock_policy_table(&mut tx).await?;
            let state = stored_policy(&mut tx).await?;
            if state.is_some() {
                return Err(Rejection::Policy.into());
            }
            tx.commit().await?;
            return Ok(Preflight::Disabled);
        };
        let portable = portable.ok_or(Rejection::Policy)?;
        if policy.bundle.cluster != portable.cluster() || policy.profile != PROFILE {
            return Err(Rejection::Policy.into());
        }
        policy.bundle.validate().map_err(Rejection::Codec)?;
        policy
            .bundle
            .voters
            .sort_unstable_by(|a, b| a.node_id.cmp(&b.node_id));
        let voters = policy
            .bundle
            .voters
            .iter()
            .map(|v| format!("{}:{}", v.node_id, v.public_key))
            .collect();
        let signers = policy
            .bundle
            .voters
            .iter()
            .map(|v| v.node_id.clone())
            .collect();
        let service = Self {
            pool: pool.clone(),
            policy,
            voters,
            signers,
        };
        portable.lock_cluster(&mut tx).await?;
        // Também serializa a primeira declaração concorrente de outro cluster.
        lock_policy_table(&mut tx).await?;
        if let Some(stored) = stored_policy(&mut tx).await? {
            service.check_policy(&stored)?;
            service.validate_state(&mut tx).await?;
        } else {
            sqlx::query(
                "INSERT INTO federation_config_policy (profile, cluster, namespace, scope_id, epoch, role, voters)
                 VALUES ($1, $2, $3, '', $4, $5, $6)",
            ).bind(service.policy.profile as i64).bind(service.cluster()).bind(NAMESPACE)
                .bind(service.policy.bundle.epoch as i64).bind(service.policy.role.as_str())
                .bind(&service.voters).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO federation_config_state DEFAULT VALUES")
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Preflight::Enabled(service))
    }

    pub fn policy(&self) -> &ServicePolicy {
        &self.policy
    }

    fn cluster(&self) -> &str {
        &self.policy.bundle.cluster
    }

    fn check_policy(&self, stored: &StoredPolicy) -> Result<(), WriteError> {
        let (profile, cluster, namespace, scope_id, epoch, role, voters) = stored;
        if *profile != self.policy.profile as i64
            || cluster != self.cluster()
            || namespace != NAMESPACE
            || !scope_id.is_empty()
            || *epoch != self.policy.bundle.epoch as i64
            || role != self.policy.role.as_str()
            || voters != &self.voters
        {
            return Err(Rejection::Policy.into());
        }
        Ok(())
    }

    async fn check_context(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        expected_role: Option<Role>,
    ) -> Result<(), WriteError> {
        if guard.cluster() != self.cluster() {
            return Err(Rejection::Policy.into());
        }
        let stored = stored_policy(guard.transaction())
            .await?
            .ok_or_else(corrupt)?;
        if expected_role.is_some_and(|role| stored.5 != role.as_str()) {
            return Err(Rejection::WrongRole.into());
        }
        self.check_policy(&stored)
    }

    fn check_portable(&self, portable: &PortableJoin) -> Result<(), WriteError> {
        if portable.cluster() != self.cluster() {
            return Err(Rejection::Policy.into());
        }
        Ok(())
    }

    async fn begin(&self, portable: &PortableJoin) -> Result<Tx<'static>, WriteError> {
        self.check_portable(portable)?;
        Ok(self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await?)
    }

    fn authenticate(&self, raw: &[u8]) -> Result<AuthenticatedBundle, WriteError> {
        Ok(authenticate_bundle(raw, &self.policy.bundle).map_err(Rejection::Codec)?)
    }

    async fn validate_state(&self, tx: &mut Tx<'_>) -> Result<(), WriteError> {
        let state = State::load(tx).await?;
        let mut cursor = String::new();
        let mut highest: Option<(i64, String)> = None;
        let mut incompatible: Option<(i64, String)> = None;
        // Frames históricos também podem ser necessários para replay ou
        // resolução. Conferir em lotes, sem acumular os bytes na memória.
        loop {
            let rows = sqlx::query(
                "SELECT hash, version, applies_at, expires_at, effective_after, floor, software,
                        accepted, incompatible, resolution, signers, frame
                   FROM federation_config_frame WHERE hash > $1 ORDER BY hash LIMIT 32",
            )
            .bind(&cursor)
            .fetch_all(&mut **tx)
            .await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                let artifact = Artifact::from_row(&row)?;
                let raw: Vec<u8> = row.try_get("frame")?;
                let signers: Vec<String> = row.try_get("signers")?;
                self.verify_stored(&artifact, &raw)?;
                if signers != self.signers {
                    return Err(corrupt());
                }
                if artifact.accepted && highest.as_ref().is_none_or(|(v, _)| *v < artifact.version)
                {
                    highest = Some((artifact.version, artifact.hash.clone()));
                }
                if artifact.incompatible
                    && !artifact.accepted
                    && incompatible
                        .as_ref()
                        .is_none_or(|(v, _)| *v < artifact.version)
                {
                    incompatible = Some((artifact.version, artifact.hash.clone()));
                }
                cursor = artifact.hash;
            }
        }
        let incompatible =
            incompatible.filter(|(v, _)| highest.as_ref().is_none_or(|(a, _)| v > a));
        // Os marcadores não podem retroceder nem apagar um conflito que a
        // auditoria ainda não registrou como resolvido.
        let (last_effective, open_conflict, recovering): (Option<String>, Option<String>, bool) = sqlx::query_as(
            "SELECT
                (SELECT hash FROM federation_config_audit WHERE transition = 'activated'
                  ORDER BY id DESC LIMIT 1),
                (SELECT hash FROM federation_config_audit WHERE transition = 'conflict'
                   AND id > COALESCE((SELECT max(id) FROM federation_config_audit WHERE transition = 'resolved'), 0)
                  ORDER BY id LIMIT 1),
                COALESCE((SELECT max(id) FROM federation_config_audit
                            WHERE transition IN ('resolved', 'recovery_required')), 0)
                  > COALESCE((SELECT max(id) FROM federation_config_audit
                                WHERE transition = 'activated'), 0)",
        ).fetch_one(&mut **tx).await?;
        let expected_published = match self.policy.role {
            Role::Publisher => state.accepted.as_ref(),
            Role::Receiver => None,
        };
        if highest.as_ref().map(|(_, hash)| hash) != state.accepted.as_ref()
            || incompatible.as_ref().map(|(_, hash)| hash) != state.incompatible.as_ref()
            || state.published.as_ref() != expected_published
            || state.effective != last_effective
            || state.conflict != open_conflict
            || state.recovering != recovering
        {
            return Err(corrupt());
        }
        if let Some(effective) = reference(tx, state.effective.as_deref()).await?
            && (!effective.accepted
                || (state.pending.is_some()
                    && highest
                        .as_ref()
                        .is_none_or(|(version, _)| effective.version >= *version)))
        {
            return Err(corrupt());
        }
        if let Some(conflict) = reference(tx, state.conflict.as_deref()).await?
            && (conflict.accepted
                || conflict.incompatible
                || !has_competing_primary(tx, conflict.version, &conflict.hash).await?)
        {
            return Err(corrupt());
        }
        Ok(())
    }

    fn verify_stored(
        &self,
        artifact: &Artifact,
        raw: &[u8],
    ) -> Result<AuthenticatedBundle, WriteError> {
        if raw.len() > self.policy.bundle.max_frame_bytes.get() {
            return Err(Rejection::Policy.into());
        }
        let auth = self.authenticate(raw).map_err(|_| corrupt())?;
        let body = auth.body();
        if auth.hash() != &artifact.digest()?
            || body.version as i64 != artifact.version
            || body.applies_at != artifact.applies_at
            || body.expires_at != artifact.expires_at
            || body.effective_after != artifact.effective_after
            || body.value as i64 != artifact.floor
            || body.min_version != artifact.software
        {
            return Err(corrupt());
        }
        Ok(auth)
    }
}

async fn has_competing_primary(
    tx: &mut Tx<'_>,
    version: i64,
    hash: &str,
) -> Result<bool, WriteError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM federation_config_frame
          WHERE version = $1 AND hash <> $2 AND (accepted OR incompatible))",
    )
    .bind(version)
    .bind(hash)
    .fetch_one(&mut **tx)
    .await?)
}

enum Submission<'a> {
    Ordinary(Role),
    Resolve { role: Role, actor: &'a str },
}

impl Submission<'_> {
    fn role(&self) -> Role {
        match *self {
            Self::Ordinary(role) | Self::Resolve { role, .. } => role,
        }
    }
}

impl BundleService {
    /// Prepara a publicação na transação da fronteira autorizada.
    pub async fn publish_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        portable: &PortableJoin,
        frame: &[u8],
        now: i64,
    ) -> Result<WriteOutcome, WriteError> {
        self.submit(
            guard,
            portable,
            frame,
            now,
            Submission::Ordinary(Role::Publisher),
        )
        .await
    }

    /// Prepara a aceitação no receptor, incluindo a evidência quando aplicável.
    pub async fn accept_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        portable: &PortableJoin,
        frame: &[u8],
        now: i64,
    ) -> Result<WriteOutcome, WriteError> {
        self.submit(
            guard,
            portable,
            frame,
            now,
            Submission::Ordinary(Role::Receiver),
        )
        .await
    }

    /// Aceitação local. Amostra o relógio uma vez após adquirir o lock.
    pub async fn accept(
        &self,
        portable: &PortableJoin,
        frame: &[u8],
        clock: impl FnOnce() -> i64,
    ) -> Result<WriteOutcome, WriteError> {
        let mut tx = self.begin(portable).await?;
        let mut guard = portable.lock_cluster_guard(&mut tx).await?;
        match self
            .accept_in_tx(&mut guard, portable, frame, clock())
            .await
        {
            Ok(result) => {
                tx.commit().await?;
                Ok(result)
            }
            Err(error) => {
                tx.rollback().await?;
                Err(error)
            }
        }
    }

    /// Prepara a recuperação auditada. O ator vem da fronteira confiável.
    pub async fn resolve_conflict_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        portable: &PortableJoin,
        frame: &[u8],
        role: Role,
        actor: &str,
        now: i64,
    ) -> Result<WriteOutcome, WriteError> {
        self.submit(
            guard,
            portable,
            frame,
            now,
            Submission::Resolve { role, actor },
        )
        .await
    }

    async fn submit(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        portable: &PortableJoin,
        frame: &[u8],
        now: i64,
        submission: Submission<'_>,
    ) -> Result<WriteOutcome, WriteError> {
        self.check_portable(portable)?;
        self.check_context(guard, Some(submission.role())).await?;
        let authenticated = self.authenticate(frame)?;
        portable.authorize_local_at(guard, now).await?;
        let tx = guard.transaction();
        let body = authenticated.body();
        let hash = hex::encode(authenticated.hash());
        let version = body.version as i64;
        let status = authenticated.evaluate(now, self.policy.software);
        let eligible = matches!(status, BundleStatus::Pending | BundleStatus::Eligible);
        let mut state = State::load(tx).await?;
        let known = load_artifact(tx, &hash).await?;
        if let Submission::Resolve { actor, .. } = submission {
            if actor.trim().is_empty() {
                return Err(Rejection::InvalidActor.into());
            }
            if known.as_ref().is_some_and(|existing| existing.resolution) {
                return Ok(WriteOutcome::Replay);
            }
            if state.conflict.is_none() {
                return Err(Rejection::NoOpenConflict.into());
            }
            let maximum: Option<i64> =
                sqlx::query_scalar("SELECT max(version) FROM federation_config_frame")
                    .fetch_one(&mut **tx)
                    .await?;
            if maximum.is_some_and(|highest| version <= highest) {
                return Err(Rejection::Rollback.into());
            }
            require_eligible(status)?;
            self.record(tx, frame, &authenticated, RecordedAs::Resolution)
                .await?;
            state.conflict = None;
            state.recovering = true;
            if let Some(incompatible) = reference(tx, state.incompatible.as_deref()).await?
                && version >= incompatible.version
            {
                state.incompatible = None;
            }
            self.set_pending(tx, &mut state, hash).await?;
            audit(tx, authenticated.hash(), now, "resolved", Some(actor)).await?;
            return Ok(WriteOutcome::Accepted);
        }

        // Evidência concorrente de uma versão histórica vem antes de frescor
        // e piso. O primeiro frame daquela versão já está durável.
        if known.is_none() && has_competing_primary(tx, version, &hash).await? {
            self.record(tx, frame, &authenticated, RecordedAs::Conflict)
                .await?;
            if state.conflict.is_none() {
                state.conflict = Some(hash);
            }
            state.save(tx).await?;
            audit(tx, authenticated.hash(), now, "conflict", None).await?;
            return Ok(WriteOutcome::ConflictRecorded);
        }
        if let Some(existing) = &known {
            if existing.accepted {
                return Ok(WriteOutcome::Replay);
            }
            if !existing.incompatible {
                return Ok(WriteOutcome::ConflictKnown);
            }
            if !eligible {
                return Ok(WriteOutcome::IncompatibleKnown);
            }
        }
        if state.conflict.is_some() {
            return Err(Rejection::ConflictOpen.into());
        }
        let accepted = reference(tx, state.accepted.as_deref()).await?;
        let incompatible = reference(tx, state.incompatible.as_deref()).await?;
        if accepted
            .as_ref()
            .is_some_and(|record| version < record.version)
            || incompatible
                .as_ref()
                .is_some_and(|record| version < record.version)
        {
            return Err(Rejection::Rollback.into());
        }
        if status == BundleStatus::Expired {
            return Err(Rejection::Expired.into());
        }
        if status == BundleStatus::IncompatibleSoftware {
            self.record(tx, frame, &authenticated, RecordedAs::Incompatible)
                .await?;
            state.incompatible = Some(hash);
            state.save(tx).await?;
            audit(tx, authenticated.hash(), now, "incompatible", None).await?;
            return Ok(WriteOutcome::IncompatibleRecorded);
        }

        // Uma pendência já exigível ou uma evidência exigível que será
        // superada mantém o escopo fechado até a ativação corretiva.
        let pending = reference(tx, state.pending.as_deref()).await?;
        let incompatible_due =
            incompatible_due(tx, accepted.as_ref().map(|record| record.version), now).await?;
        let starts_recovery = !state.recovering
            && (pending.is_some_and(|record| record.due(now)) || incompatible_due);
        state.recovering |= starts_recovery;
        if incompatible
            .as_ref()
            .is_some_and(|record| version >= record.version)
        {
            state.incompatible = None;
        }
        if known.is_some() {
            sqlx::query("UPDATE federation_config_frame SET accepted = true WHERE hash = $1")
                .bind(&hash)
                .execute(&mut **tx)
                .await?;
        } else {
            self.record(tx, frame, &authenticated, RecordedAs::Accepted)
                .await?;
        }
        self.set_pending(tx, &mut state, hash).await?;
        audit(tx, authenticated.hash(), now, "accepted", None).await?;
        if starts_recovery {
            audit(tx, authenticated.hash(), now, "recovery_required", None).await?;
        }
        Ok(WriteOutcome::Accepted)
    }

    async fn set_pending(
        &self,
        tx: &mut Tx<'_>,
        state: &mut State,
        hash: String,
    ) -> Result<(), WriteError> {
        state.pending = Some(hash.clone());
        state.accepted = Some(hash.clone());
        if self.policy.role == Role::Publisher {
            state.published = Some(hash);
        }
        state.save(tx).await
    }

    async fn record(
        &self,
        tx: &mut Tx<'_>,
        frame: &[u8],
        bundle: &AuthenticatedBundle,
        kind: RecordedAs,
    ) -> Result<(), WriteError> {
        let body = bundle.body();
        let software = body.min_version.components().map(|value| value as i64);
        sqlx::query(
            "INSERT INTO federation_config_frame
                (hash, version, frame, applies_at, expires_at, effective_after, floor, software,
                 signers, accepted, incompatible, resolution)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(hex::encode(bundle.hash()))
        .bind(body.version as i64)
        .bind(frame)
        .bind(body.applies_at)
        .bind(body.expires_at)
        .bind(body.effective_after)
        .bind(body.value as i64)
        .bind(software.as_slice())
        .bind(&self.signers)
        .bind(matches!(
            kind,
            RecordedAs::Accepted | RecordedAs::Resolution
        ))
        .bind(matches!(kind, RecordedAs::Incompatible))
        .bind(matches!(kind, RecordedAs::Resolution))
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn activate_due(
        &self,
        portable: &PortableJoin,
        clock: impl FnOnce() -> i64,
    ) -> Result<Activation, WriteError> {
        let mut tx = self.begin(portable).await?;
        let mut guard = portable.lock_cluster_guard(&mut tx).await?;
        let now = clock();
        self.check_context(&mut guard, None).await?;
        portable.authorize_local_at(&mut guard, now).await?;
        let tx_ref = guard.transaction();
        let mut state = State::load(tx_ref).await?;
        let result = self.activate_in_tx(tx_ref, &mut state, now).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn activate_in_tx(
        &self,
        tx: &mut Tx<'_>,
        state: &mut State,
        now: i64,
    ) -> Result<Activation, WriteError> {
        if state.conflict.is_some() {
            return Ok(Activation::Blocked(Block::Conflict));
        }
        let accepted = reference(tx, state.accepted.as_deref()).await?;
        if incompatible_due(tx, accepted.map(|record| record.version), now).await? {
            return Ok(Activation::Blocked(Block::Incompatible));
        }
        let Some(pending) = reference(tx, state.pending.as_deref()).await? else {
            return Ok(Activation::NoChange);
        };
        let raw = stored_frame(tx, &pending.hash).await?;
        let auth = self.verify_stored(&pending, &raw)?;
        match auth.evaluate(now, self.policy.software) {
            BundleStatus::Expired => return Ok(Activation::Blocked(Block::Expired)),
            BundleStatus::IncompatibleSoftware => {
                return Ok(Activation::Blocked(Block::Incompatible));
            }
            BundleStatus::Pending => return Ok(Activation::NoChange),
            BundleStatus::Eligible => {}
        }
        state.effective = state.pending.take();
        state.recovering = false;
        state.save(tx).await?;
        audit(tx, auth.hash(), now, "activated", None).await?;
        Ok(Activation::Activated)
    }

    /// Prepara corpo e hash sob o lock do leitor autorizado; não faz I/O externo.
    pub async fn select_published_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        conditional: Option<&Hash>,
        now: i64,
    ) -> Result<Published, WriteError> {
        self.check_context(guard, Some(Role::Publisher)).await?;
        let tx = guard.transaction();
        let state = State::load(tx).await?;
        if state.conflict.is_some() {
            return Ok(Published::BlockedConflict);
        }
        let Some(published) = reference(tx, state.published.as_deref()).await? else {
            return Ok(Published::Unavailable);
        };
        if now >= published.expires_at {
            return Ok(Published::Unavailable);
        }
        let hash = published.digest()?;
        let version = published.version as u64;
        if conditional == Some(&hash) {
            return Ok(Published::NotModified { version, hash });
        }
        let frame = stored_frame(tx, &published.hash).await?;
        Ok(Published::Frame {
            frame,
            version,
            hash,
        })
    }

    /// Classificação no mesmo lock e transação que protegem o efeito.
    ///
    /// ```compile_fail
    /// # use godmesh_server::config_bundle::BundleService;
    /// # use sqlx::{Postgres, Transaction};
    /// # async fn run(service: &BundleService, tx: &mut Transaction<'_, Postgres>) {
    /// service.classify_fence_in_tx(tx, 1, 0).await.unwrap();
    /// # }
    /// ```
    pub async fn classify_fence_in_tx(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        protocol: u64,
        now: i64,
    ) -> Result<Fence, WriteError> {
        self.check_context(guard, None).await?;
        let tx = guard.transaction();
        let state = State::load(tx).await?;
        if state.conflict.is_some() {
            return Ok(Fence::Blocked(Block::Conflict));
        }
        if state.recovering {
            return Ok(Fence::Blocked(Block::Recovering));
        }
        let accepted = reference(tx, state.accepted.as_deref()).await?;
        if incompatible_due(tx, accepted.map(|record| record.version), now).await? {
            return Ok(Fence::Blocked(Block::Incompatible));
        }
        if reference(tx, state.pending.as_deref())
            .await?
            .is_some_and(|record| record.due(now))
        {
            return Ok(Fence::Blocked(Block::PendingDue));
        }
        let Some(effective) = reference(tx, state.effective.as_deref()).await? else {
            return Ok(Fence::Blocked(Block::NoEffective));
        };
        if now >= effective.expires_at {
            return Ok(Fence::Blocked(Block::Expired));
        }
        if self.policy.software < effective.software {
            return Ok(Fence::Blocked(Block::Incompatible));
        }
        if protocol < effective.floor as u64 {
            return Ok(Fence::Blocked(Block::BelowFloor));
        }
        Ok(Fence::Allowed {
            version: effective.version as u64,
            hash: effective.digest()?,
            floor: effective.floor as u64,
            expires_at: effective.expires_at,
        })
    }
}

enum RecordedAs {
    Accepted,
    Incompatible,
    Conflict,
    Resolution,
}

fn require_eligible(status: BundleStatus) -> Result<(), WriteError> {
    match status {
        BundleStatus::Expired => Err(Rejection::Expired.into()),
        BundleStatus::IncompatibleSoftware => Err(Rejection::Incompatible.into()),
        BundleStatus::Pending | BundleStatus::Eligible => Ok(()),
    }
}

async fn incompatible_due(
    tx: &mut Tx<'_>,
    accepted: Option<i64>,
    now: i64,
) -> Result<bool, WriteError> {
    // Uma evidência mais nova incompatível não cancela exigências anteriores
    // da mesma série. Somente aceitação igual ou superior pode superá-las.
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM federation_config_frame
          WHERE incompatible AND NOT accepted AND version > COALESCE($1, 0)
            AND GREATEST(applies_at, effective_after) <= $2)",
    )
    .bind(accepted)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?)
}

async fn audit(
    tx: &mut Tx<'_>,
    hash: &Hash,
    now: i64,
    transition: &str,
    actor: Option<&str>,
) -> Result<(), WriteError> {
    sqlx::query(
        "INSERT INTO federation_config_audit (hash, happened_at, transition, actor) VALUES ($1, $2, $3, $4)",
    ).bind(hex::encode(hash)).bind(now).bind(transition).bind(actor)
        .execute(&mut **tx).await?;
    Ok(())
}
