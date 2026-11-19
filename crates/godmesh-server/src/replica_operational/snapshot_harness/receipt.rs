use crate::replication::Series;
use sqlx::{PgConnection, Row, postgres::PgRow};

use super::SnapshotEffectError;

pub(super) struct Policy {
    pub id: String,
    pub version: i64,
    pub seconds: i64,
}

#[derive(PartialEq, Eq)]
pub(super) struct Identity {
    pub base: i64,
    pub count: i64,
    pub manifest: [u8; 32],
    pub package: [u8; 32],
}

pub(super) struct Receipt {
    pub identity: Identity,
    pub policy_id: String,
    pub policy_version: i64,
    pub applied_at: i64,
    pub proof_expires_at: i64,
    pub expires_at: i64,
    pub jurisdiction: String,
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
    historical: Option<&Receipt>,
) -> Result<Policy, SnapshotEffectError> {
    crate::sql_budget::refresh(db).await?;
    let row = scope!(
        "SELECT policy_id,policy_version,duration_seconds FROM federation_snapshot_receipt_policy
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5,$6)
           AND jurisdiction=$7 AND purpose='snapshot_receipt'
           AND (($8::text IS NULL AND active) OR (policy_id,policy_version)=($8,$9)) FOR SHARE",
        local, series
    ).bind(region).bind(historical.map(|r| r.policy_id.as_str())).bind(historical.map(|r| r.policy_version))
     .fetch_optional(db).await?.ok_or(SnapshotEffectError::ReceiptPolicy)?;
    Ok(Policy {
        id: row.try_get("policy_id")?,
        version: row.try_get("policy_version")?,
        seconds: row.try_get("duration_seconds")?,
    })
}

fn digest(row: &PgRow, name: &str) -> Result<[u8; 32], SnapshotEffectError> {
    let value: &[u8] = row.try_get(name)?;
    value
        .try_into()
        .map_err(|_| SnapshotEffectError::ReceiptInconsistent)
}

pub(super) async fn load(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    cut: i64,
) -> Result<Option<Receipt>, SnapshotEffectError> {
    crate::sql_budget::refresh(db).await?;
    let row = scope!(
        "SELECT base_seq,event_count,manifest_digest,package_digest,policy_id,policy_version,applied_at,proof_expires_at,expires_at,jurisdiction
         FROM federation_snapshot_receipt
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,cut_seq)=($1,$2,$3,$4,$5,$6,$7)
         FOR SHARE", local, series,
    ).bind(cut).fetch_optional(db).await?;
    row.map(|r| {
        Ok(Receipt {
            identity: Identity {
                base: r.try_get("base_seq")?,
                count: r.try_get("event_count")?,
                manifest: digest(&r, "manifest_digest")?,
                package: digest(&r, "package_digest")?,
            },
            policy_id: r.try_get("policy_id")?,
            policy_version: r.try_get("policy_version")?,
            applied_at: r.try_get("applied_at")?,
            proof_expires_at: r.try_get("proof_expires_at")?,
            expires_at: r.try_get("expires_at")?,
            jurisdiction: r.try_get("jurisdiction")?,
        })
    })
    .transpose()
}

pub(super) async fn insert(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    cut: i64,
    r: &Receipt,
) -> Result<(), SnapshotEffectError> {
    crate::sql_budget::refresh(db).await?;
    scope!(
        "INSERT INTO federation_snapshot_receipt
         (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,cut_seq,
          base_seq,event_count,manifest_digest,package_digest,policy_id,policy_version,applied_at,proof_expires_at,expires_at,jurisdiction)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)", local, series,
    ).bind(cut).bind(r.identity.base).bind(r.identity.count)
     .bind(&r.identity.manifest[..]).bind(&r.identity.package[..]).bind(&r.policy_id)
     .bind(r.policy_version).bind(r.applied_at).bind(r.proof_expires_at).bind(r.expires_at).bind(&r.jurisdiction).execute(db).await?;
    Ok(())
}
