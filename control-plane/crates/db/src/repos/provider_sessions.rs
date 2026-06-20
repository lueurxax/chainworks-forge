//! P083: Provider session lifecycle and cancellation intent repository.
//!
//! provider_sessions is the authoritative lifecycle record for provider processes.
//! provider_cancellation_intents records durable cancellation requests.
//! Per provider_cancellation_intent_contract_v1: held+identity_ambiguous intents
//! are not auto-retried on restart.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, SqlitePool};

/// A row from the provider_sessions table.
#[derive(Debug, Clone)]
pub struct ProviderSession {
    pub provider_session_id: String,
    pub run_id: String,
    pub agent_execution_id: Option<String>,
    pub provider: String,
    pub lifecycle_state: String,
    pub process_id: Option<i64>,
    pub process_start_identity: Option<String>,
    pub process_fate: String,
    pub process_fate_updated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A row from the provider_cancellation_intents table.
#[derive(Debug, Clone)]
pub struct ProviderCancellationIntent {
    pub provider_session_id: String,
    pub cancellation_epoch: i64,
    pub intent_state: String,
    pub reason: String,
    pub requested_at_monotonic_ms: i64,
    pub requested_at_wall_clock: String,
    pub baseline_sample_id: Option<String>,
    pub shutdown_epoch: Option<i64>,
    pub shutdown_epoch_assigned_at: Option<String>,
}

fn map_session(r: sqlx::sqlite::SqliteRow) -> ProviderSession {
    ProviderSession {
        provider_session_id: r.get("provider_session_id"),
        run_id: r.get("run_id"),
        agent_execution_id: r.get("agent_execution_id"),
        provider: r.get("provider"),
        lifecycle_state: r.get("lifecycle_state"),
        process_id: r.get("process_id"),
        process_start_identity: r.get("process_start_identity"),
        process_fate: r.get("process_fate"),
        process_fate_updated_at: r.get("process_fate_updated_at"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

fn map_intent(r: sqlx::sqlite::SqliteRow) -> ProviderCancellationIntent {
    ProviderCancellationIntent {
        provider_session_id: r.get("provider_session_id"),
        cancellation_epoch: r.get("cancellation_epoch"),
        intent_state: r.get("intent_state"),
        reason: r.get("reason"),
        requested_at_monotonic_ms: r.get("requested_at_monotonic_ms"),
        requested_at_wall_clock: r.get("requested_at_wall_clock"),
        baseline_sample_id: r.get("baseline_sample_id"),
        shutdown_epoch: r.get("shutdown_epoch"),
        shutdown_epoch_assigned_at: r.get("shutdown_epoch_assigned_at"),
    }
}

const SESSION_SELECT: &str = r#"SELECT provider_session_id, run_id, agent_execution_id,
       provider, lifecycle_state, process_id, process_start_identity,
       process_fate, process_fate_updated_at, created_at, updated_at
  FROM provider_sessions"#;

const INTENT_SELECT: &str = r#"SELECT provider_session_id, cancellation_epoch,
       intent_state, reason, requested_at_monotonic_ms, requested_at_wall_clock,
       baseline_sample_id, shutdown_epoch, shutdown_epoch_assigned_at
  FROM provider_cancellation_intents"#;

// ── provider_sessions ────────────────────────────────────────────────────────

/// Insert a new provider session with initial lifecycle_state='registered' and
/// process_fate='running'.
pub async fn insert(
    pool: &SqlitePool,
    provider_session_id: &str,
    run_id: &str,
    agent_execution_id: Option<&str>,
    provider: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO provider_sessions
           (provider_session_id, run_id, agent_execution_id, provider,
            lifecycle_state, process_fate, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, 'registered', 'running', ?5, ?6)"#,
    )
    .bind(provider_session_id)
    .bind(run_id)
    .bind(agent_execution_id)
    .bind(provider)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .context("provider_sessions: insert")?;
    Ok(())
}

/// Insert a provider session row if one does not already exist.
/// Uses INSERT OR IGNORE so multi-turn reused sessions do not fail on subsequent turns.
/// Per provider_sessions truth contract: lifecycle_state starts as 'registered' and transitions
/// to 'live' / terminal states via executor lifecycle hooks.
pub async fn insert_or_ignore(
    pool: &SqlitePool,
    provider_session_id: &str,
    run_id: &str,
    agent_execution_id: Option<&str>,
    provider: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT OR IGNORE INTO provider_sessions
           (provider_session_id, run_id, agent_execution_id, provider,
            lifecycle_state, process_fate, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, 'registered', 'running', ?5, ?6)"#,
    )
    .bind(provider_session_id)
    .bind(run_id)
    .bind(agent_execution_id)
    .bind(provider)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .context("provider_sessions: insert_or_ignore")?;
    Ok(())
}

/// Update the lifecycle state of a provider session.
pub async fn update_lifecycle_state(
    pool: &SqlitePool,
    provider_session_id: &str,
    lifecycle_state: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE provider_sessions
           SET lifecycle_state = ?1, updated_at = ?2
           WHERE provider_session_id = ?3"#,
    )
    .bind(lifecycle_state)
    .bind(&now)
    .bind(provider_session_id)
    .execute(pool)
    .await
    .context("provider_sessions: update_lifecycle_state")?;
    Ok(rows.rows_affected() > 0)
}

/// Find a provider session by its primary key.
pub async fn find_by_id(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Option<ProviderSession>> {
    let row = sqlx::query(&format!("{SESSION_SELECT} WHERE provider_session_id = ?1"))
        .bind(provider_session_id)
        .fetch_optional(pool)
        .await
        .context("provider_sessions: find_by_id")?;
    Ok(row.map(map_session))
}

/// Find all provider sessions for a run.
pub async fn find_by_run_id(pool: &SqlitePool, run_id: &str) -> Result<Vec<ProviderSession>> {
    let rows = sqlx::query(&format!(
        "{SESSION_SELECT} WHERE run_id = ?1 ORDER BY created_at"
    ))
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("provider_sessions: find_by_run_id")?;
    Ok(rows.into_iter().map(map_session).collect())
}

/// Find all provider sessions with process_fate='identity_ambiguous'.
/// Used by recovery to surface manual_process_identity_check holds.
pub async fn find_identity_ambiguous(pool: &SqlitePool) -> Result<Vec<ProviderSession>> {
    let rows = sqlx::query(&format!(
        "{SESSION_SELECT} WHERE process_fate = 'identity_ambiguous'"
    ))
    .fetch_all(pool)
    .await
    .context("provider_sessions: find_identity_ambiguous")?;
    Ok(rows.into_iter().map(map_session).collect())
}

/// Update process_fate. Per identity_ambiguous_canonical_rule: sets
/// process_fate and records process_fate_updated_at.
pub async fn update_process_fate(
    pool: &SqlitePool,
    provider_session_id: &str,
    process_fate: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE provider_sessions
           SET process_fate = ?1, process_fate_updated_at = ?2, updated_at = ?2
           WHERE provider_session_id = ?3"#,
    )
    .bind(process_fate)
    .bind(&now)
    .bind(provider_session_id)
    .execute(pool)
    .await
    .context("provider_sessions: update_process_fate")?;
    Ok(rows.rows_affected() > 0)
}

/// Record process identity (pid + start identity) at launch handshake.
pub async fn set_process_identity(
    pool: &SqlitePool,
    provider_session_id: &str,
    process_id: i64,
    process_start_identity: &str,
) -> Result<bool> {
    // SEC-P083-HIGH-001: Reject non-positive PIDs and empty identities at the persistence boundary.
    // PID 0 and negative PIDs target process groups, not individual provider processes.
    // Empty process_start_identity is unverifiable and must not be stored as a valid identity.
    if process_id <= 0 {
        anyhow::bail!(
            "provider_sessions: set_process_identity rejected non-positive PID {} for session {}",
            process_id,
            provider_session_id
        );
    }
    if process_start_identity.is_empty() {
        anyhow::bail!(
            "provider_sessions: set_process_identity rejected empty process_start_identity for session {}",
            provider_session_id
        );
    }
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE provider_sessions
           SET process_id = ?1, process_start_identity = ?2, updated_at = ?3
           WHERE provider_session_id = ?4"#,
    )
    .bind(process_id)
    .bind(process_start_identity)
    .bind(&now)
    .bind(provider_session_id)
    .execute(pool)
    .await
    .context("provider_sessions: set_process_identity")?;
    Ok(rows.rows_affected() > 0)
}

