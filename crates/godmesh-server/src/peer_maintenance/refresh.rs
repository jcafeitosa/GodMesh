use std::{io, time::Duration};

use godmesh_federation::peer::PeerRecord;
use sqlx::{Connection, PgPool};
use tokio::{
    sync::watch,
    time::{Instant, timeout, timeout_at},
};

use crate::{api::ApiError, federation, replica_operational::with_sql_deadline, sql_budget};

pub(super) async fn read(
    pool: &PgPool,
    mut stop: watch::Receiver<bool>,
) -> Result<Vec<PeerRecord>, ApiError> {
    // Reusa o orçamento de aquisição do pool para limitar a espera no banco.
    // Os timeouts SQL que já forem menores permanecem vigentes.
    let deadline = Instant::now() + pool.options().get_acquire_timeout();
    let mut connection = tokio::select! {
        biased;
        _ = stop.wait_for(|stop| *stop) => return Err(stopped()),
        result = timeout_at(deadline, pool.acquire()) =>
            result.map_err(|_| sqlx::Error::PoolTimedOut)??,
    };
    let result = tokio::select! {
        biased;
        _ = stop.wait_for(|stop| *stop) => Err(stopped()),
        result = timeout_at(deadline, with_sql_deadline(deadline, async {
            let mut tx = connection.begin_with("BEGIN READ ONLY").await?;
            sql_budget::refresh(&mut tx).await?;
            let records = federation::list_peers_on(&mut tx).await?;
            tx.rollback().await?;
            Ok(records)
        })) => result.unwrap_or_else(|_| Err(sqlx::Error::PoolTimedOut.into())),
    };
    if result.is_err() {
        // Uma consulta abandonada pode ter respostas ainda em trânsito e um
        // rollback enfileirado. Fechar a sessão, sem devolvê-la ao pool.
        let _ = timeout(Duration::from_secs(1), connection.close()).await;
    }
    result
}

fn stopped() -> ApiError {
    sqlx::Error::Io(io::Error::new(
        io::ErrorKind::Interrupted,
        "refresh de pares cancelado",
    ))
    .into()
}
