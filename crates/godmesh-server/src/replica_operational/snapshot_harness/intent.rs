use super::{SnapshotEffectError, Window, receipt};
use crate::{replication::Series, sql_budget};
use sqlx::{PgConnection, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptId(i64);

impl AttemptId {
    pub(super) fn from_db(value: i64) -> Result<Self, SnapshotEffectError> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(SnapshotEffectError::IntentInconsistent)
        }
    }

    pub fn get(self) -> i64 {
        self.0
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Facts {
    pub cut: i64,
    pub receipt: receipt::Identity,
    pub events: [u8; 32],
    pub window: Window,
}

pub(super) struct Policy {
    pub id: String,
    pub version: i64,
    pub seconds: i64,
    pub allow_reduce: bool,
}

pub(super) struct Detail {
    pub facts: Facts,
    pub policy_id: String,
    pub policy_version: i64,
    pub jurisdiction: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl Detail {
    pub fn consistent(
        &self,
        policy: &Policy,
        region: &str,
        now: i64,
    ) -> Result<(), SnapshotEffectError> {
        if self.jurisdiction != region
            || self.created_at > now
            || self.created_at < self.facts.window.not_before
            || expiry(self.created_at, policy.seconds, self.facts.window)? != self.expires_at
        {
            return Err(SnapshotEffectError::IntentInconsistent);
        }
        Ok(())
    }
}

pub(super) fn expiry(now: i64, duration: i64, window: Window) -> Result<i64, SnapshotEffectError> {
    now.checked_add(duration)
        .map(|v| v.min(window.expires_at))
        .filter(|v| *v > now)
        .ok_or(SnapshotEffectError::IntentInconsistent)
}

macro_rules! scope {
    ($sql:literal, $local:expr, $series:expr $(,)?) => {{
        let ns = $series.namespace;
        sqlx::query($sql)
            .bind($local)
            .bind(&ns.origin_node)
            .bind(&ns.tenant_id)
            .bind(&ns.stream_id)
            .bind(ns.partition)
            .bind($series.epoch)
    }};
}

pub(super) async fn policy(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    region: &str,
    historical: Option<&Detail>,
) -> Result<Policy, SnapshotEffectError> {
    sql_budget::refresh(db).await?;
    let row = scope!(
        "SELECT policy_id,policy_version,duration_seconds,allow_reduce FROM federation_snapshot_intent_policy
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5,$6)
           AND jurisdiction=$7 AND purpose='snapshot_intent'
           AND (($8::text IS NULL AND active) OR (policy_id,policy_version)=($8,$9)) FOR SHARE",
        local, series,
    ).bind(region).bind(historical.map(|r| r.policy_id.as_str())).bind(historical.map(|r| r.policy_version))
        .fetch_optional(db).await?.ok_or(SnapshotEffectError::IntentPolicy)?;
    Ok(Policy {
        id: row.try_get("policy_id")?,
        version: row.try_get("policy_version")?,
        seconds: row.try_get("duration_seconds")?,
        allow_reduce: row.try_get("allow_reduce")?,
    })
}

pub(super) async fn new_marker(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
) -> Result<AttemptId, SnapshotEffectError> {
    sql_budget::refresh(db).await?;
    let row = scope!(
        "INSERT INTO federation_snapshot_intent_marker
         (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING attempt_id",
        local,
        series,
    )
    .fetch_one(db)
    .await?;
    AttemptId::from_db(row.try_get("attempt_id")?)
}

pub(super) async fn insert(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    id: AttemptId,
    detail: &Detail,
) -> Result<(), SnapshotEffectError> {
    sql_budget::refresh(db).await?;
    let f = &detail.facts;
    scope!(
        "INSERT INTO federation_snapshot_intent_detail
         (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,attempt_id,
          base_seq,cut_seq,event_count,manifest_digest,package_digest,events_digest,not_before,signed_expires_at,
          created_at,detail_expires_at,policy_id,policy_version,jurisdiction)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)", local, series,
    ).bind(id.get()).bind(f.receipt.base).bind(f.cut).bind(f.receipt.count).bind(&f.receipt.manifest[..])
     .bind(&f.receipt.package[..]).bind(&f.events[..]).bind(f.window.not_before).bind(f.window.expires_at)
     .bind(detail.created_at).bind(detail.expires_at).bind(&detail.policy_id).bind(detail.policy_version)
     .bind(&detail.jurisdiction).execute(db).await?;
    Ok(())
}

pub(super) async fn load(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    id: AttemptId,
) -> Result<Option<Detail>, SnapshotEffectError> {
    sql_budget::refresh(db).await?;
    let row = scope!(
        "SELECT base_seq,cut_seq,event_count,manifest_digest,package_digest,events_digest,not_before,signed_expires_at,
                created_at,detail_expires_at,policy_id,policy_version,jurisdiction
         FROM federation_snapshot_intent_detail
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,attempt_id)=($1,$2,$3,$4,$5,$6,$7)
         FOR SHARE", local, series,
    ).bind(id.get()).fetch_optional(db).await?;
    row.map(|r| {
        Ok(Detail {
            facts: Facts {
                cut: r.try_get("cut_seq")?,
                receipt: receipt::Identity {
                    base: r.try_get("base_seq")?,
                    count: r.try_get("event_count")?,
                    manifest: receipt::digest(&r, "manifest_digest")?,
                    package: receipt::digest(&r, "package_digest")?,
                },
                events: receipt::digest(&r, "events_digest")?,
                window: Window {
                    not_before: r.try_get("not_before")?,
                    expires_at: r.try_get("signed_expires_at")?,
                },
            },
            policy_id: r.try_get("policy_id")?,
            policy_version: r.try_get("policy_version")?,
            jurisdiction: r.try_get("jurisdiction")?,
            created_at: r.try_get("created_at")?,
            expires_at: r.try_get("detail_expires_at")?,
        })
    })
    .transpose()
}

pub(super) async fn reduce(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    id: AttemptId,
) -> Result<(), SnapshotEffectError> {
    sql_budget::refresh(db).await?;
    let result = scope!(
        "DELETE FROM federation_snapshot_intent_detail
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,attempt_id)=($1,$2,$3,$4,$5,$6,$7)",
        local, series,
    ).bind(id.get()).execute(db).await?;
    if result.rows_affected() != 1 {
        return Err(SnapshotEffectError::IntentInconsistent);
    }
    Ok(())
}

pub(super) async fn resolve(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    id: AttemptId,
) -> Result<(), SnapshotEffectError> {
    reduce(db, local, series, id).await?;
    sql_budget::refresh(db).await?;
    let result = scope!(
        "DELETE FROM federation_snapshot_intent_marker
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,attempt_id)=($1,$2,$3,$4,$5,$6,$7)",
        local, series,
    ).bind(id.get()).execute(db).await?;
    if result.rows_affected() != 1 {
        return Err(SnapshotEffectError::IntentInconsistent);
    }
    Ok(())
}
