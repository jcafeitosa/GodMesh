use sqlx::PgConnection;

use crate::replication::Series;

use super::{EffectError, ReceiverState};

macro_rules! key {
    ($sql:literal, $local:expr, $series:expr) => {{
        let ns = $series.namespace;
        sqlx::query_scalar::<_, i64>($sql)
            .bind($local)
            .bind(&ns.origin_node)
            .bind(&ns.tenant_id)
            .bind(&ns.stream_id)
            .bind(ns.partition)
            .bind($series.epoch)
    }};
}

pub(super) async fn state(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    prefix: i64,
) -> Result<ReceiverState, EffectError> {
    crate::sql_budget::refresh(db).await?;
    match key!(
        "SELECT blocked_next_seq FROM federation_replica_resume_block
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5,$6)
         FOR SHARE", local, series
    ).fetch_optional(db).await? {
        None => Ok(ReceiverState::Ready { after_seq: prefix }),
        Some(next) if prefix.checked_add(1) == Some(next) => Ok(ReceiverState::SuspendedHistory { blocked_next_seq: next }),
        Some(_) => Err(EffectError::ResumeInconsistent),
    }
}

pub(super) async fn insert(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
    next: i64,
) -> Result<(), EffectError> {
    crate::sql_budget::refresh(db).await?;
    let inserted = key!(
        "INSERT INTO federation_replica_resume_block
         (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,blocked_next_seq)
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING RETURNING blocked_next_seq",
        local,
        series
    )
    .bind(next)
    .fetch_optional(db)
    .await?;
    if inserted != Some(next) {
        return Err(EffectError::ResumeInconsistent);
    }
    Ok(())
}
