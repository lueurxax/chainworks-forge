//! P083: Command idempotency lease repository.
//!
//! Implements acquire/replay/commit/fail/abandon for the command_idempotency
//! table and alias lookup for command_request_aliases.
//! Per command_idempotency_contract_v1: at most one active lease per
//! (principal_id, request_id) and at most one active committed/pending lease
//! per (principal_id, command, intent_hash).
//!
//! INVARIANT (P083-SEC-L1): Every caller must supply a principal_id that
//! exactly matches the authenticated principal from the request context.
//! Passing a caller-supplied or unauthenticated principal_id is a security
//! violation — it allows one principal to acquire or replay leases belonging
//! to another. The principal_id MUST be resolved from the verified auth
//! context before any call to this module's functions.
//!
//! NOT YET WIRED: These repo functions are tested by proposal_083_migrations.rs
//! integration tests but are not yet called from any production command path.
//! Production lifecycle commands (runs.cancel, runs.retry, stages.retry,
//! approvals.resolve, provider_session.shutdown, p083.rollback_execution,
//! p083.set_enforcement_mode) must be wired to acquire/commit/replay leases
//! before command_idempotency_contract_v1 is considered fully implemented.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

/// A row from the command_idempotency table.
#[derive(Debug, Clone)]
pub struct CommandIdempotencyLease {
    pub principal_id: String,
    pub request_id: String,
    pub command: String,
    pub intent_hash: String,
    pub lease_generation: i64,
    pub lease_state: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub committed_at: Option<String>,
    pub outcome_json: Option<String>,
    pub failure_code: Option<String>,
}

/// A row from the command_request_aliases table.
#[derive(Debug, Clone)]
pub struct CommandRequestAlias {
    pub principal_id: String,
    pub command: String,
    pub intent_hash: String,
    pub request_id: String,
    pub canonical_request_id: String,
    pub created_at: String,
}

fn map_lease(r: sqlx::sqlite::SqliteRow) -> CommandIdempotencyLease {
    CommandIdempotencyLease {
        principal_id: r.get("principal_id"),
        request_id: r.get("request_id"),
        command: r.get("command"),
        intent_hash: r.get("intent_hash"),
        lease_generation: r.get("lease_generation"),
        lease_state: r.get("lease_state"),
        acquired_at: r.get("acquired_at"),
        expires_at: r.get("expires_at"),
        committed_at: r.get("committed_at"),
        outcome_json: r.get("outcome_json"),
        failure_code: r.get("failure_code"),
    }
}

const LEASE_SELECT: &str = r#"SELECT principal_id, request_id, command, intent_hash,
       lease_generation, lease_state, acquired_at, expires_at,
       committed_at, outcome_json, failure_code
  FROM command_idempotency"#;

/// Find the active (pending/committed/failed) lease for a (principal_id, request_id) pair.
pub async fn find_active_by_request(
    pool: &SqlitePool,
    principal_id: &str,
    request_id: &str,
) -> Result<Option<CommandIdempotencyLease>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{LEASE_SELECT} WHERE principal_id = ?1 AND request_id = ?2
         AND lease_state IN ('pending','committed','failed')"
    )))
    .bind(principal_id)
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .context("command_idempotency: find_active_by_request")?;
    Ok(row.map(map_lease))
}

/// Find the active committed lease for a (principal_id, command, intent_hash).
/// Returns None if no committed lease exists for this intent.
pub async fn find_committed_by_intent(
    pool: &SqlitePool,
    principal_id: &str,
    command: &str,
    intent_hash: &str,
) -> Result<Option<CommandIdempotencyLease>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{LEASE_SELECT} WHERE principal_id = ?1 AND command = ?2
         AND intent_hash = ?3 AND lease_state = 'committed'"
    )))
    .bind(principal_id)
    .bind(command)
    .bind(intent_hash)
    .fetch_optional(pool)
    .await
    .context("command_idempotency: find_committed_by_intent")?;
    Ok(row.map(map_lease))
}

/// Find the most recent failed terminal lease for a (principal_id, command, intent_hash).
///
/// Per P083-HARDEN-007 (failed-terminal retry policy): acquisition must check this before
/// allowing a new same-intent lease for commands whose retry_allowed is Never.
/// Returns None if no failed lease exists for this intent.
pub async fn find_failed_by_intent(
    pool: &SqlitePool,
    principal_id: &str,
    command: &str,
    intent_hash: &str,
) -> Result<Option<CommandIdempotencyLease>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{LEASE_SELECT} WHERE principal_id = ?1 AND command = ?2
         AND intent_hash = ?3 AND lease_state = 'failed'
         ORDER BY lease_generation DESC LIMIT 1"
    )))
    .bind(principal_id)
    .bind(command)
    .bind(intent_hash)
    .fetch_optional(pool)
    .await
    .context("command_idempotency: find_failed_by_intent")?;
    Ok(row.map(map_lease))
}

