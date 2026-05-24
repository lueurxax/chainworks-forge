//! P081 Phase 5: Approval mutation idempotency repository.
//!
//! Stores one record per idempotency_key so approveApproval / rejectApproval
//! retries return the original committed result without duplicating side effects.
//! See: proposal P081 architecture.approval_idempotency_contract.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

/// Retention window: 7 days in milliseconds.
const RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct ApprovalMutationIdempotencyRecord {
    pub idempotency_key: String,
    pub approval_id: String,
    pub action: String,
    pub caller_fingerprint: String,
    pub request_id: Option<String>,
    /// SEC-P081-M002: sha256 of canonical request fields (approval_id, action,
    /// caller_class, principal_id). Stored alongside the key so same-key/different-
    /// canonical-request can be detected as IDEMPOTENCY_CONFLICT without side effects.
    pub request_hash: Option<String>,
    pub command_journal_id: String,
    pub result_hash: Option<String>,
    pub committed_at_ms: i64,
    pub expires_at_ms: i64,
}

/// Look up an existing idempotency record by key.
/// Returns `None` if no record exists (first attempt).
pub async fn find_by_key(
    pool: &SqlitePool,
    idempotency_key: &str,
) -> Result<Option<ApprovalMutationIdempotencyRecord>> {
    let row = sqlx::query(
        r#"SELECT idempotency_key, approval_id, action, caller_fingerprint,
                  request_id, request_hash, command_journal_id, result_hash,
                  committed_at_ms, expires_at_ms
           FROM approval_mutation_idempotency
           WHERE idempotency_key = ?1"#,
    )
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .context("approval_mutation_idempotency: find_by_key")?;

    Ok(row.map(|r| ApprovalMutationIdempotencyRecord {
        idempotency_key: r.get("idempotency_key"),
        approval_id: r.get("approval_id"),
        action: r.get("action"),
        caller_fingerprint: r.get("caller_fingerprint"),
        request_id: r.get("request_id"),
        request_hash: r.get("request_hash"),
        command_journal_id: r.get("command_journal_id"),
        result_hash: r.get("result_hash"),
        committed_at_ms: r.get("committed_at_ms"),
        expires_at_ms: r.get("expires_at_ms"),
    }))
}

/// Look up an existing idempotency record by key inside a caller-owned transaction.
/// SEC-P081-MED-001: Must run inside the same BEGIN IMMEDIATE settlement transaction
/// as the terminal-state check so concurrent retries cannot race past the record.
pub async fn find_by_key_tx<'c>(
    tx: &mut sqlx::Transaction<'c, sqlx::Sqlite>,
    idempotency_key: &str,
) -> Result<Option<ApprovalMutationIdempotencyRecord>> {
    let row = sqlx::query(
        r#"SELECT idempotency_key, approval_id, action, caller_fingerprint,
                  request_id, request_hash, command_journal_id, result_hash,
                  committed_at_ms, expires_at_ms
           FROM approval_mutation_idempotency
           WHERE idempotency_key = ?1"#,
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .context("approval_mutation_idempotency: find_by_key_tx")?;

    Ok(row.map(|r| ApprovalMutationIdempotencyRecord {
        idempotency_key: r.get("idempotency_key"),
        approval_id: r.get("approval_id"),
        action: r.get("action"),
        caller_fingerprint: r.get("caller_fingerprint"),
        request_id: r.get("request_id"),
        request_hash: r.get("request_hash"),
        command_journal_id: r.get("command_journal_id"),
        result_hash: r.get("result_hash"),
        committed_at_ms: r.get("committed_at_ms"),
        expires_at_ms: r.get("expires_at_ms"),
    }))
}

/// Insert one idempotency record inside an existing caller-owned transaction.
/// Called after approval settlement commits, so the record lands in the same
/// write unit as command_journal, approval settlement, and audit_log rows.
pub async fn insert_tx<'c>(
    tx: &mut Transaction<'c, Sqlite>,
    record: &ApprovalMutationIdempotencyRecord,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO approval_mutation_idempotency
               (idempotency_key, approval_id, action, caller_fingerprint,
                request_id, request_hash, command_journal_id, result_hash,
                committed_at_ms, expires_at_ms, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(&record.idempotency_key)
    .bind(&record.approval_id)
    .bind(&record.action)
    .bind(&record.caller_fingerprint)
    .bind(&record.request_id)
    .bind(&record.request_hash)
    .bind(&record.command_journal_id)
    .bind(&record.result_hash)
    .bind(record.committed_at_ms)
    .bind(record.expires_at_ms)
    .bind(&now)
    .execute(&mut **tx)
    .await
    .context("approval_mutation_idempotency: insert_tx")?;
    Ok(())
}

/// Build a new record for insertion. `committed_at_ms` is set to now; `expires_at_ms`
/// is set 7 days out.
pub fn build_record(
    idempotency_key: &str,
    approval_id: &str,
    action: &str,
    caller_fingerprint: &str,
    request_id: Option<&str>,
    request_hash: Option<&str>,
    command_journal_id: &str,
    result_hash: Option<&str>,
) -> ApprovalMutationIdempotencyRecord {
    let now_ms = Utc::now().timestamp_millis();
    ApprovalMutationIdempotencyRecord {
        idempotency_key: idempotency_key.to_string(),
        approval_id: approval_id.to_string(),
        action: action.to_string(),
        caller_fingerprint: caller_fingerprint.to_string(),
        request_id: request_id.map(|s| s.to_string()),
        request_hash: request_hash.map(|s| s.to_string()),
        command_journal_id: command_journal_id.to_string(),
        result_hash: result_hash.map(|s| s.to_string()),
        committed_at_ms: now_ms,
        expires_at_ms: now_ms + RETENTION_MS,
    }
}