// ── provider_cancellation_intents ────────────────────────────────────────────

/// Insert a new cancellation intent in 'requested' state.
/// cancellation_epoch must be unique per provider_session_id.
pub async fn insert_cancellation_intent(
    pool: &SqlitePool,
    provider_session_id: &str,
    cancellation_epoch: i64,
    reason: &str,
    requested_at_monotonic_ms: i64,
    baseline_sample_id: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO provider_cancellation_intents
           (provider_session_id, cancellation_epoch, intent_state, reason,
            requested_at_monotonic_ms, requested_at_wall_clock, baseline_sample_id)
           VALUES (?1, ?2, 'requested', ?3, ?4, ?5, ?6)"#,
    )
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .bind(reason)
    .bind(requested_at_monotonic_ms)
    .bind(&now)
    .bind(baseline_sample_id)
    .execute(pool)
    .await
    .context("provider_cancellation_intents: insert")?;
    Ok(())
}

/// Find an active (requested or shutdown_started) cancellation intent.
/// Returns the most recent cancellation_epoch for the session.
pub async fn find_active_cancellation_intent(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Option<ProviderCancellationIntent>> {
    let row = sqlx::query(&format!(
        "{INTENT_SELECT}
         WHERE provider_session_id = ?1
           AND intent_state IN ('requested','shutdown_started')
         ORDER BY cancellation_epoch DESC LIMIT 1"
    ))
    .bind(provider_session_id)
    .fetch_optional(pool)
    .await
    .context("provider_cancellation_intents: find_active")?;
    Ok(row.map(map_intent))
}

/// Find all cancellation intents for a provider session.
pub async fn find_cancellation_intents(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Vec<ProviderCancellationIntent>> {
    let rows = sqlx::query(&format!(
        "{INTENT_SELECT} WHERE provider_session_id = ?1
         ORDER BY cancellation_epoch"
    ))
    .bind(provider_session_id)
    .fetch_all(pool)
    .await
    .context("provider_cancellation_intents: find_all")?;
    Ok(rows.into_iter().map(map_intent).collect())
}

/// Update intent_state for a given (provider_session_id, cancellation_epoch).
/// For 'shutdown_started' or 'settled', assign shutdown_epoch per V7 constraint.
pub async fn update_cancellation_intent_state(
    pool: &SqlitePool,
    provider_session_id: &str,
    cancellation_epoch: i64,
    intent_state: &str,
    shutdown_epoch: Option<i64>,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE provider_cancellation_intents
           SET intent_state = ?1, shutdown_epoch = ?2, shutdown_epoch_assigned_at = ?3
           WHERE provider_session_id = ?4 AND cancellation_epoch = ?5"#,
    )
    .bind(intent_state)
    .bind(shutdown_epoch)
    .bind(if shutdown_epoch.is_some() {
        Some(now.as_str())
    } else {
        None
    })
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .execute(pool)
    .await
    .context("provider_cancellation_intents: update_state")?;
    Ok(rows.rows_affected() > 0)
}

/// Transition a 'requested' intent to 'held' and set process_fate='identity_ambiguous'
/// on the provider session atomically in one SQLite transaction.
/// Per identity_ambiguous_canonical_rule: shutdown_epoch stays NULL, not auto-retried.
/// Per provider_cancellation_intent_contract_v1: both writes must be atomic — a crash
/// between them would leave intent_state=held but process_fate=running.
pub async fn hold_identity_ambiguous(
    pool: &SqlitePool,
    provider_session_id: &str,
    cancellation_epoch: i64,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let mut tx = pool
        .begin()
        .await
        .context("provider_sessions: hold_identity_ambiguous begin tx")?;

    let rows = sqlx::query(
        r#"UPDATE provider_cancellation_intents
           SET intent_state = 'held'
           WHERE provider_session_id = ?1 AND cancellation_epoch = ?2
             AND intent_state = 'requested'"#,
    )
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .execute(&mut *tx)
    .await
    .context("provider_cancellation_intents: hold_identity_ambiguous")?;

    if rows.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE provider_sessions
               SET process_fate = 'identity_ambiguous',
                   process_fate_updated_at = ?1,
                   updated_at = ?1
               WHERE provider_session_id = ?2"#,
        )
        .bind(&now)
        .bind(provider_session_id)
        .execute(&mut *tx)
        .await
        .context("provider_sessions: hold_identity_ambiguous fate update")?;

        tx.commit()
            .await
            .context("provider_sessions: hold_identity_ambiguous commit")?;
        Ok(true)
    } else {
        tx.rollback().await.ok();
        Ok(false)
    }
}

