use sqlx::PgConnection;

use crate::{replication::Series, sql_budget};

pub(super) async fn pending(
    db: &mut PgConnection,
    local: &str,
    series: Series<'_>,
) -> Result<Option<i64>, sqlx::Error> {
    let ns = series.namespace;
    sql_budget::refresh(db).await?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT attempt_id,state FROM federation_snapshot_intent_marker
         WHERE (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5,$6)
         FOR SHARE")
        .bind(local).bind(&ns.origin_node).bind(&ns.tenant_id).bind(&ns.stream_id)
        .bind(ns.partition).bind(series.epoch).fetch_optional(db).await?;
    match row {
        Some((id, state)) if id > 0 && state == "pending" => Ok(Some(id)),
        Some(_) => Err(sqlx::Error::Protocol(
            "marcador de snapshot inconsistente".into(),
        )),
        None => Ok(None),
    }
}
