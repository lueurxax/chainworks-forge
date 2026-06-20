//! P083: Shutdown interrupted receipt and signal side effect repository.
//!
//! shutdown_interrupted_receipts tracks provider sessions that could not be
//! signaled before a deadline expired. shutdown_signal_side_effects tracks
//! planned/dispatching/issued/observed/suppressed/identity_mismatch signals
//! with generation replay semantics. Per shutdown_signal_side_effect_contract_v1:
//! generation is never incremented merely because the daemon restarted.
//!
//! PARTIAL RUNTIME WIRING: The planned→dispatching→issued CAS path is implemented
//! (mark_signal_dispatching, mark_signal_issued). Full end-to-end wiring of
//! issue_shutdown_signal and observe_exit to the AppKit graceful shutdown path and
//! session teardown remains deferred pending architectural change.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, SqlitePool};

/// A row from the shutdown_interrupted_receipts table.
#[derive(Debug, Clone)]
pub struct ShutdownInterruptedReceipt {
    pub receipt_id: String,
    pub provider_session_id: String,
    pub shutdown_epoch: i64,
    pub receipt_generation: i64,
    pub interrupted_state: String,
    /// Non-null only when interrupted_state = 'queued_no_signal'.
    pub queue_rank: Option<i64>,
    pub created_at: String,
    pub recovered_at: Option<String>,
}

/// A row from the shutdown_signal_side_effects table.
#[derive(Debug, Clone)]
pub struct ShutdownSignalSideEffect {
    pub signal_effect_id: String,
    pub provider_session_id: String,
    pub shutdown_epoch: i64,
    pub process_id: i64,
    pub process_start_identity: String,
    pub signal_kind: String,
    pub generation: i64,
    pub intent_state: String,
    pub issued_at_monotonic_ms: Option<i64>,
    pub issued_at_wall_clock: Option<String>,
    pub observed_exit_at_monotonic_ms: Option<i64>,
    pub baseline_sample_id: Option<String>,
    pub error_code: Option<String>,
}

fn map_receipt(r: sqlx::sqlite::SqliteRow) -> ShutdownInterruptedReceipt {
    ShutdownInterruptedReceipt {
        receipt_id: r.get("receipt_id"),
        provider_session_id: r.get("provider_session_id"),
        shutdown_epoch: r.get("shutdown_epoch"),
        receipt_generation: r.get("receipt_generation"),
        interrupted_state: r.get("interrupted_state"),
        queue_rank: r.get("queue_rank"),
        created_at: r.get("created_at"),
        recovered_at: r.get("recovered_at"),
    }
}

fn map_signal(r: sqlx::sqlite::SqliteRow) -> ShutdownSignalSideEffect {
    ShutdownSignalSideEffect {
        signal_effect_id: r.get("signal_effect_id"),
        provider_session_id: r.get("provider_session_id"),
        shutdown_epoch: r.get("shutdown_epoch"),
        process_id: r.get("process_id"),
        process_start_identity: r.get("process_start_identity"),
        signal_kind: r.get("signal_kind"),
        generation: r.get("generation"),
        intent_state: r.get("intent_state"),
        issued_at_monotonic_ms: r.get("issued_at_monotonic_ms"),
        issued_at_wall_clock: r.get("issued_at_wall_clock"),
        observed_exit_at_monotonic_ms: r.get("observed_exit_at_monotonic_ms"),
        baseline_sample_id: r.get("baseline_sample_id"),
        error_code: r.get("error_code"),
    }
}

const RECEIPT_SELECT: &str = r#"SELECT receipt_id, provider_session_id, shutdown_epoch,
       receipt_generation, interrupted_state, queue_rank, created_at, recovered_at
  FROM shutdown_interrupted_receipts"#;

const SIGNAL_SELECT: &str = r#"SELECT signal_effect_id, provider_session_id, shutdown_epoch,
       process_id, process_start_identity, signal_kind, generation, intent_state,
       issued_at_monotonic_ms, issued_at_wall_clock, observed_exit_at_monotonic_ms,
       baseline_sample_id, error_code
  FROM shutdown_signal_side_effects"#;

// ── shutdown_interrupted_receipts ────────────────────────────────────────────

