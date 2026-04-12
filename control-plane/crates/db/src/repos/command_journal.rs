use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// Record a command invocation in the command journal.
///
/// The journal is a write-once audit trail; entries are never mutated after
/// the initial insert — they are closed by `complete_entry` or `fail_entry`.
pub async fn record(
    pool: &SqlitePool,
    id: &str,
    command_type: &str,
    payload_json: &str,
    run_id: Option<&str>,
    created_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO command_journal (id, command_type, payload_json, result_status, run_id, created_at)
           VALUES (?1, ?2, ?3, 'pending', ?4, ?5)"#,
    )
    .bind(id)
    .bind(command_type)
    .bind(payload_json)
    .bind(run_id)
    .bind(created_at.to_rfc3339())
    .execute(pool)
    .await
    .context("record command journal entry")?;
    Ok(())
}

/// Mark a journal entry as successfully completed.
pub async fn complete_entry(pool: &SqlitePool, id: &str, completed_at: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        "UPDATE command_journal SET result_status = 'completed', completed_at = ?1 WHERE id = ?2",
    )
    .bind(completed_at.to_rfc3339())
    .bind(id)
    .execute(pool)
    .await
    .context("complete command journal entry")?;
    Ok(())
}

/// Mark a journal entry as failed with an error message.
pub async fn fail_entry(
    pool: &SqlitePool,
    id: &str,
    completed_at: DateTime<Utc>,
    error: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE command_journal SET result_status = 'failed', completed_at = ?1, error = ?2 WHERE id = ?3",
    )
    .bind(completed_at.to_rfc3339())
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("fail command journal entry")?;
    Ok(())
}
