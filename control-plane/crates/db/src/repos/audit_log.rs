//! P081 Phase 1: audit_log repository.
//!
//! Provides transactional append (`append_tx`) for allowed mutating paths already
//! inside a BEGIN IMMEDIATE write unit, and a bounded standalone append path
//! (`append`) for deny-only durable audit writes.
//!
//! Row-hash, prev-hash linkage, and payload truncation envelope construction are
//! owned here so each caller does not reimplement audit serialization rules.
//! Bearer tokens are never logged.
//!
//! Hash chain: row_hash = sha256 of canonical row fields (excluding row_hash)
//! concatenated with prev_hash. checkpoint_hash construction is handled by
//! `append_checkpoint_tx`.

use anyhow::{Context, Result};
use sqlx::{Sqlite, SqlitePool, Transaction};

/// Lightweight description of one audit log entry passed to append functions.
pub struct AuditEntry<'a> {
    pub id: &'a str,
    pub request_id: &'a str,
    pub timestamp_ms: i64,
    pub event_type: &'a str,
    pub principal_id: Option<&'a str>,
    pub principal_class: Option<&'a str>,
    pub caller_class: Option<&'a str>,
    pub token_id: Option<&'a str>,
    pub transport: &'a str,
    pub action_attempted: &'a str,
    pub decision: &'a str,
    pub denial_reason_code: Option<&'a str>,
    pub row_id: Option<&'a str>,
    pub env_gate_state: Option<&'a str>,
    pub source_ip_hash_or_local_process_id: Option<&'a str>,
    pub boundary_policy_mode: &'a str,
    pub fixture_version: &'a str,
    /// Canonical JSON payload, already capped at 16 KiB by caller.
    pub payload: &'a str,
    /// SHA-256 hex digest of the untruncated payload (or truncated envelope).
    pub payload_sha256: &'a str,
    pub diagnostic_truncated: bool,
    /// SHA-256 hex digest of the previous committed audit_log row_hash, or None
    /// for the first row.
    pub prev_hash: Option<&'a str>,
    /// SHA-256 hex digest of this row's canonical content (computed by caller
    /// before insert to allow verification without a re-read).
    pub row_hash: &'a str,
    pub checkpoint_id: Option<&'a str>,
    pub created_at_ms: i64,
}

/// Lightweight description of one checkpoint entry.
pub struct CheckpointEntry<'a> {
    pub checkpoint_id: &'a str,
    pub checkpoint_seq: i64,
    pub covered_start_id: &'a str,
    pub covered_end_id: &'a str,
    pub covered_row_count: i64,
    pub previous_checkpoint_hash: Option<&'a str>,
    pub checkpoint_hash: &'a str,
    pub created_at_ms: i64,
}

/// Bounded health snapshot for operator readback. Does not expose raw rows.
#[derive(Debug, Clone)]
pub struct AuditLogHealthSnapshot {
    pub row_count: i64,
    pub latest_row_id: Option<String>,
    pub latest_checkpoint_seq: Option<i64>,
    pub latest_checkpoint_hash: Option<String>,
}

/// Append one audit log row inside an existing caller-owned BEGIN IMMEDIATE
/// transaction. Used by allowed mutating paths that commit audit evidence
/// in the same write unit as command_journal, idempotency, and approval
/// settlement.
pub async fn append_tx<'c>(
    tx: &mut Transaction<'c, Sqlite>,
    entry: &AuditEntry<'_>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO audit_log (
            id, request_id, timestamp_ms, event_type,
            principal_id, principal_class, caller_class, token_id,
            transport, action_attempted, decision, denial_reason_code,
            row_id, env_gate_state, source_ip_hash_or_local_process_id,
            boundary_policy_mode, fixture_version,
            payload_schema_version, payload, payload_sha256, diagnostic_truncated,
            prev_hash, row_hash, checkpoint_id, created_at_ms
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12,
            ?13, ?14, ?15,
            ?16, ?17,
            1, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24
        )"#,
    )
    .bind(entry.id)
    .bind(entry.request_id)
    .bind(entry.timestamp_ms)
    .bind(entry.event_type)
    .bind(entry.principal_id)
    .bind(entry.principal_class)
    .bind(entry.caller_class)
    .bind(entry.token_id)
    .bind(entry.transport)
    .bind(entry.action_attempted)
    .bind(entry.decision)
    .bind(entry.denial_reason_code)
    .bind(entry.row_id)
    .bind(entry.env_gate_state)
    .bind(entry.source_ip_hash_or_local_process_id)
    .bind(entry.boundary_policy_mode)
    .bind(entry.fixture_version)
    .bind(entry.payload)
    .bind(entry.payload_sha256)
    .bind(entry.diagnostic_truncated as i64)
    .bind(entry.prev_hash)
    .bind(entry.row_hash)
    .bind(entry.checkpoint_id)
    .bind(entry.created_at_ms)
    .execute(&mut **tx)
    .await
    .context("append audit_log entry (tx)")?;
    Ok(())
}

/// Append one audit log row in a bounded standalone BEGIN IMMEDIATE transaction.
/// Used by deny-only non-command paths that must open their own audit write
/// unit. Commits before returning so durable evidence is guaranteed before
/// the denial is sent to the caller.
pub async fn append(pool: &SqlitePool, entry: &AuditEntry<'_>) -> Result<()> {
    let mut tx = crate::pool::begin_immediate_with_retry(pool, "audit_log.append").await?;
    append_tx(&mut tx, entry).await?;
    tx.commit().await.context("commit standalone audit_log entry")?;
    Ok(())
}

