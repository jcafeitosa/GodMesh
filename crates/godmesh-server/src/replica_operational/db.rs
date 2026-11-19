use std::ops::{Deref, DerefMut};

use sqlx::{PgConnection, PgPool, Postgres, Transaction, pool::PoolConnection};
use tokio::time::{Instant, timeout_at};

use super::EffectError;

pub(crate) struct Lease {
    connection: PoolConnection<Postgres>,
    dirty: bool,
}

impl Lease {
    pub async fn acquire(pool: &PgPool, deadline: Instant) -> Result<Self, EffectError> {
        let connection = timeout_at(deadline, pool.acquire())
            .await
            .map_err(|_| EffectError::Timeout)??;
        Ok(Self {
            connection,
            dirty: true,
        })
    }

    pub fn clean(&mut self) {
        self.dirty = false;
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.dirty {
            // An externally cancelled future, or a failed rollback/commit,
            // cannot prove that this session finished synchronizing.
            self.connection.close_on_drop();
        }
    }
}

impl Deref for Lease {
    type Target = PgConnection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for Lease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

pub(super) async fn commit(
    tx: Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), EffectError> {
    // Once polled, COMMIT can succeed even when the acknowledgment is lost.
    match timeout_at(deadline, tx.commit()).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(EffectError::CommitUnknown(Some(error))),
        Err(_) => Err(EffectError::CommitUnknown(None)),
    }
}
