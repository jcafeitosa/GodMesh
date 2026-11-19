//! Leitores integrais exclusivos do ensaio de snapshot.
use super::*;

pub(crate) async fn scope(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
) -> Result<(String, String, i64), Error> {
    let db = locked(guard, series.namespace).await?;
    let r = load_series(db, series).await?;
    let prefix = store::checkpoint(db, &r)
        .await?
        .ok_or(Error::CheckpointMissing)?;
    Ok((r.retention_class, r.jurisdiction, prefix))
}

pub(crate) async fn select_closed(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
    authority: CopyAuthority<'_>,
    base: i64,
    cut: i64,
    now: i64,
) -> Result<Vec<Vec<u8>>, Error> {
    let first = base
        .checked_add(1)
        .filter(|seq| *seq > 0 && *seq <= cut)
        .ok_or(Error::SequenceGap)?;
    let db = locked(guard, series.namespace).await?;
    let r = load_series(db, series).await?;
    authority.origin.check(&r.namespace.origin_node)?;
    authority.destination.check(authority.destination.node_id)?;
    if !r.primary() {
        return Err(Error::Authority);
    }
    let primary = store::current_policy(db, &r).await?;
    let secondary = store::copy_policy(db, &r, Some(authority.destination.node_id), now).await?;
    let mut frames = Vec::new();
    let mut remaining = authority.limits.max_data_bytes;
    for seq in first..=cut {
        let proof = usable(db, &r, &primary, seq, now).await?;
        verify_local_proof(&proof, &r, authority.origin, seq, true)?;
        secondary
            .expiration(proof.issued_at, now)
            .map_err(|error| match error {
                Error::RetentionExpired => Error::HistoryUnavailable,
                other => other,
            })?;
        if frames.len() >= authority.limits.max_events
            || proof.frame.len() > authority.limits.max_envelope_bytes
        {
            return Err(Error::LimitExceeded);
        }
        remaining = remaining
            .checked_sub(4)
            .and_then(|n| n.checked_sub(proof.frame.len()))
            .ok_or(Error::LimitExceeded)?;
        frames.push(proof.frame);
    }
    Ok(frames)
}

pub(crate) async fn exact_local_expiry(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
    origin: NodeContext<'_>,
    base: i64,
    events: &[&[u8]],
    now: i64,
) -> Result<i64, Error> {
    let db = locked(guard, series.namespace).await?;
    let r = load_series(db, series).await?;
    if r.primary() {
        return Err(Error::Authority);
    }
    let policy = store::current_policy(db, &r).await?;
    let mut seq = base;
    let mut expiry = None;
    for raw in events {
        seq = next(seq)?;
        let proof = usable(db, &r, &policy, seq, now).await?;
        verify_local_proof(&proof, &r, origin, seq, false)?;
        if proof.frame != *raw {
            return Err(Error::ProofConflict);
        }
        expiry = Some(expiry.map_or(proof.expires_at, |prior: i64| prior.min(proof.expires_at)));
    }
    expiry.ok_or(Error::SequenceGap)
}

pub(crate) async fn range_unused(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
    base: i64,
    cut: i64,
) -> Result<bool, Error> {
    let ns = series.namespace;
    let db = locked(guard, ns).await?;
    crate::sql_budget::refresh(db).await?;
    Ok(sqlx::query_scalar(
        "SELECT NOT EXISTS (
           SELECT 1 FROM event_position WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5)
             AND seq>$6 AND seq<=$7
           UNION ALL
           SELECT 1 FROM federation_replica_proof WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5)
             AND seq>$6 AND seq<=$7
           UNION ALL
           SELECT 1 FROM federation_replica_disposition WHERE (origin_node,tenant_id,stream_id,partition,authority_epoch)=($1,$2,$3,$4,$5)
             AND seq>$6 AND seq<=$7
         )",
    ).bind(&ns.origin_node).bind(&ns.tenant_id).bind(&ns.stream_id).bind(ns.partition)
      .bind(series.epoch).bind(base).bind(cut).fetch_one(db).await?)
}

pub(crate) async fn local_snapshot_proofs(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
    authority: CopyAuthority<'_>,
    base: i64,
    cut: i64,
    now: i64,
) -> Result<(Vec<Vec<u8>>, i64), Error> {
    let mut ttl = i64::MAX;
    let mut remaining = authority.limits.max_data_bytes;
    let mut frames = Vec::new();
    visit_local_proofs(
        guard,
        series,
        authority,
        base,
        cut,
        now,
        |frame, expires_at| {
            if frames.len() >= authority.limits.max_events {
                return Err(Error::LimitExceeded);
            }
            remaining = remaining
                .checked_sub(4)
                .and_then(|n| n.checked_sub(frame.len()))
                .ok_or(Error::LimitExceeded)?;
            ttl = ttl.min(expires_at);
            frames.push(frame);
            Ok(())
        },
    )
    .await?;
    Ok((frames, ttl))
}

/// Lê uma posição por vez, dentro da transação e do segundo já amostrado.
/// O visitante decide se acumula o subconjunto ou só atualiza seu compromisso.
pub(crate) async fn visit_local_proofs<E: From<Error>>(
    guard: &mut ClusterGuard<'_, '_>,
    series: Series<'_>,
    authority: CopyAuthority<'_>,
    base: i64,
    cut: i64,
    now: i64,
    mut visit: impl FnMut(Vec<u8>, i64) -> Result<(), E>,
) -> Result<(), E> {
    let first = base
        .checked_add(1)
        .filter(|n| *n > 0 && *n <= cut)
        .ok_or(Error::SequenceGap)?;
    let db = locked(guard, series.namespace).await?;
    let r = load_series(db, series).await?;
    if r.primary() {
        return Err(Error::Authority.into());
    }
    let policy = store::current_policy(db, &r).await?;
    for seq in first..=cut {
        let proof = usable(db, &r, &policy, seq, now).await?;
        verify_local_proof(&proof, &r, authority.origin, seq, false)?;
        if proof.frame.len() > authority.limits.max_envelope_bytes {
            return Err(Error::LimitExceeded.into());
        }
        visit(proof.frame, proof.expires_at)?;
    }
    Ok(())
}
