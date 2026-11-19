//! Orçamento de instruções dentro de uma transação M4.
//! O escopo acompanha o future da decisão e não se propaga a tarefas novas.
use sqlx::PgConnection;
use tokio::time::Instant;

tokio::task_local! {
    pub(crate) static DEADLINE: Instant;
}

pub(crate) async fn refresh(db: &mut PgConnection) -> Result<(), sqlx::Error> {
    let Ok(deadline) = DEADLINE.try_with(|deadline| *deadline) else {
        return Ok(());
    };
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "orçamento PostgreSQL esgotado",
            )
        })?;
    let ms = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
    // Zero é ilimitado no PostgreSQL. Mantém o menor teto já vigente e
    // limita somente esta transação, inclusive se ela precisar de rollback.
    sqlx::query(
        "SELECT set_config(name, LEAST(NULLIF(setting::integer, 0), $1)::text, true)
         FROM pg_settings WHERE name IN ('lock_timeout', 'statement_timeout')",
    )
    .bind(ms)
    .execute(db)
    .await?;
    Ok(())
}
