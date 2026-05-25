//! P081 Phase 4: MCP command idempotency repository.
//!
//! Stores one record per idempotency_key so state-changing MCP tool retries
//! return the original committed result without duplicating command_journal writes.
//! See: proposal P081 architecture.mcp_idempotency_contract.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

/// Retention window: 7 days in milliseconds.
const RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct McpCommandIdempotencyRecord {
    pub idempotency_key: String,
    pub tool_name: String,
    pub caller_fingerprint: String,
    /// sha256 of canonical(tool_name, normalized_args, caller_class, principal_id, token_id).
    pub canonical_request_hash: String,
    pub row_id: Option<String>,
    pub command_journal_id: Option<String>,
    /// JSON-serialized result returned on the original successful call.
    pub result_json: String,
    pub result_hash: Option<String>,
    pub committed_at_ms: i64,
    pub expires_at_ms: i64,
}

/// Look up an existing idempotency record by key.
/// Returns `None` if no record exists (first attempt).
pub async fn find_by_key(
    pool: &SqlitePool,
    idempotency_key: &str,
) -> Result<Option<McpCommandIdempotencyRecord>> {
    let row = sqlx::query(
        r#"SELECT idempotency_key, tool_name, caller_fingerprint, canonical_request_hash,
                  row_id, command_journal_id, result_json, result_hash,
                  committed_at_ms, expires_at_ms
           FROM mcp_command_idempotency
           WHERE idempotency_key = ?1"#,
    )
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .context("mcp_command_idempotency: find_by_key")?;

    Ok(row.map(|r| McpCommandIdempotencyRecord {
        idempotency_key: r.get("idempotency_key"),
        tool_name: r.get("tool_name"),
        caller_fingerprint: r.get("caller_fingerprint"),
        canonical_request_hash: r.get("canonical_request_hash"),
        row_id: r.get("row_id"),
        command_journal_id: r.get("command_journal_id"),
        result_json: r.get("result_json"),
        result_hash: r.get("result_hash"),
        committed_at_ms: r.get("committed_at_ms"),
        expires_at_ms: r.get("expires_at_ms"),
    }))
}

/// Sentinel result_json used for pending (pre-claim) records.
/// A record with this value was claimed before dispatch but has not yet
/// received a committed result (either in-flight or committed-unack).
pub const PENDING_SENTINEL: &str = r#"{"_pending":true}"#;

/// Insert a pending claim record BEFORE command dispatch.
/// Uses INSERT OR FAIL so concurrent races surface as a unique constraint error.
/// Returns Ok(true) if the claim was won, Ok(false) if a unique constraint
/// violation indicates another request already holds this key.
/// `boundary_row_id` is the matrix row_id from BoundaryPolicy::evaluate Allow decision.
pub async fn insert_pending(
    pool: &SqlitePool,
    idempotency_key: &str,
    tool_name: &str,
    caller_fingerprint: &str,
    canonical_request_hash: &str,
    boundary_row_id: Option<&str>,
) -> Result<bool> {
    let now_ms = Utc::now().timestamp_millis();
    let now_str = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT OR FAIL INTO mcp_command_idempotency
               (idempotency_key, tool_name, caller_fingerprint, canonical_request_hash,
                row_id, command_journal_id, result_json, result_hash,
                committed_at_ms, expires_at_ms, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, ?7, ?8, ?9)"#,
    )
    .bind(idempotency_key)
    .bind(tool_name)
    .bind(caller_fingerprint)
    .bind(canonical_request_hash)
    .bind(boundary_row_id)
    .bind(PENDING_SENTINEL)
    .bind(now_ms)
    .bind(now_ms + RETENTION_MS)
    .bind(&now_str)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e).context("mcp_command_idempotency: insert_pending"),
    }
}

/// Transactional pending claim used by command write units.
///
/// The claim must be inserted in the same BEGIN IMMEDIATE transaction as the
/// command_journal row and durable domain mutation. A unique-key conflict means
/// another request has already claimed or committed this idempotency key; callers
/// must roll back their command transaction and return a retry/replay response
/// without writing business state.
pub async fn insert_pending_tx(
    tx: &mut Transaction<'_, Sqlite>,
    idempotency_key: &str,
    tool_name: &str,
    caller_fingerprint: &str,
    canonical_request_hash: &str,
    boundary_row_id: Option<&str>,
) -> Result<bool> {
    let now_ms = Utc::now().timestamp_millis();
    let now_str = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT OR FAIL INTO mcp_command_idempotency
               (idempotency_key, tool_name, caller_fingerprint, canonical_request_hash,
                row_id, command_journal_id, result_json, result_hash,
                committed_at_ms, expires_at_ms, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, ?7, ?8, ?9)"#,
    )
    .bind(idempotency_key)
    .bind(tool_name)
    .bind(caller_fingerprint)
    .bind(canonical_request_hash)
    .bind(boundary_row_id)
    .bind(PENDING_SENTINEL)
    .bind(now_ms)
    .bind(now_ms + RETENTION_MS)
    .bind(&now_str)
    .execute(&mut **tx)
    .await;

    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e).context("mcp_command_idempotency: insert_pending_tx"),
    }
}