/// Denormalized view of an identity-ambiguous provider session for UI display.
/// Joins provider_sessions with their held cancellation intent and latest
/// shutdown_interrupted_receipt for the manual_process_identity_check_ui_v1 banner.
#[derive(Debug, Clone)]
pub struct IdentityHoldSummary {
    pub provider_session_id: String,
    pub provider: String,
    pub process_id: Option<i64>,
    pub process_start_identity: Option<String>,
    pub cancellation_epoch: Option<i64>,
    pub held_reason: Option<String>,
    pub latest_receipt_id: Option<String>,
}

/// Returns identity-ambiguous sessions for a run with their held cancellation
/// epoch and most recent shutdown_interrupted receipt.
/// Per manual_process_identity_check_ui_v1: drives the ManualProcessIdentityCheckBanner.
pub async fn find_identity_ambiguous_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<IdentityHoldSummary>> {
    let rows = sqlx::query(
        r#"SELECT ps.provider_session_id,
                  ps.provider,
                  ps.process_id,
                  ps.process_start_identity,
                  pci.cancellation_epoch,
                  pci.reason AS held_reason,
                  (SELECT receipt_id FROM shutdown_interrupted_receipts
                   WHERE provider_session_id = ps.provider_session_id
                   ORDER BY created_at DESC LIMIT 1) AS latest_receipt_id
             FROM provider_sessions ps
             LEFT JOIN provider_cancellation_intents pci
                    ON pci.provider_session_id = ps.provider_session_id
                   AND pci.intent_state = 'held'
            WHERE ps.run_id = ?1
              AND ps.process_fate = 'identity_ambiguous'
            ORDER BY ps.created_at"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("provider_sessions: find_identity_ambiguous_for_run")?;
    Ok(rows
        .iter()
        .map(|r| IdentityHoldSummary {
            provider_session_id: r.get("provider_session_id"),
            provider: r.get("provider"),
            process_id: r.get("process_id"),
            process_start_identity: r.get("process_start_identity"),
            cancellation_epoch: r.get("cancellation_epoch"),
            held_reason: r.get("held_reason"),
            latest_receipt_id: r.get("latest_receipt_id"),
        })
        .collect())
}

/// Atomically: set process_fate='absent_verified' on the session and transition the
/// held cancellation intent back to 'requested' so shutdown settlement can resume.
/// Per manual_process_identity_check_ui_v1.available_actions.mark_process_absent:
/// operator has confirmed the process is absent; settlement may proceed without a
/// live process identity check.
///
/// Returns `true` if both rows were updated (held intent found and process_fate was
/// identity_ambiguous), `false` if the preconditions were not met (e.g., already settled).
///
/// Order matters: the held cancellation intent is checked FIRST (SELECT COUNT(*)) so
/// that if no held intent exists for the given epoch, provider_sessions is left
/// untouched and the caller can safely commit a failure without clearing the
/// identity_ambiguous hold. A wrong epoch would otherwise silently clear the hold
/// without re-opening any intent.
pub async fn mark_process_absent_verified_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    provider_session_id: &str,
    cancellation_epoch: i64,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();

    // Check that a held intent exists for this epoch BEFORE touching provider_sessions.
    // If the intent row is absent, returning false here leaves both tables unchanged so
    // the caller can commit a failure without clearing the identity_ambiguous hold.
    let intent_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM provider_cancellation_intents
           WHERE provider_session_id = ?1
             AND cancellation_epoch = ?2
             AND intent_state = 'held'"#,
    )
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .fetch_one(&mut **tx)
    .await
    .context("provider_cancellation_intents: mark_process_absent_verified_tx existence check")?;

    if intent_count == 0 {
        return Ok(false);
    }

    // Held intent confirmed; now update provider_sessions. If process_fate is not
    // identity_ambiguous the precondition is not met and we return false.
    let fate_rows = sqlx::query(
        r#"UPDATE provider_sessions
           SET process_fate = 'absent_verified',
               process_fate_updated_at = ?1,
               updated_at = ?1
           WHERE provider_session_id = ?2
             AND process_fate = 'identity_ambiguous'"#,
    )
    .bind(&now)
    .bind(provider_session_id)
    .execute(&mut **tx)
    .await
    .context("provider_sessions: mark_process_absent_verified_tx fate update")?;

    if fate_rows.rows_affected() == 0 {
        return Ok(false);
    }

    // provider_sessions updated; now re-open the held intent so shutdown can proceed.
    let intent_rows = sqlx::query(
        r#"UPDATE provider_cancellation_intents
           SET intent_state = 'requested'
           WHERE provider_session_id = ?1
             AND cancellation_epoch = ?2
             AND intent_state = 'held'"#,
    )
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .execute(&mut **tx)
    .await
    .context("provider_cancellation_intents: mark_process_absent_verified_tx")?;

    if intent_rows.rows_affected() == 0 {
        return Ok(false);
    }

    Ok(true)
}

/// Find all provider_cancellation_intents rows with intent_state='requested'
/// and shutdown_epoch IS NULL. Used by startup recovery to detect intents whose
/// process status cannot be proven — per provider_cancellation_intent_contract_v1
/// identity_ambiguous_canonical_rule, these must be transitioned to held.
pub async fn find_requested_intents_null_shutdown_epoch(
    pool: &SqlitePool,
) -> Result<Vec<ProviderCancellationIntent>> {
    let rows = sqlx::query(&format!(
        "{INTENT_SELECT} WHERE intent_state = 'requested' AND shutdown_epoch IS NULL"
    ))
    .fetch_all(pool)
    .await
    .context("provider_cancellation_intents: find_requested_null_shutdown_epoch")?;
    Ok(rows.into_iter().map(map_intent).collect())
}
