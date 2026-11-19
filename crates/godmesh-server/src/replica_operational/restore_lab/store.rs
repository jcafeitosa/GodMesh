use godmesh_federation::restore_evidence::{
    Fence, RestoreKey, VerifiedHighWater, VerifiedPromotion,
};
use sqlx::{Connection, PgConnection, PgPool, Row};
use tokio::time::{Instant, timeout, timeout_at};

use super::{Error, PromotionCandidate, db, sql_budget, with_sql_deadline};

pub(super) const KEY_SQL: &str =
    "fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch";

macro_rules! keyed {
    ($sql:expr, $key:expr) => {
        sqlx::query($sql)
            .bind(&$key.fixture_id)
            .bind(&$key.cluster)
            .bind(&$key.local_node)
            .bind(&$key.origin_node)
            .bind(&$key.tenant_id)
            .bind(&$key.stream_id)
            .bind($key.partition as i32)
            .bind($key.authority_epoch as i64)
    };
}
pub(super) use keyed;

macro_rules! where_key {
    () => {
        "(fixture_id,cluster,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5,$6,$7,$8)"
    };
}
pub(super) use where_key;

pub(super) fn read_digest(row: &sqlx::postgres::PgRow, field: &str) -> Result<[u8; 32], Error> {
    let bytes: &[u8] = row.try_get(field)?;
    bytes.try_into().map_err(|_| Error::Incomplete)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPhase {
    Fenced,
    PromotionConsumed,
}

/// Fotografia do ponto de controle externo, obtida somente por leitura/CAS.
#[derive(Clone, Debug)]
pub struct ExternalObservation {
    pub(super) binding: super::Binding,
    pub(super) phase: ExternalPhase,
    pub(super) decision: Option<Decision>,
    opening_origin: Option<super::OriginObservation>,
    pub(super) origin: Option<super::OriginObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Decision {
    pub consumed_at: i64,
    pub authorization: [u8; 32],
    pub reconciliation: [u8; 32],
}

/// Leitura de A efetuada após observar a decisão externa.
#[cfg_attr(test, derive(Clone))]
pub struct OpeningOrigin {
    binding: super::Binding,
    decision: Decision,
    pub(super) observation: super::OriginObservation,
}

pub async fn observe_opening_origin(
    service: &super::ReplicaService,
    consumed: &ExternalObservation,
    peer: super::SessionPeer<'_>,
    deadline: Instant,
) -> Result<OpeningOrigin, Error> {
    let decision = consumed.decision.ok_or(Error::Incomplete)?;
    let observation = super::observe_origin(service, &consumed.binding.key, peer, deadline).await?;
    Ok(OpeningOrigin {
        binding: consumed.binding.clone(),
        decision,
        observation,
    })
}

impl ExternalObservation {
    pub fn binding(&self) -> &super::Binding {
        &self.binding
    }
    pub fn phase(&self) -> ExternalPhase {
        self.phase
    }
    pub fn origin(&self) -> Option<&super::OriginObservation> {
        self.origin.as_ref()
    }
    pub fn opening_origin(&self) -> Option<&super::OriginObservation> {
        self.opening_origin.as_ref()
    }
    pub fn authorization_sha256(&self) -> Option<[u8; 32]> {
        self.decision.map(|d| d.authorization)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationLogins {
    pub target_login: String,
    pub observer_login: String,
    pub cas_login: String,
}

impl GenerationLogins {
    fn validate(&self) -> Result<(), Error> {
        let valid =
            |login: &str| !login.trim().is_empty() && login.len() <= 63 && !login.contains('\0');
        if ![&self.target_login, &self.observer_login, &self.cas_login]
            .into_iter()
            .all(|login| valid(login))
            || self.target_login == self.observer_login
            || self.target_login == self.cas_login
            || self.observer_login == self.cas_login
        {
            return Err(Error::Scope);
        }
        Ok(())
    }
}

pub struct ExternalStore(PgPool, std::sync::Arc<dyn Fn() -> i64 + Send + Sync>);

async fn in_tx<T>(
    pool: &PgPool,
    deadline: Instant,
    op: impl AsyncFnOnce(&mut PgConnection) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut lease = db::Lease::acquire(pool, deadline).await?;
    let mut tx = timeout_at(
        deadline,
        lease.begin_with("BEGIN ISOLATION LEVEL READ COMMITTED"),
    )
    .await
    .map_err(|_| Error::Timeout)??;
    let outcome = timeout_at(
        deadline,
        with_sql_deadline(deadline, async {
            let result = op(&mut tx).await?;
            sql_budget::refresh(&mut tx).await?;
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            Ok(result)
        }),
    )
    .await
    .unwrap_or(Err(Error::Timeout));
    match outcome {
        Ok(result) => {
            db::commit(tx, deadline).await?;
            lease.clean();
            Ok(result)
        }
        Err(error) => {
            if matches!(
                timeout(std::time::Duration::from_secs(1), tx.rollback()).await,
                Ok(Ok(()))
            ) {
                lease.clean();
            }
            Err(error)
        }
    }
}

impl ExternalStore {
    pub fn new(pool: PgPool) -> Self {
        Self(pool, std::sync::Arc::new(godmesh_federation::now_unix))
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        self.1 = clock;
        self
    }

    /// Seed inicial administrativo do exercício. Proíbe sobrescrever a chave.
    pub async fn seed(&self, binding: &super::Binding, deadline: Instant) -> Result<(), Error> {
        binding.validate()?;
        if binding.generation != 1 {
            return Err(Error::Scope);
        }
        in_tx(&self.0,deadline,async |db| {
            sql_budget::refresh(db).await?;
            let query = format!("INSERT INTO federation_restore_control ({KEY_SQL},generation,target_db_id,high_water_sha256,fence_sha256,phase)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'fenced') ON CONFLICT DO NOTHING");
            let inserted = keyed!(sqlx::AssertSqlSafe(query),binding.key).bind(binding.generation as i64).bind(&binding.target_db_id)
                .bind(&binding.high_water_sha256[..]).bind(&binding.fence_sha256[..]).execute(db).await?;
            if inserted.rows_affected()!=1 { return Err(Error::Conflict); }
            Ok(())
        }).await
    }

    /// Registro administrativo. Repetição exata é idempotente.
    pub async fn register_logins(
        &self,
        binding: &super::Binding,
        logins: &GenerationLogins,
        deadline: Instant,
    ) -> Result<(), Error> {
        binding.validate()?;
        logins.validate()?;
        in_tx(&self.0, deadline, async |db| {
            sql_budget::refresh(db).await?;
            let query = format!(
                "INSERT INTO federation_restore_generation_logins
                ({KEY_SQL},generation,target_db_id,target_login,observer_login,cas_login)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) ON CONFLICT DO NOTHING"
            );
            keyed!(sqlx::AssertSqlSafe(query), binding.key)
                .bind(binding.generation as i64)
                .bind(&binding.target_db_id)
                .bind(&logins.target_login)
                .bind(&logins.observer_login)
                .bind(&logins.cas_login)
                .execute(&mut *db)
                .await?;
            match read_logins(db, &binding.key, binding.generation, &binding.target_db_id).await {
                Ok(stored) if stored == *logins => Ok(()),
                Ok(_) | Err(Error::Incomplete) => Err(Error::Conflict),
                Err(error) => Err(error),
            }
        })
        .await
    }

    pub async fn logins(
        &self,
        binding: &super::Binding,
        deadline: Instant,
    ) -> Result<GenerationLogins, Error> {
        binding.validate()?;
        in_tx(&self.0, deadline, async |db| {
            read_logins(db, &binding.key, binding.generation, &binding.target_db_id).await
        })
        .await
    }

    pub async fn observe(
        &self,
        key: &RestoreKey,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        in_tx(&self.0, deadline, async |db| observe(db, key, false).await).await
    }

    /// Aguarda o CAS pendente antes de apurar uma confirmação incerta.
    pub async fn resolve_promotion(
        &self,
        key: &RestoreKey,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        in_tx(&self.0, deadline, async |db| observe(db, key, true).await).await
    }

    pub async fn publish(
        &self,
        high_water: &VerifiedHighWater,
        fence: &Fence,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        let high = high_water.high_water();
        let binding = super::Binding::from_high_water(high_water)?;
        if fence.key != high.key
            || fence.fenced_generation != high.fenced_generation
            || fence.target_db_id != high.target_db_id
            || fence.target_db_id == fence.cut_db_id
            || fence.digest()? != high.fence_sha256
            || !fence.admission_closed
            || !fence.nologin_confirmed
            || !fence.grants_removed
            || fence.remaining_sessions != 0
            || fence.unknown_effects != 0
            || fence.fenced_at > high.cut_at
        {
            return Err(Error::Incomplete);
        }
        in_tx(&self.0,deadline,async |db| {
            let old = observe(db,&high.key,true).await?;
            let previous_logins = read_logins(db, &high.key, high.fenced_generation, &fence.cut_db_id).await?;
            if previous_logins.target_login != fence.old_login {
                return Err(Error::Conflict);
            }
            read_logins(db, &high.key, binding.generation, &binding.target_db_id).await?;
            if old.binding == binding { return Ok(old); }
            if old.binding.generation != high.fenced_generation || old.binding.target_db_id != fence.cut_db_id
                || high.fenced_generation.checked_add(1) != Some(binding.generation) {
                return Err(Error::Conflict);
            }
            sql_budget::refresh(db).await?;
            let changed = keyed!(concat!(
                "UPDATE federation_restore_control SET generation=$9,target_db_id=$10,high_water_sha256=$11,
                 fence_sha256=$12,phase='fenced',origin_observation=NULL WHERE ",where_key!()," AND generation=$13"
            ),high.key)
            .bind(binding.generation as i64).bind(&binding.target_db_id).bind(&binding.high_water_sha256[..])
            .bind(&binding.fence_sha256[..]).bind(high.fenced_generation as i64).execute(&mut *db).await?;
            if changed.rows_affected()!=1 { return Err(Error::Conflict); }
            observe(db,&high.key,true).await
        }).await
    }

    pub async fn record_origin(
        &self,
        binding: &super::Binding,
        origin: &super::OriginObservation,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        if origin.0.key != binding.key {
            return Err(Error::Scope);
        }
        let data = origin.encode()?;
        in_tx(&self.0, deadline, async |db| {
            let current = observe(db, &binding.key, true).await?;
            if current.binding != *binding
                || current.phase != ExternalPhase::Fenced
                || !((origin.0.sampled_at..origin.0.valid_until).contains(&(self.1)()))
            {
                return Err(Error::Conflict);
            }
            sql_budget::refresh(db).await?;
            let result = keyed!(
                concat!(
                    "UPDATE federation_restore_control SET origin_observation=$10 WHERE ",
                    where_key!(),
                    " AND generation=$9 AND phase='fenced'"
                ),
                binding.key
            )
            .bind(binding.generation as i64)
            .bind(data)
            .execute(&mut *db)
            .await?;
            if result.rows_affected() != 1 {
                return Err(Error::Conflict);
            }
            observe(db, &binding.key, true).await
        })
        .await
    }

    pub async fn record_opening_origin(
        &self,
        opening: &OpeningOrigin,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        let binding = &opening.binding;
        let origin = &opening.observation;
        let data = origin.encode()?;
        in_tx(&self.0, deadline, async |db| {
            let current = observe(db, &binding.key, true).await?;
            let now = (self.1)();
            if current.binding != *binding
                || current.decision != Some(opening.decision)
                || origin.0.sampled_at < opening.decision.consumed_at
                || !(origin.0.sampled_at..origin.0.valid_until).contains(&now)
                || current
                    .opening_origin()
                    .is_some_and(|previous| previous.0.sampled_at > origin.0.sampled_at)
            {
                return Err(Error::Conflict);
            }
            sql_budget::refresh(db).await?;
            keyed!(
                concat!(
                    "UPDATE federation_restore_decision SET opening_origin_observation=$10 WHERE ",
                    where_key!(),
                    " AND generation=$9"
                ),
                binding.key
            )
            .bind(binding.generation as i64)
            .bind(data)
            .execute(&mut *db)
            .await?;
            observe(db, &binding.key, true).await
        })
        .await
    }

    pub(super) async fn consume(
        &self,
        promotion: &VerifiedPromotion,
        candidate: &PromotionCandidate,
        deadline: Instant,
    ) -> Result<ExternalObservation, Error> {
        let body = promotion.promotion();
        let authorization = promotion.digest();
        let reconciliation = body.reconciliation_sha256;
        if candidate.authorization != Some(authorization)
            || !candidate.binding.matches_promotion(body)
            || candidate.reconciliation.digest()? != reconciliation
        {
            return Err(Error::Conflict);
        }
        in_tx(&self.0,deadline,async |db| {
            let observed = observe(db,&body.key,true).await?;
            if observed.binding != candidate.binding || observed.origin.as_ref()!=Some(&candidate.origin) { return Err(Error::Conflict); }
            if observed.phase == ExternalPhase::PromotionConsumed {
                return if observed.decision.is_some_and(|d| d.authorization == authorization && d.reconciliation == reconciliation) { Ok(observed) } else { Err(Error::Conflict) };
            }
            let now = (self.1)();
            if !super::window(body,now) || now >= candidate.valid_until { return Err(Error::Incomplete); }
            sql_budget::refresh(db).await?;
            let query = format!("INSERT INTO federation_restore_decision ({KEY_SQL},generation,authorization_sha256,reconciliation_sha256,consumed_at)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)");
            keyed!(sqlx::AssertSqlSafe(query),body.key).bind(body.target_generation as i64)
                .bind(&authorization[..]).bind(&reconciliation[..]).bind(now).execute(&mut *db).await?;
            sql_budget::refresh(db).await?;
            let changed = keyed!(concat!(
                "UPDATE federation_restore_control SET phase='promotion_consumed' WHERE ",where_key!(),
                " AND generation=$9 AND high_water_sha256=$10 AND fence_sha256=$11 AND phase='fenced'"
            ),body.key).bind(body.target_generation as i64).bind(&body.high_water_sha256[..])
                .bind(&body.fence_sha256[..]).execute(&mut *db).await?;
            if changed.rows_affected()!=1 { return Err(Error::Conflict); }
            observe(db,&body.key,true).await
        }).await
    }
}

async fn read_logins(
    db: &mut PgConnection,
    key: &RestoreKey,
    generation: u64,
    target_db_id: &str,
) -> Result<GenerationLogins, Error> {
    sql_budget::refresh(db).await?;
    let row = keyed!(
        concat!(
            "SELECT target_db_id,target_login,observer_login,cas_login
            FROM federation_restore_generation_logins WHERE ",
            where_key!(),
            " AND generation=$9"
        ),
        key
    )
    .bind(generation as i64)
    .fetch_optional(db)
    .await?
    .ok_or(Error::Incomplete)?;
    if row.try_get::<&str, _>("target_db_id")? != target_db_id {
        return Err(Error::Conflict);
    }
    let logins = GenerationLogins {
        target_login: row.try_get("target_login")?,
        observer_login: row.try_get("observer_login")?,
        cas_login: row.try_get("cas_login")?,
    };
    logins.validate()?;
    Ok(logins)
}

async fn observe(
    db: &mut PgConnection,
    key: &RestoreKey,
    lock: bool,
) -> Result<ExternalObservation, Error> {
    sql_budget::refresh(db).await?;
    if lock {
        keyed!(
            concat!(
                "SELECT generation FROM federation_restore_control WHERE ",
                where_key!(),
                " FOR UPDATE"
            ),
            key
        )
        .fetch_optional(&mut *db)
        .await?
        .ok_or(Error::Incomplete)?;
        sql_budget::refresh(db).await?;
    }
    let query=format!("SELECT c.generation,c.target_db_id,c.high_water_sha256,c.fence_sha256,c.phase,
         d.authorization_sha256,d.reconciliation_sha256,d.consumed_at,d.opening_origin_observation,c.origin_observation
         FROM federation_restore_control c LEFT JOIN federation_restore_decision d USING ({KEY_SQL},generation)
         WHERE {}",where_key!());
    // USING mescla as colunas comuns da chave no SELECT.
    let row = keyed!(sqlx::AssertSqlSafe(query), key)
        .fetch_optional(db)
        .await?
        .ok_or(Error::Incomplete)?;
    let binding = super::Binding {
        key: key.clone(),
        generation: u64::try_from(row.try_get::<i64, _>("generation")?)
            .map_err(|_| Error::Incomplete)?,
        target_db_id: row.try_get("target_db_id")?,
        high_water_sha256: read_digest(&row, "high_water_sha256")?,
        fence_sha256: read_digest(&row, "fence_sha256")?,
    };
    binding.validate()?;
    let auth: Option<&[u8]> = row.try_get("authorization_sha256")?;
    let reconciliation: Option<&[u8]> = row.try_get("reconciliation_sha256")?;
    let (phase, decision) = match (row.try_get::<&str, _>("phase")?, auth, reconciliation) {
        ("fenced", None, None) => (ExternalPhase::Fenced, None),
        ("promotion_consumed", Some(auth), Some(reconciliation)) => (
            ExternalPhase::PromotionConsumed,
            Some(Decision {
                consumed_at: row.try_get("consumed_at")?,
                authorization: auth.try_into().map_err(|_| Error::Incomplete)?,
                reconciliation: reconciliation.try_into().map_err(|_| Error::Incomplete)?,
            }),
        ),
        _ => return Err(Error::Incomplete),
    };
    let origin = row
        .try_get::<Option<&str>, _>("origin_observation")?
        .map(super::OriginObservation::decode)
        .transpose()?;
    if origin.as_ref().is_some_and(|obs| obs.0.key != *key)
        || (phase == ExternalPhase::PromotionConsumed && origin.is_none())
    {
        return Err(Error::Incomplete);
    }
    let opening_origin = row
        .try_get::<Option<&str>, _>("opening_origin_observation")?
        .map(super::OriginObservation::decode)
        .transpose()?;
    if opening_origin.as_ref().is_some_and(|opening| {
        opening.0.key != *key || decision.is_none_or(|d| opening.0.sampled_at < d.consumed_at)
    }) {
        return Err(Error::Incomplete);
    }
    Ok(ExternalObservation {
        binding,
        phase,
        decision,
        origin,
        opening_origin,
    })
}