/// Claim an MCP idempotency key inside an existing command write unit.
///
/// Direct MCP tool write units that do not pass through `CommandHandler` still
/// need the same pending-row claim before they record command_journal and apply
/// durable mutations. If no key is present this is a no-op for legacy/direct
/// test callers that bypass the MCP transport.
pub async fn claim_pending_for_command_tx(
    tx: &mut Transaction<'_, Sqlite>,
    idempotency_key: Option<&str>,
    tool_name: &str,
    caller_fingerprint: &str,
    canonical_request_hash: Option<&str>,
    boundary_row_id: Option<&str>,
) -> Result<()> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(());
    };
    let request_hash = canonical_request_hash
        .ok_or_else(|| anyhow::anyhow!("MCP idempotency request hash missing for write unit"))?;
    let claimed = insert_pending_tx(
        tx,
        idempotency_key,
        tool_name,
        caller_fingerprint,
        request_hash,
        boundary_row_id,
    )
    .await?;
    if !claimed {
        anyhow::bail!("IDEMPOTENCY_IN_FLIGHT: idempotency key already claimed or committed");
    }
    Ok(())
}

/// Update a pending claim record with the committed result.
/// Only updates records whose result_json is still the pending sentinel
/// to avoid overwriting a racing concurrent update.
/// Returns Ok(true) if the update succeeded (this request owns the result),
/// Ok(false) if no pending record was found (lost the race or already updated).
pub async fn update_result(
    pool: &SqlitePool,
    idempotency_key: &str,
    result_json: &str,
    result_hash: Option<&str>,
    command_journal_id: Option<&str>,
) -> Result<bool> {
    let now_ms = Utc::now().timestamp_millis();
    let rows = sqlx::query(
        r#"UPDATE mcp_command_idempotency
           SET result_json = ?1, result_hash = ?2, command_journal_id = ?3, committed_at_ms = ?4
           WHERE idempotency_key = ?5 AND result_json = ?6"#,
    )
    .bind(result_json)
    .bind(result_hash)
    .bind(command_journal_id)
    .bind(now_ms)
    .bind(idempotency_key)
    .bind(PENDING_SENTINEL)
    .execute(pool)
    .await
    .context("mcp_command_idempotency: update_result")?;
    Ok(rows.rows_affected() > 0)
}

/// Delete a pending claim record when the dispatch failed.
/// Only deletes records still in the pending state to avoid removing a
/// record that another process has already committed.
pub async fn delete_pending(pool: &SqlitePool, idempotency_key: &str) -> Result<()> {
    sqlx::query(
        r#"DELETE FROM mcp_command_idempotency
           WHERE idempotency_key = ?1 AND result_json = ?2"#,
    )
    .bind(idempotency_key)
    .bind(PENDING_SENTINEL)
    .execute(pool)
    .await
    .context("mcp_command_idempotency: delete_pending")?;
    Ok(())
}

/// Insert an idempotency record directly (used when the result is already known at insert time).
/// Uses INSERT OR IGNORE for backward compatibility with callers that handle conflict detection
/// before calling this function.
pub async fn insert(pool: &SqlitePool, record: &McpCommandIdempotencyRecord) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT OR IGNORE INTO mcp_command_idempotency
               (idempotency_key, tool_name, caller_fingerprint, canonical_request_hash,
                row_id, command_journal_id, result_json, result_hash,
                committed_at_ms, expires_at_ms, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(&record.idempotency_key)
    .bind(&record.tool_name)
    .bind(&record.caller_fingerprint)
    .bind(&record.canonical_request_hash)
    .bind(&record.row_id)
    .bind(&record.command_journal_id)
    .bind(&record.result_json)
    .bind(&record.result_hash)
    .bind(record.committed_at_ms)
    .bind(record.expires_at_ms)
    .bind(&now)
    .execute(pool)
    .await
    .context("mcp_command_idempotency: insert")?;
    Ok(())
}

/// Build a new record for insertion. `committed_at_ms` is set to now; `expires_at_ms`
/// is set 7 days out.
pub fn build_record(
    idempotency_key: &str,
    tool_name: &str,
    caller_fingerprint: &str,
    canonical_request_hash: &str,
    row_id: Option<&str>,
    command_journal_id: Option<&str>,
    result_json: &str,
    result_hash: Option<&str>,
) -> McpCommandIdempotencyRecord {
    let now_ms = Utc::now().timestamp_millis();
    McpCommandIdempotencyRecord {
        idempotency_key: idempotency_key.to_string(),
        tool_name: tool_name.to_string(),
        caller_fingerprint: caller_fingerprint.to_string(),
        canonical_request_hash: canonical_request_hash.to_string(),
        row_id: row_id.map(|s| s.to_string()),
        command_journal_id: command_journal_id.map(|s| s.to_string()),
        result_json: result_json.to_string(),
        result_hash: result_hash.map(|s| s.to_string()),
        committed_at_ms: now_ms,
        expires_at_ms: now_ms + RETENTION_MS,
    }
}
