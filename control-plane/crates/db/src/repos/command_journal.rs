use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};

/// Record a command invocation in the command journal.
///
/// The journal is a write-once audit trail; entries are never mutated after
/// the initial insert — they are closed by `complete_entry` or `fail_entry`.
///
/// `request_id` is the P042 §9.3 correlation id attached to the inbound
/// request by the `X-Request-ID` middleware. `None` for MCP-stdio mode
/// (which has no HTTP envelope) and for legacy callers that bypass the
/// middleware; all HTTP-born commands carry a populated value.
pub async fn record(
    pool: &SqlitePool,
    id: &str,
    command_type: &str,
    payload_json: &str,
    run_id: Option<&str>,
    created_at: DateTime<Utc>,
    caller_surface: Option<&str>,
    caller_principal_id: Option<&str>,
    caller_principal_class: Option<&str>,
    caller_tool: Option<&str>,
    request_id: Option<&str>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "command_journal.record",
        sqlx::query(
        r#"INSERT INTO command_journal (id, command_type, payload_json, result_status, run_id, created_at, caller_surface, caller_principal_id, caller_principal_class, caller_tool, request_id)
           VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
        .bind(id)
        .bind(command_type)
        .bind(payload_json)
        .bind(run_id)
        .bind(created_at.to_rfc3339())
        .bind(caller_surface)
        .bind(caller_principal_id)
        .bind(caller_principal_class)
        .bind(caller_tool)
        .bind(request_id)
    )
    .context("record command journal entry")?;
    Ok(())
}

pub async fn record_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    command_type: &str,
    payload_json: &str,
    run_id: Option<&str>,
    created_at: DateTime<Utc>,
    caller_surface: Option<&str>,
    caller_principal_id: Option<&str>,
    caller_principal_class: Option<&str>,
    caller_tool: Option<&str>,
    request_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO command_journal (id, command_type, payload_json, result_status, run_id, created_at, caller_surface, caller_principal_id, caller_principal_class, caller_tool, request_id)
           VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(id)
    .bind(command_type)
    .bind(payload_json)
    .bind(run_id)
    .bind(created_at.to_rfc3339())
    .bind(caller_surface)
    .bind(caller_principal_id)
    .bind(caller_principal_class)
    .bind(caller_tool)
    .bind(request_id)
    .execute(&mut **tx)
    .await
    .context("record command journal entry")?;
    Ok(())
}

/// Look up a journal entry's `request_id` (§9.3 cross-surface
/// correlation test helper).
pub async fn find_request_id(pool: &SqlitePool, id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT request_id FROM command_journal WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("find journal request_id")?;
    Ok(row.and_then(|(request_id,)| request_id))
}

/// Mark a journal entry as successfully completed.
pub async fn complete_entry(
    pool: &SqlitePool,
    id: &str,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "command_journal.complete_entry",
        sqlx::query(
        "UPDATE command_journal SET result_status = 'completed', completed_at = ?1 WHERE id = ?2",
    )
        .bind(completed_at.to_rfc3339())
        .bind(id)
    )
    .context("complete command journal entry")?;
    Ok(())
}

pub async fn complete_entry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE command_journal SET result_status = 'completed', completed_at = ?1 WHERE id = ?2",
    )
    .bind(completed_at.to_rfc3339())
    .bind(id)
    .execute(&mut **tx)
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
    crate::execute_repository_write!(
        pool,
        "command_journal.fail_entry",
        sqlx::query(
        "UPDATE command_journal SET result_status = 'failed', completed_at = ?1, error = ?2 WHERE id = ?3",
    )
        .bind(completed_at.to_rfc3339())
        .bind(error)
        .bind(id)
    )
    .context("fail command journal entry")?;
    Ok(())
}

pub async fn fail_entry_tx(
    tx: &mut Transaction<'_, Sqlite>,
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
    .execute(&mut **tx)
    .await
    .context("fail command journal entry")?;
    Ok(())
}