/// Insert a new shutdown interrupted receipt. queue_rank must be Some only
/// when interrupted_state='queued_no_signal', None otherwise.
pub async fn insert_interrupted_receipt(
    pool: &SqlitePool,
    receipt_id: &str,
    provider_session_id: &str,
    shutdown_epoch: i64,
    receipt_generation: i64,
    interrupted_state: &str,
    queue_rank: Option<i64>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO shutdown_interrupted_receipts
           (receipt_id, provider_session_id, shutdown_epoch, receipt_generation,
            interrupted_state, queue_rank, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
    )
    .bind(receipt_id)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(receipt_generation)
    .bind(interrupted_state)
    .bind(queue_rank)
    .bind(&now)
    .execute(pool)
    .await
    .context("shutdown_interrupted_receipts: insert")?;
    Ok(())
}

/// Mark a receipt as recovered. Sets recovered_at to now.
pub async fn mark_receipt_recovered(pool: &SqlitePool, receipt_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE shutdown_interrupted_receipts
           SET recovered_at = ?1
           WHERE receipt_id = ?2 AND recovered_at IS NULL"#,
    )
    .bind(&now)
    .bind(receipt_id)
    .execute(pool)
    .await
    .context("shutdown_interrupted_receipts: mark_recovered")?;
    Ok(rows.rows_affected() > 0)
}

/// Find all receipts for a provider session, ordered by shutdown_epoch then generation.
pub async fn find_receipts_by_session(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Vec<ShutdownInterruptedReceipt>> {
    let rows = sqlx::query(&format!(
        "{RECEIPT_SELECT} WHERE provider_session_id = ?1
         ORDER BY shutdown_epoch, receipt_generation"
    ))
    .bind(provider_session_id)
    .fetch_all(pool)
    .await
    .context("shutdown_interrupted_receipts: find_by_session")?;
    Ok(rows.into_iter().map(map_receipt).collect())
}

/// Find a specific receipt by (provider_session_id, shutdown_epoch, generation).
pub async fn find_receipt_by_key(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    receipt_generation: i64,
) -> Result<Option<ShutdownInterruptedReceipt>> {
    let row = sqlx::query(&format!(
        "{RECEIPT_SELECT} WHERE provider_session_id = ?1
         AND shutdown_epoch = ?2 AND receipt_generation = ?3"
    ))
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(receipt_generation)
    .fetch_optional(pool)
    .await
    .context("shutdown_interrupted_receipts: find_by_key")?;
    Ok(row.map(map_receipt))
}

// ── shutdown_signal_side_effects ─────────────────────────────────────────────

/// Insert a new signal side effect in 'planned' state.
/// Per generation_replay_rule: INSERT OR IGNORE lets a restart safely call
/// this again for the same (provider_session_id, shutdown_epoch, signal_kind,
/// generation) without creating a duplicate.
pub async fn insert_signal_planned(
    pool: &SqlitePool,
    signal_effect_id: &str,
    provider_session_id: &str,
    shutdown_epoch: i64,
    process_id: i64,
    process_start_identity: &str,
    signal_kind: &str,
    generation: i64,
    baseline_sample_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT OR IGNORE INTO shutdown_signal_side_effects
           (signal_effect_id, provider_session_id, shutdown_epoch,
            process_id, process_start_identity, signal_kind, generation, intent_state,
            baseline_sample_id)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'planned', ?8)"#,
    )
    .bind(signal_effect_id)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(process_id)
    .bind(process_start_identity)
    .bind(signal_kind)
    .bind(generation)
    .bind(baseline_sample_id)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: insert_planned")?;
    Ok(())
}

/// Atomically claim a signal for dispatch by transitioning 'planned' → 'dispatching'.
///
/// SEC-P083-HIGH-001: This CAS must succeed before calling kill(). Only one dispatcher can
/// claim a planned row; a second dispatcher that calls this for the same row gets false back
/// and must skip the signal. Recovery also skips 'dispatching' rows (already claimed/attempted).
pub async fn mark_signal_dispatching(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE shutdown_signal_side_effects
           SET intent_state = 'dispatching'
           WHERE provider_session_id = ?1 AND shutdown_epoch = ?2
             AND signal_kind = ?3 AND generation = ?4
             AND intent_state = 'planned'"#,
    )
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: mark_dispatching")?;
    Ok(rows.rows_affected() > 0)
}