/// Acquire a new pending lease. Uses INSERT OR FAIL so concurrent races
/// surface as a unique constraint error (IDEMPOTENCY_IN_FLIGHT).
/// Returns Ok(true) on success, Ok(false) on unique constraint violation.
pub async fn acquire(
    pool: &SqlitePool,
    principal_id: &str,
    request_id: &str,
    command: &str,
    intent_hash: &str,
    lease_generation: i64,
    expires_at: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT OR FAIL INTO command_idempotency
           (principal_id, request_id, command, intent_hash,
            lease_generation, lease_state, acquired_at, expires_at)
           VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(command)
    .bind(intent_hash)
    .bind(lease_generation)
    .bind(&now)
    .bind(expires_at)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e).context("command_idempotency: acquire"),
    }
}

/// Acquire inside a caller-owned transaction.
pub async fn acquire_tx(
    tx: &mut Transaction<'_, Sqlite>,
    principal_id: &str,
    request_id: &str,
    command: &str,
    intent_hash: &str,
    lease_generation: i64,
    expires_at: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT OR FAIL INTO command_idempotency
           (principal_id, request_id, command, intent_hash,
            lease_generation, lease_state, acquired_at, expires_at)
           VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(command)
    .bind(intent_hash)
    .bind(lease_generation)
    .bind(&now)
    .bind(expires_at)
    .execute(&mut **tx)
    .await;

    match result {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e).context("command_idempotency: acquire_tx"),
    }
}

/// Commit a pending lease with the terminal outcome JSON.
/// Only updates rows still in 'pending' state to prevent double-commit races.
pub async fn commit(
    pool: &SqlitePool,
    principal_id: &str,
    request_id: &str,
    lease_generation: i64,
    outcome_json: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'committed', committed_at = ?1, outcome_json = ?2
           WHERE principal_id = ?3 AND request_id = ?4
             AND lease_generation = ?5 AND lease_state = 'pending'"#,
    )
    .bind(&now)
    .bind(outcome_json)
    .bind(principal_id)
    .bind(request_id)
    .bind(lease_generation)
    .execute(pool)
    .await
    .context("command_idempotency: commit")?;
    Ok(rows.rows_affected() > 0)
}

/// Commit a pending lease inside a caller-owned transaction.
/// Per command_idempotency_contract_v1 transaction_rule: acquisition, side-effect writes,
/// and terminal outcome commit must happen in one SQLite transaction.
pub async fn commit_tx(
    tx: &mut Transaction<'_, Sqlite>,
    principal_id: &str,
    request_id: &str,
    lease_generation: i64,
    outcome_json: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'committed', committed_at = ?1, outcome_json = ?2
           WHERE principal_id = ?3 AND request_id = ?4
             AND lease_generation = ?5 AND lease_state = 'pending'"#,
    )
    .bind(&now)
    .bind(outcome_json)
    .bind(principal_id)
    .bind(request_id)
    .bind(lease_generation)
    .execute(&mut **tx)
    .await
    .context("command_idempotency: commit_tx")?;
    Ok(rows.rows_affected() > 0)
}

/// Mark a pending lease as failed with a failure_code.
pub async fn fail_lease(
    pool: &SqlitePool,
    principal_id: &str,
    request_id: &str,
    lease_generation: i64,
    failure_code: &str,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'failed', failure_code = ?1
           WHERE principal_id = ?2 AND request_id = ?3
             AND lease_generation = ?4 AND lease_state = 'pending'"#,
    )
    .bind(failure_code)
    .bind(principal_id)
    .bind(request_id)
    .bind(lease_generation)
    .execute(pool)
    .await
    .context("command_idempotency: fail_lease")?;
    Ok(rows.rows_affected() > 0)
}

/// Mark a pending lease as failed inside a caller-owned transaction.
/// Per command_idempotency_contract_v1 transaction_rule: failure must be recorded atomically
/// with the command journal fail entry in one SQLite transaction.
pub async fn fail_lease_tx(
    tx: &mut Transaction<'_, Sqlite>,
    principal_id: &str,
    request_id: &str,
    lease_generation: i64,
    failure_code: &str,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'failed', failure_code = ?1
           WHERE principal_id = ?2 AND request_id = ?3
             AND lease_generation = ?4 AND lease_state = 'pending'"#,
    )
    .bind(failure_code)
    .bind(principal_id)
    .bind(request_id)
    .bind(lease_generation)
    .execute(&mut **tx)
    .await
    .context("command_idempotency: fail_lease_tx")?;
    Ok(rows.rows_affected() > 0)
}

/// Abandon a lease (move to 'abandoned'). Used for TTL expiry or explicit cancel.
/// May update pending, failed, or committed rows.
pub async fn abandon(
    pool: &SqlitePool,
    principal_id: &str,
    request_id: &str,
    lease_generation: i64,
) -> Result<bool> {
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'abandoned'
           WHERE principal_id = ?1 AND request_id = ?2
             AND lease_generation = ?3 AND lease_state != 'abandoned'"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(lease_generation)
    .execute(pool)
    .await
    .context("command_idempotency: abandon")?;
    Ok(rows.rows_affected() > 0)
}

