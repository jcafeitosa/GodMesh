use std::{sync::Arc, time::Duration};

use godmesh_federation::{
    node::{LinkAuthorizationError, LinkIdentity},
    now_unix,
};
use sqlx::{Connection, Postgres, Transaction};
use tokio::{
    sync::oneshot,
    time::{Instant, timeout, timeout_at},
};

use crate::{
    config_bundle::{BundleService, Fence},
    portable_join::PortableJoin,
};

use super::{EffectError, db::Lease, with_sql_deadline};

/// Autoriza o enlace mantendo o lock M1C na transação do chamador.
pub async fn authorize_link_in_tx(
    portable: &PortableJoin,
    m3: Option<&BundleService>,
    local: &LinkIdentity,
    remote: &LinkIdentity,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<i64>, LinkAuthorizationError> {
    if portable.cluster() != local.cluster
        || local.cluster != remote.cluster
        || portable.identity().node_id() != local.node_id
        || portable.identity().public_key_bytes() != local.public_key
    {
        return Err(LinkAuthorizationError::Denied);
    }
    let mut guard = portable
        .lock_cluster_guard(tx)
        .await
        .map_err(|error| link_error(error.into()))?;
    let now = now_unix();
    let local_expiry = portable
        .authorize_local_at(&mut guard, now)
        .await
        .map_err(|error| link_error(error.into()))?;
    let remote_expiry = portable
        .authorize_peer_at(
            &mut guard,
            &remote.node_id,
            &hex::encode(remote.public_key),
            now,
        )
        .await
        .map_err(|error| link_error(error.into()))?;
    let m3_expiry = if let Some(m3) = m3 {
        // O transporte validou o ALPN godmesh/1 antes do callback.
        match m3
            .classify_fence_in_tx(&mut guard, 1, now)
            .await
            .map_err(|error| link_error(error.into()))?
        {
            Fence::Allowed { recheck_at, .. } => Some(recheck_at),
            Fence::Disabled | Fence::Blocked(_) => return Err(LinkAuthorizationError::Denied),
        }
    } else {
        None
    };
    Ok([local_expiry, remote_expiry, m3_expiry]
        .into_iter()
        .flatten()
        .min())
}

fn link_error(error: EffectError) -> LinkAuthorizationError {
    match error {
        EffectError::Timeout => LinkAuthorizationError::Timeout,
        EffectError::M1c(_) => LinkAuthorizationError::Denied,
        _ => LinkAuthorizationError::Unavailable,
    }
}

type Decision = Result<(Box<dyn Send>, Option<i64>), LinkAuthorizationError>;

/// Autoriza sob o prazo da tentativa. O token retornado mantém o lock até seu
/// descarte. Se o chamador abandonar a consulta, a tarefa abandona a conexão
/// não sincronizada. O timeout local do PostgreSQL também limita a consulta.
pub async fn authorize_link_until(
    portable: Arc<PortableJoin>,
    m3: Option<Arc<BundleService>>,
    local: &LinkIdentity,
    remote: &LinkIdentity,
    deadline: Instant,
) -> Result<(Box<dyn Send>, Option<i64>), LinkAuthorizationError> {
    let (send, receive) = oneshot::channel();
    tokio::spawn(hold_link(
        portable,
        m3,
        local.clone(),
        remote.clone(),
        deadline,
        send,
    ));
    receive
        .await
        .unwrap_or(Err(LinkAuthorizationError::Unavailable))
}

async fn hold_link(
    portable: Arc<PortableJoin>,
    m3: Option<Arc<BundleService>>,
    local: LinkIdentity,
    remote: LinkIdentity,
    deadline: Instant,
    mut send: oneshot::Sender<Decision>,
) {
    let mut lease = tokio::select! {
        biased;
        () = send.closed() => return,
        result = Lease::acquire(portable.pool(), deadline) => match result {
            Ok(lease) => lease,
            Err(error) => { let _ = send.send(Err(link_error(error))); return; }
        }
    };
    let mut tx = tokio::select! {
        biased;
        () = send.closed() => return,
        result = timeout_at(deadline, lease.begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")) => match result {
            Ok(Ok(tx)) => tx,
            Ok(Err(error)) => { let _ = send.send(Err(link_error(error.into()))); return; }
            Err(_) => { let _ = send.send(Err(LinkAuthorizationError::Timeout)); return; }
        }
    };
    let authorized = tokio::select! {
        biased;
        () = send.closed() => return,
        result = timeout_at(deadline, with_sql_deadline(
            deadline, authorize_link_in_tx(&portable,m3.as_deref(),&local,&remote,&mut tx),
        )) => result.unwrap_or(Err(LinkAuthorizationError::Timeout)),
    };
    match authorized {
        Ok(expires_at) if Instant::now() < deadline => {
            let (release, released) = oneshot::channel::<()>();
            if send.send(Ok((Box::new(release), expires_at))).is_ok() {
                let _ = released.await;
            }
        }
        Ok(_) | Err(LinkAuthorizationError::Timeout) => {
            let _ = send.send(Err(LinkAuthorizationError::Timeout));
        }
        Err(error) => {
            let _ = send.send(Err(error));
        }
    }
    if matches!(
        timeout(Duration::from_secs(1), tx.rollback()).await,
        Ok(Ok(()))
    ) {
        lease.clean();
    }
}
