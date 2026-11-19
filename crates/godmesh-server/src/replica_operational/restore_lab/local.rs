use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use sqlx::{PgConnection, PgPool, Row};
use tokio::time::Instant;

use crate::portable_join::ClusterGuard;

use super::{
    Binding, EffectError, Error, ExternalObservation, ExternalPhase, ReplicaService, sql_budget,
    store::{KEY_SQL, keyed, read_digest, where_key},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalPhase {
    RestoreClosed,
    ReconciledClosed,
    Open,
    RetiredClosed,
}

#[derive(Clone)]
pub struct AdmissionGate {
    binding: Arc<Binding>,
    open: Arc<AtomicBool>,
}

impl AdmissionGate {
    pub fn new(binding: Binding) -> Result<Self, Error> {
        binding.validate()?;
        Ok(Self {
            binding: Arc::new(binding),
            open: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    pub(super) fn opened(&self) {
        self.open.store(true, Ordering::Release);
    }
    fn close(&self) {
        self.open.store(false, Ordering::Release);
    }

    pub(in crate::replica_operational) async fn authorize(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
    ) -> Result<(), EffectError> {
        if !self.open.load(Ordering::Acquire) {
            return Err(EffectError::RestoreClosed);
        }
        let row = load(guard.connection(), self.binding())
            .await
            .map_err(|e| match e {
                Error::Effect(error) => error,
                _ => EffectError::RestoreClosed,
            })?;
        if row.phase != LocalPhase::Open {
            return Err(EffectError::RestoreClosed);
        }
        Ok(())
    }
}

pub struct LocalRestore {
    pub(super) service: Arc<ReplicaService>,
    pub(super) gate: AdmissionGate,
}

impl LocalRestore {
    pub fn new(service: Arc<ReplicaService>, gate: AdmissionGate) -> Result<Self, Error> {
        let k = &gate.binding.key;
        let cfg = service.config();
        if cfg.role != super::super::Role::Receiver
            || service.local_pin().node_id != k.local_node
            || cfg.cluster != k.cluster
            || cfg.namespace.origin_node != k.origin_node
            || cfg.namespace.tenant_id != k.tenant_id
            || cfg.namespace.stream_id != k.stream_id
            || cfg.namespace.partition != k.partition as i32
            || cfg.authority_epoch != k.authority_epoch as i64
            || !service
                .restore_gate
                .as_ref()
                .is_some_and(|attached| Arc::ptr_eq(&attached.open, &gate.open))
        {
            return Err(Error::Scope);
        }
        Ok(Self { service, gate })
    }

    pub fn binding(&self) -> &Binding {
        self.gate.binding()
    }

    pub async fn phase(&self, deadline: Instant) -> Result<LocalPhase, Error> {
        self.service
            .transact_restore(deadline, async |guard, _| {
                Ok(load(guard.connection(), self.binding()).await?.phase)
            })
            .await
    }

    pub async fn boot_initial(
        &self,
        observed: &ExternalObservation,
        deadline: Instant,
    ) -> Result<(), Error> {
        self.gate.close();
        if self.binding().generation != 1
            || observed.binding() != self.binding()
            || observed.phase() != ExternalPhase::Fenced
        {
            return Err(Error::Scope);
        }
        self.service
            .transact_restore(deadline, async |guard, _| {
                ensure_allocation(guard.connection(), self.binding()).await?;
                ensure_row(guard.connection(), self.binding()).await?;
                let local = load(guard.connection(), self.binding()).await?;
                match local.phase {
                    LocalPhase::RestoreClosed | LocalPhase::Open => {
                        set_phase(guard.connection(), self.binding(), "open").await
                    }
                    LocalPhase::RetiredClosed => Err(Error::Retired),
                    _ => Err(Error::Incomplete),
                }
            })
            .await?;
        self.gate.opened();
        Ok(())
    }

    pub async fn boot_closed(
        &self,
        observed: &ExternalObservation,
        deadline: Instant,
    ) -> Result<LocalPhase, Error> {
        self.gate.close();
        if self.binding().generation <= 1 || observed.binding() != self.binding() {
            return Err(Error::Scope);
        }
        self.service
            .transact_restore(deadline, async |guard, _| {
                ensure_allocation(guard.connection(), self.binding()).await?;
                ensure_row(guard.connection(), self.binding()).await?;
                match load(guard.connection(), self.binding()).await?.phase {
                    LocalPhase::RetiredClosed => Err(Error::Retired),
                    LocalPhase::Open => {
                        set_phase(guard.connection(), self.binding(), "reconciled_closed").await?;
                        Ok(LocalPhase::ReconciledClosed)
                    }
                    phase => Ok(phase),
                }
            })
            .await
    }

    pub async fn retire(&self, deadline: Instant) -> Result<(), Error> {
        self.gate.close();
        self.service
            .transact_restore(deadline, async |guard, _| {
                ensure_allocation(guard.connection(), self.binding()).await?;
                load(guard.connection(), self.binding()).await?;
                set_phase(guard.connection(), self.binding(), "retired_closed").await
            })
            .await
    }
}

pub async fn allocation_id(pool: &PgPool) -> Result<String, Error> {
    let mut db = pool.acquire().await?;
    allocation(&mut db).await
}

async fn allocation(db: &mut PgConnection) -> Result<String, Error> {
    sql_budget::refresh(db).await?;
    Ok(sqlx::query_scalar(
        "SELECT 'pg:'||p.system_identifier::text||':'||db.oid::text
         FROM pg_control_system() p CROSS JOIN pg_database db WHERE db.datname=current_database()",
    )
    .fetch_one(db)
    .await?)
}

async fn ensure_allocation(db: &mut PgConnection, binding: &Binding) -> Result<(), Error> {
    if allocation(db).await? != binding.target_db_id {
        return Err(Error::Scope);
    }
    Ok(())
}

pub(super) struct LocalRow {
    pub phase: LocalPhase,
    pub reconciliation: Option<[u8; 32]>,
    pub authorization: Option<[u8; 32]>,
    pub final_prefix: Option<i64>,
    pub proofs_sha256: Option<[u8; 32]>,
    pub current_control_sha256: Option<[u8; 32]>,
}

pub(super) async fn load(db: &mut PgConnection, b: &Binding) -> Result<LocalRow, Error> {
    sql_budget::refresh(db).await?;
    let row = keyed!(
        concat!(
            "SELECT phase,target_db_id,high_water_sha256,fence_sha256,
            reconciliation_sha256,authorization_sha256,final_prefix,proofs_sha256,current_control_sha256 FROM federation_restore_local WHERE ",
            where_key!(),
            " AND generation=$9 FOR SHARE"
        ),
        b.key
    )
    .bind(b.generation as i64)
    .fetch_optional(db)
    .await?
    .ok_or(Error::Incomplete)?;
    if row.try_get::<&str, _>("target_db_id")? != b.target_db_id
        || read_digest(&row, "high_water_sha256")? != b.high_water_sha256
        || read_digest(&row, "fence_sha256")? != b.fence_sha256
    {
        return Err(Error::Conflict);
    }
    let phase = match row.try_get::<&str, _>("phase")? {
        "restore_closed" => LocalPhase::RestoreClosed,
        "reconciled_closed" => LocalPhase::ReconciledClosed,
        "open" => LocalPhase::Open,
        "retired_closed" => LocalPhase::RetiredClosed,
        _ => return Err(Error::Incomplete),
    };
    let optional = |field: &str| -> Result<Option<[u8; 32]>, Error> {
        let value: Option<&[u8]> = row.try_get(field)?;
        value
            .map(|digest| digest.try_into().map_err(|_| Error::Incomplete))
            .transpose()
    };
    Ok(LocalRow {
        phase,
        reconciliation: optional("reconciliation_sha256")?,
        authorization: optional("authorization_sha256")?,
        final_prefix: row.try_get("final_prefix")?,
        proofs_sha256: optional("proofs_sha256")?,
        current_control_sha256: optional("current_control_sha256")?,
    })
}

async fn ensure_row(db: &mut PgConnection, b: &Binding) -> Result<(), Error> {
    sql_budget::refresh(db).await?;
    let query = format!(
        "INSERT INTO federation_restore_local ({KEY_SQL},generation,target_db_id,
        high_water_sha256,fence_sha256,phase)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'restore_closed') ON CONFLICT DO NOTHING"
    );
    keyed!(sqlx::AssertSqlSafe(query), b.key)
        .bind(b.generation as i64)
        .bind(&b.target_db_id)
        .bind(&b.high_water_sha256[..])
        .bind(&b.fence_sha256[..])
        .execute(db)
        .await?;
    Ok(())
}

async fn set_phase(db: &mut PgConnection, b: &Binding, phase: &str) -> Result<(), Error> {
    sql_budget::refresh(db).await?;
    keyed!(
        concat!(
            "UPDATE federation_restore_local SET phase=$10 WHERE ",
            where_key!(),
            " AND generation=$9"
        ),
        b.key
    )
    .bind(b.generation as i64)
    .bind(phase)
    .execute(db)
    .await?;
    Ok(())
}

pub(super) async fn record_reconciliation(
    db: &mut PgConnection,
    b: &Binding,
    r: &godmesh_federation::restore_evidence::Reconciliation,
) -> Result<(), Error> {
    sql_budget::refresh(db).await?;
    let digest = r.digest()?;
    let set = keyed!(
        concat!(
            "UPDATE federation_restore_local SET phase='reconciled_closed',
         reconciliation_sha256=$10, final_prefix=$11, proofs_sha256=$12,current_control_sha256=$13
         WHERE ",
            where_key!(),
            " AND generation=$9 AND phase='restore_closed'"
        ),
        b.key
    )
    .bind(b.generation as i64)
    .bind(&digest[..])
    .bind(r.final_prefix as i64)
    .bind(&r.proofs_sha256[..])
    .bind(&r.current_control_sha256[..])
    .execute(db)
    .await?;
    if set.rows_affected() != 1 {
        return Err(Error::Conflict);
    }
    Ok(())
}

pub(super) async fn prepare_authorization(
    db: &mut PgConnection,
    b: &Binding,
    auth: [u8; 32],
) -> Result<(), Error> {
    sql_budget::refresh(db).await?;
    let set = keyed!(
        concat!(
            "UPDATE federation_restore_local SET authorization_sha256=$10 WHERE ",
            where_key!(),
            " AND generation=$9 AND phase='reconciled_closed'
          AND (authorization_sha256 IS NULL OR authorization_sha256=$10)"
        ),
        b.key
    )
    .bind(b.generation as i64)
    .bind(&auth[..])
    .execute(db)
    .await?;
    if set.rows_affected() != 1 {
        return Err(Error::Conflict);
    }
    Ok(())
}

pub(super) async fn record_open(
    db: &mut PgConnection,
    b: &Binding,
    auth: [u8; 32],
) -> Result<(), Error> {
    sql_budget::refresh(db).await?;
    let set = keyed!(
        concat!(
            "UPDATE federation_restore_local SET phase='open',authorization_sha256=$10
        WHERE ",
            where_key!(),
            " AND generation=$9 AND phase='reconciled_closed' AND authorization_sha256=$10"
        ),
        b.key
    )
    .bind(b.generation as i64)
    .bind(&auth[..])
    .execute(db)
    .await?;
    if set.rows_affected() != 1 {
        return Err(Error::Conflict);
    }
    Ok(())
}