/// Expire pending leases whose expires_at is before now.
/// Returns the number of leases moved to 'abandoned'.
pub async fn expire_stale(pool: &SqlitePool) -> Result<u64> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'abandoned'
           WHERE lease_state = 'pending' AND expires_at < ?1"#,
    )
    .bind(&now)
    .execute(pool)
    .await
    .context("command_idempotency: expire_stale")?;
    Ok(rows.rows_affected())
}

/// CAS reacquire for an expired pending lease.
/// Per command_idempotency_contract_v1 recovery rule "pending_expired_no_side_effect_receipt":
/// mark the prior generation abandoned and acquire generation+1 in one transaction.
///
/// Returns Ok(Some(new_generation)) if the expired pending lease was found and reacquired.
/// Returns Ok(None) if no expired pending lease exists for (principal_id, request_id).
/// The caller must verify `expires_at < now` before calling this — the function rechecks
/// inside the transaction to avoid TOCTOU races, but the caller should pre-filter.
pub async fn reacquire_expired_tx(
    tx: &mut Transaction<'_, Sqlite>,
    principal_id: &str,
    request_id: &str,
    command: &str,
    intent_hash: &str,
    new_expires_at: &str,
) -> Result<Option<i64>> {
    let now = Utc::now().to_rfc3339();

    // Find the pending expired row — must re-check lease_state and expires_at atomically
    // inside the transaction to prevent concurrent reacquire races.
    let row: Option<i64> = sqlx::query_scalar(
        r#"SELECT lease_generation FROM command_idempotency
           WHERE principal_id = ?1 AND request_id = ?2
             AND lease_state = 'pending' AND expires_at < ?3
           LIMIT 1"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(&now)
    .fetch_optional(&mut **tx)
    .await
    .context("command_idempotency: reacquire_expired_tx select")?;

    let Some(old_generation) = row else {
        return Ok(None);
    };

    // Mark prior generation abandoned.
    sqlx::query(
        r#"UPDATE command_idempotency
           SET lease_state = 'abandoned'
           WHERE principal_id = ?1 AND request_id = ?2
             AND lease_generation = ?3 AND lease_state = 'pending'"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(old_generation)
    .execute(&mut **tx)
    .await
    .context("command_idempotency: reacquire_expired_tx abandon")?;

    let new_generation = old_generation + 1;
    let acquired_at = Utc::now().to_rfc3339();

    // Insert new pending row with incremented generation.
    let result = sqlx::query(
        r#"INSERT OR FAIL INTO command_idempotency
           (principal_id, request_id, command, intent_hash,
            lease_generation, lease_state, acquired_at, expires_at)
           VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)"#,
    )
    .bind(principal_id)
    .bind(request_id)
    .bind(command)
    .bind(intent_hash)
    .bind(new_generation)
    .bind(&acquired_at)
    .bind(new_expires_at)
    .execute(&mut **tx)
    .await;

    match result {
        Ok(_) => Ok(Some(new_generation)),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            anyhow::bail!(
                "command_idempotency: reacquire_expired_tx: unique violation on generation {}",
                new_generation
            )
        }
        Err(e) => Err(e).context("command_idempotency: reacquire_expired_tx insert"),
    }
}

// ── request aliases ──────────────────────────────────────────────────────────

/// Insert a command_request_aliases row mapping a replacement request_id
/// to the canonical_request_id for same-intent replay.
pub async fn insert_alias(
    pool: &SqlitePool,
    principal_id: &str,
    command: &str,
    intent_hash: &str,
    request_id: &str,
    canonical_request_id: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT OR IGNORE INTO command_request_aliases
           (principal_id, command, intent_hash, request_id, canonical_request_id, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
    )
    .bind(principal_id)
    .bind(command)
    .bind(intent_hash)
    .bind(request_id)
    .bind(canonical_request_id)
    .bind(&now)
    .execute(pool)
    .await
    .context("command_idempotency: insert_alias")?;
    Ok(())
}

/// Look up the canonical_request_id for a given alias.
pub async fn find_canonical_by_alias(
    pool: &SqlitePool,
    principal_id: &str,
    command: &str,
    intent_hash: &str,
    request_id: &str,
) -> Result<Option<String>> {
    let row: Option<String> = sqlx::query_scalar(
        r#"SELECT canonical_request_id FROM command_request_aliases
           WHERE principal_id = ?1 AND command = ?2
             AND intent_hash = ?3 AND request_id = ?4"#,
    )
    .bind(principal_id)
    .bind(command)
    .bind(intent_hash)
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .context("command_idempotency: find_canonical_by_alias")?;
    Ok(row)
}