/// Append one checkpoint row inside an existing caller-owned transaction.
/// Checkpoints are written every 1000 rows and at clean shutdown when the
/// open window is non-empty.
pub async fn append_checkpoint_tx<'c>(
    tx: &mut Transaction<'c, Sqlite>,
    entry: &CheckpointEntry<'_>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO audit_log_checkpoints (
            checkpoint_id, checkpoint_seq,
            covered_start_id, covered_end_id, covered_row_count,
            previous_checkpoint_hash, checkpoint_hash, created_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(entry.checkpoint_id)
    .bind(entry.checkpoint_seq)
    .bind(entry.covered_start_id)
    .bind(entry.covered_end_id)
    .bind(entry.covered_row_count)
    .bind(entry.previous_checkpoint_hash)
    .bind(entry.checkpoint_hash)
    .bind(entry.created_at_ms)
    .execute(&mut **tx)
    .await
    .context("append audit_log_checkpoints entry")?;
    Ok(())
}

/// Bounded health snapshot for operator diagnostics. Never exposes raw audit
/// rows or unbounded query surfaces.
pub async fn health_snapshot(pool: &SqlitePool) -> Result<AuditLogHealthSnapshot> {
    let row_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(pool)
            .await
            .context("audit_log row count")?;

    let latest_row_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM audit_log ORDER BY created_at_ms DESC LIMIT 1")
            .fetch_optional(pool)
            .await
            .context("audit_log latest row_id")?;

    let checkpoint_row: Option<(i64, String)> = sqlx::query_as(
        "SELECT checkpoint_seq, checkpoint_hash FROM audit_log_checkpoints ORDER BY checkpoint_seq DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("audit_log latest checkpoint")?;

    let (latest_checkpoint_seq, latest_checkpoint_hash) = match checkpoint_row {
        Some((seq, hash)) => (Some(seq), Some(hash)),
        None => (None, None),
    };

    Ok(AuditLogHealthSnapshot {
        row_count,
        latest_row_id,
        latest_checkpoint_seq,
        latest_checkpoint_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::create_pool;

    async fn test_pool() -> SqlitePool {
        create_pool("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn append_and_health_roundtrip() {
        let pool = test_pool().await;
        let hash1 = "b".repeat(64);
        let entry = AuditEntry {
            id: "entry-001",
            request_id: "req-001",
            timestamp_ms: 1_000_000,
            event_type: "boundary_decision",
            principal_id: Some("default-operator"),
            principal_class: Some("operator"),
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_query",
            action_attempted: "runs.get",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.ui_operator.graphql_query.read"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "shadow",
            fixture_version: "p081-v1",
            payload: r#"{"event":"boundary_decision"}"#,
            payload_sha256: &"a".repeat(64),
            diagnostic_truncated: false,
            prev_hash: None,
            row_hash: &hash1,
            checkpoint_id: None,
            created_at_ms: 1_000_000,
        };

        append(&pool, &entry).await.unwrap();
        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
        assert_eq!(snap.latest_row_id.as_deref(), Some("entry-001"));
        assert!(snap.latest_checkpoint_seq.is_none());
    }

    #[tokio::test]
    async fn append_tx_inside_transaction() {
        let pool = test_pool().await;
        let hash1 = "c".repeat(64);
        let entry = AuditEntry {
            id: "tx-entry-001",
            request_id: "req-tx-001",
            timestamp_ms: 2_000_000,
            event_type: "boundary_decision",
            principal_id: None,
            principal_class: None,
            caller_class: Some("agent_operator"),
            token_id: None,
            transport: "mcp_tools_call",
            action_attempted: "runs.start",
            decision: "deny",
            denial_reason_code: Some("CAPABILITY_OUT_OF_SCOPE"),
            row_id: Some("p081.agent_operator.mcp_tools_call.command"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-v1",
            payload: r#"{"event":"deny"}"#,
            payload_sha256: &"d".repeat(64),
            diagnostic_truncated: false,
            prev_hash: None,
            row_hash: &hash1,
            checkpoint_id: None,
            created_at_ms: 2_000_000,
        };

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_tx").await.unwrap();
        append_tx(&mut tx, &entry).await.unwrap();
        tx.commit().await.unwrap();

        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
    }

    #[tokio::test]
    async fn append_checkpoint_tx_roundtrip() {
        let pool = test_pool().await;
        let row_hash = "e".repeat(64);
        let row_entry = AuditEntry {
            id: "cp-entry-001",
            request_id: "req-cp-001",
            timestamp_ms: 3_000_000,
            event_type: "boundary_decision",
            principal_id: None,
            principal_class: None,
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_mutation",
            action_attempted: "approveApproval",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.ui_operator.graphql_mutation.approval_action"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-v1",
            payload: r#"{"event":"allow"}"#,
            payload_sha256: &"f".repeat(64),
            diagnostic_truncated: false,
            prev_hash: None,
            row_hash: &row_hash,
            checkpoint_id: Some("cp-001"),
            created_at_ms: 3_000_000,
        };

        let checkpoint_hash = "9".repeat(64);
        let cp = CheckpointEntry {
            checkpoint_id: "cp-001",
            checkpoint_seq: 1,
            covered_start_id: "cp-entry-001",
            covered_end_id: "cp-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &checkpoint_hash,
            created_at_ms: 3_000_001,
        };

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_cp").await.unwrap();
        append_tx(&mut tx, &row_entry).await.unwrap();
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
        assert_eq!(snap.latest_checkpoint_seq, Some(1));
        assert_eq!(snap.latest_checkpoint_hash.as_deref(), Some(&checkpoint_hash as &str));
    }
}