/// Transition a signal from 'dispatching' to 'issued'. Records monotonic and wall-clock timestamps.
///
/// SEC-P083-HIGH-001: The caller must have already acquired the row via mark_signal_dispatching
/// (which transitions from 'planned' to 'dispatching'). The WHERE clause guards 'dispatching'
/// so a second call on the same row is a no-op.
pub async fn mark_signal_issued(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
    issued_at_monotonic_ms: i64,
) -> Result<bool> {
    let wall_clock = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE shutdown_signal_side_effects
           SET intent_state = 'issued',
               issued_at_monotonic_ms = ?1,
               issued_at_wall_clock = ?2
           WHERE provider_session_id = ?3 AND shutdown_epoch = ?4
             AND signal_kind = ?5 AND generation = ?6
             AND intent_state = 'dispatching'"#,
    )
    .bind(issued_at_monotonic_ms)
    .bind(&wall_clock)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: mark_issued")?;
    Ok(rows.rows_affected() > 0)
}

/// Transition to 'observed' once process exit is confirmed.
pub async fn mark_signal_observed(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
    observed_exit_at_monotonic_ms: i64,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE shutdown_signal_side_effects
           SET intent_state = 'observed',
               observed_exit_at_monotonic_ms = ?1
           WHERE provider_session_id = ?2 AND shutdown_epoch = ?3
             AND signal_kind = ?4 AND generation = ?5
             AND intent_state = 'issued'"#,
    )
    .bind(observed_exit_at_monotonic_ms)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: mark_observed")?;
    Ok(rows.rows_affected() > 0)
}

/// Transition to 'suppressed' — duplicate send for already-issued generation.
/// Per duplicate_suppression rule: the unique key prevents re-insert; this
/// updates an 'issued' row that recovery detects as already issued.
pub async fn mark_signal_suppressed(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
    error_code: Option<&str>,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE shutdown_signal_side_effects
           SET intent_state = 'suppressed', error_code = ?1
           WHERE provider_session_id = ?2 AND shutdown_epoch = ?3
             AND signal_kind = ?4 AND generation = ?5
             AND intent_state IN ('planned','issued')"#,
    )
    .bind(error_code)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: mark_suppressed")?;
    Ok(rows.rows_affected() > 0)
}

/// Transition to 'identity_mismatch' when OS process identity differs from stored identity.
pub async fn mark_signal_identity_mismatch(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
    error_code: Option<&str>,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE shutdown_signal_side_effects
           SET intent_state = 'identity_mismatch', error_code = ?1
           WHERE provider_session_id = ?2 AND shutdown_epoch = ?3
             AND signal_kind = ?4 AND generation = ?5
             AND intent_state IN ('planned','issued')"#,
    )
    .bind(error_code)
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .execute(pool)
    .await
    .context("shutdown_signal_side_effects: mark_identity_mismatch")?;
    Ok(rows.rows_affected() > 0)
}

/// Find the active (planned, dispatching, or issued) signal for a given key.
/// Used by recovery to determine whether to reuse the generation or issue fresh.
pub async fn find_active_signal(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
) -> Result<Option<ShutdownSignalSideEffect>> {
    let row = sqlx::query(&format!(
        "{SIGNAL_SELECT}
         WHERE provider_session_id = ?1 AND shutdown_epoch = ?2
           AND signal_kind = ?3 AND generation = ?4
           AND intent_state IN ('planned','dispatching','issued')"
    ))
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .bind(signal_kind)
    .bind(generation)
    .fetch_optional(pool)
    .await
    .context("shutdown_signal_side_effects: find_active")?;
    Ok(row.map(map_signal))
}

/// Find all signals for a provider session ordered by epoch and generation.
pub async fn find_signals_by_session(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Vec<ShutdownSignalSideEffect>> {
    let rows = sqlx::query(&format!(
        "{SIGNAL_SELECT} WHERE provider_session_id = ?1
         ORDER BY shutdown_epoch, signal_kind, generation"
    ))
    .bind(provider_session_id)
    .fetch_all(pool)
    .await
    .context("shutdown_signal_side_effects: find_by_session")?;
    Ok(rows.into_iter().map(map_signal).collect())
}
