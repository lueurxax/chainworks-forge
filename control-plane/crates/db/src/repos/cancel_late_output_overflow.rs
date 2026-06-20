//! P083 cancel_late_output_overflow repository.
//!
//! Records overflow latch activations when provider output arrives after
//! cancellation. Per post_cancel_late_output_contract_v1:
//! - unique latch key: (scope, normalized_run_id, normalized_provider_session_id,
//!   cancellation_epoch, overflow_kind)
//! - restart_idempotency: INSERT OR IGNORE to avoid duplicate rows; updates
//!   counter columns on existing rows via a follow-up UPDATE.
//! - projection_mutation_blocked=1 confirms active projections are not mutated.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tracing;
use uuid::Uuid;

// ── P083 post_cancel_late_output_contract_v1 cap bounds ────────────────────

/// Maximum message count per session scope before latching the overflow.
pub const CAP_SESSION_MESSAGE_COUNT: i64 = 200;
/// Maximum bytes per session scope.
pub const CAP_SESSION_BYTES: i64 = 1_048_576;
/// Maximum bytes per run scope.
pub const CAP_RUN_BYTES: i64 = 8_388_608;
/// Maximum bytes at global scope.
pub const CAP_GLOBAL_BYTES: i64 = 67_108_864;
/// Maximum elapsed time in milliseconds for any session scope.
pub const CAP_ELAPSED_TIME_MS: i64 = 300_000;

/// Outcome of a late-output cap check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapCheckOutcome {
    /// Counter is within bounds; no latch needed.
    WithinBounds,
    /// Counter has reached or exceeded the cap; latch should be activated.
    ExceedsCapacity {
        overflow_kind: &'static str,
        cap: i64,
    },
}

/// Check whether the running counter for an overflow kind would exceed the cap.
/// Per post_cancel_late_output_contract_v1.cap_bounds.
///
/// `scope` must be one of "session", "run", or "global".
/// `overflow_kind` must be one of the metric_labels_contract_v1.overflow_kind values.
/// `running_count` is the value about to be accumulated (messages or bytes).
/// `running_elapsed_ms` is total elapsed ms for elapsed_time scope checks.
pub fn check_cap(
    scope: &str,
    overflow_kind: &str,
    running_count: i64,
    running_elapsed_ms: Option<i64>,
) -> CapCheckOutcome {
    let cap = match (scope, overflow_kind) {
        ("session", "message_count") => CAP_SESSION_MESSAGE_COUNT,
        ("session", "session_bytes") => CAP_SESSION_BYTES,
        ("session", "elapsed_time") => {
            return match running_elapsed_ms {
                Some(ms) if ms >= CAP_ELAPSED_TIME_MS => CapCheckOutcome::ExceedsCapacity {
                    overflow_kind: "elapsed_time",
                    cap: CAP_ELAPSED_TIME_MS,
                },
                _ => CapCheckOutcome::WithinBounds,
            }
        }
        ("run", "run_bytes") => CAP_RUN_BYTES,
        ("global", "global_bytes") => CAP_GLOBAL_BYTES,
        _ => return CapCheckOutcome::WithinBounds,
    };
    if running_count >= cap {
        CapCheckOutcome::ExceedsCapacity {
            overflow_kind: match overflow_kind {
                "message_count" => "message_count",
                "session_bytes" => "session_bytes",
                "run_bytes" => "run_bytes",
                "global_bytes" => "global_bytes",
                other => {
                    tracing::warn!("cap_check: unknown overflow_kind {other:?}; treating as within bounds");
                    return CapCheckOutcome::WithinBounds;
                }
            },
            cap,
        }
    } else {
        CapCheckOutcome::WithinBounds
    }
}

/// Insert or update a session-scope late-output overflow latch.
///
/// Uses INSERT OR IGNORE followed by counter UPDATE for restart idempotency.
/// Per post_cancel_late_output_contract_v1: the latch key is
/// (scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind).
///
/// `run_id` and `provider_session_id` may be None for global-scope latches.
pub async fn record_overflow_latch(
    pool: &SqlitePool,
    scope: &str,
    run_id: Option<&str>,
    provider_session_id: Option<&str>,
    cancellation_epoch: i64,
    overflow_kind: &str,
    dropped_message_count: i64,
    dropped_byte_count: i64,
) -> Result<()> {
    let overflow_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // INSERT OR IGNORE: idempotent across restarts — same latch key is silent no-op.
    sqlx::query(
        r#"INSERT OR IGNORE INTO cancel_late_output_overflow
           (overflow_id, scope, run_id, provider_session_id, cancellation_epoch,
            overflow_kind, dropped_message_count, dropped_byte_count,
            projection_mutation_blocked, latched_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)"#,
    )
    .bind(&overflow_id)
    .bind(scope)
    .bind(run_id)
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .bind(overflow_kind)
    .bind(dropped_message_count)
    .bind(dropped_byte_count)
    .bind(&now)
    .execute(pool)
    .await
    .context("cancel_late_output_overflow: INSERT OR IGNORE")?;

    // UPDATE counter columns on existing row (restart may add more late outputs).
    sqlx::query(
        r#"UPDATE cancel_late_output_overflow
           SET dropped_message_count = dropped_message_count + ?1,
               dropped_byte_count    = dropped_byte_count    + ?2,
               projection_mutation_blocked = 1,
               updated_at = ?3
           WHERE scope = ?4
             AND LOWER(TRIM(COALESCE(run_id, ''))) = LOWER(TRIM(COALESCE(?5, '')))
             AND LOWER(TRIM(COALESCE(provider_session_id, ''))) = LOWER(TRIM(COALESCE(?6, '')))
             AND cancellation_epoch = ?7
             AND overflow_kind = ?8
             AND overflow_id <> ?9"#,
    )
    .bind(dropped_message_count)
    .bind(dropped_byte_count)
    .bind(&now)
    .bind(scope)
    .bind(run_id)
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .bind(overflow_kind)
    .bind(&overflow_id)
    .execute(pool)
    .await
    .context("cancel_late_output_overflow: UPDATE counters")?;

    Ok(())
}

/// Transactional variant: insert or update a session-scope late-output overflow
/// latch as part of the caller's open write transaction so the latch row commits
/// atomically with the runtime facts and ignored-output decisions. Used by the
/// executor to make P083 post_cancel_late_output_contract_v1 a durable invariant
/// rather than a best-effort post-commit write.
pub async fn record_overflow_latch_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scope: &str,
    run_id: Option<&str>,
    provider_session_id: Option<&str>,
    cancellation_epoch: i64,
    overflow_kind: &str,
    dropped_message_count: i64,
    dropped_byte_count: i64,
) -> Result<()> {
    let overflow_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT OR IGNORE INTO cancel_late_output_overflow
           (overflow_id, scope, run_id, provider_session_id, cancellation_epoch,
            overflow_kind, dropped_message_count, dropped_byte_count,
            projection_mutation_blocked, latched_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)"#,
    )
    .bind(&overflow_id)
    .bind(scope)
    .bind(run_id)
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .bind(overflow_kind)
    .bind(dropped_message_count)
    .bind(dropped_byte_count)
    .bind(&now)
    .execute(&mut **tx)
    .await
    .context("cancel_late_output_overflow: INSERT OR IGNORE (tx)")?;

    sqlx::query(
        r#"UPDATE cancel_late_output_overflow
           SET dropped_message_count = dropped_message_count + ?1,
               dropped_byte_count    = dropped_byte_count    + ?2,
               projection_mutation_blocked = 1,
               updated_at = ?3
           WHERE scope = ?4
             AND LOWER(TRIM(COALESCE(run_id, ''))) = LOWER(TRIM(COALESCE(?5, '')))
             AND LOWER(TRIM(COALESCE(provider_session_id, ''))) = LOWER(TRIM(COALESCE(?6, '')))
             AND cancellation_epoch = ?7
             AND overflow_kind = ?8
             AND overflow_id <> ?9"#,
    )
    .bind(dropped_message_count)
    .bind(dropped_byte_count)
    .bind(&now)
    .bind(scope)
    .bind(run_id)
    .bind(provider_session_id)
    .bind(cancellation_epoch)
    .bind(overflow_kind)
    .bind(&overflow_id)
    .execute(&mut **tx)
    .await
    .context("cancel_late_output_overflow: UPDATE counters (tx)")?;

    Ok(())
}

/// Look up the most recent cancellation_epoch for a provider session.
/// Returns 0 if no cancellation intent exists (treated as epoch 0 latch).
pub async fn latest_cancellation_epoch(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<i64> {
    let epoch: Option<i64> = sqlx::query_scalar(
        r#"SELECT cancellation_epoch
             FROM provider_cancellation_intents
            WHERE provider_session_id = ?1
            ORDER BY cancellation_epoch DESC
            LIMIT 1"#,
    )
    .bind(provider_session_id)
    .fetch_optional(pool)
    .await
    .context("cancel_late_output_overflow: latest_cancellation_epoch")?;
    Ok(epoch.unwrap_or(0))
}

/// Transactional variant of `latest_cancellation_epoch` for in-transaction reads.
pub async fn latest_cancellation_epoch_tx(
    tx: &mut Transaction<'_, Sqlite>,
    provider_session_id: &str,
) -> Result<i64> {
    let epoch: Option<i64> = sqlx::query_scalar(
        r#"SELECT cancellation_epoch
             FROM provider_cancellation_intents
            WHERE provider_session_id = ?1
            ORDER BY cancellation_epoch DESC
            LIMIT 1"#,
    )
    .bind(provider_session_id)
    .fetch_optional(&mut **tx)
    .await
    .context("cancel_late_output_overflow: latest_cancellation_epoch (tx)")?;
    Ok(epoch.unwrap_or(0))
}

/// Look up the provider_session_id for a session generation.
pub async fn provider_session_id_for_generation(
    pool: &SqlitePool,
    session_generation_id: &str,
) -> Result<Option<String>> {
    let id: Option<String> = sqlx::query_scalar(
        r#"SELECT provider_session_id
             FROM session_generations
            WHERE id = ?1"#,
    )
    .bind(session_generation_id)
    .fetch_optional(pool)
    .await
    .context("cancel_late_output_overflow: provider_session_id_for_generation")?;
    Ok(id)
}

/// Transactional variant of `provider_session_id_for_generation` for atomic latch writes.
pub async fn provider_session_id_for_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    session_generation_id: &str,
) -> Result<Option<String>> {
    let id: Option<String> = sqlx::query_scalar(
        r#"SELECT provider_session_id
             FROM session_generations
            WHERE id = ?1"#,
    )
    .bind(session_generation_id)
    .fetch_optional(&mut **tx)
    .await
    .context("cancel_late_output_overflow: provider_session_id_for_generation (tx)")?;
    Ok(id)
}

/// A readback row from cancel_late_output_overflow.
/// Per post_cancel_late_output_contract_v1.readback_fields.
#[derive(Debug, Clone)]
pub struct CancelLateOutputOverflowRow {
    pub overflow_id: String,
    pub scope: String,
    pub run_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub cancellation_epoch: i64,
    pub overflow_kind: String,
    pub dropped_message_count: i64,
    pub dropped_byte_count: i64,
    pub quarantine_uri: Option<String>,
    pub reservation_release_state: String,
    pub projection_mutation_blocked: bool,
    pub normalized_run_id: String,
    pub normalized_provider_session_id: String,
    pub latched_at: String,
}

fn map_overflow_row(row: sqlx::sqlite::SqliteRow) -> CancelLateOutputOverflowRow {
    use sqlx::Row;
    let pmb: i64 = row.get("projection_mutation_blocked");
    CancelLateOutputOverflowRow {
        overflow_id: row.get("overflow_id"),
        scope: row.get("scope"),
        run_id: row.get("run_id"),
        provider_session_id: row.get("provider_session_id"),
        cancellation_epoch: row.get("cancellation_epoch"),
        overflow_kind: row.get("overflow_kind"),
        dropped_message_count: row.get("dropped_message_count"),
        dropped_byte_count: row.get("dropped_byte_count"),
        quarantine_uri: row.get("quarantine_uri"),
        reservation_release_state: row.get("reservation_release_state"),
        projection_mutation_blocked: pmb != 0,
        normalized_run_id: row.get("normalized_run_id"),
        normalized_provider_session_id: row.get("normalized_provider_session_id"),
        latched_at: row.get("latched_at"),
    }
}

/// Find all cancel_late_output_overflow rows for a run by run_id.
/// Used for GraphQL/MCP readback per post_cancel_late_output_contract_v1.
pub async fn find_by_run_id(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<CancelLateOutputOverflowRow>> {
    let rows = sqlx::query(
        r#"SELECT overflow_id, scope, run_id, provider_session_id,
                  cancellation_epoch, overflow_kind, dropped_message_count,
                  dropped_byte_count, quarantine_uri, reservation_release_state,
                  projection_mutation_blocked, normalized_run_id,
                  normalized_provider_session_id, latched_at
           FROM cancel_late_output_overflow
           WHERE run_id = ?1
           ORDER BY latched_at"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("cancel_late_output_overflow: find_by_run_id")?;
    Ok(rows.into_iter().map(map_overflow_row).collect())
}

/// Find all cancel_late_output_overflow rows for a provider session.
/// Used for session-scoped readback.
pub async fn find_by_provider_session_id(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Result<Vec<CancelLateOutputOverflowRow>> {
    let rows = sqlx::query(
        r#"SELECT overflow_id, scope, run_id, provider_session_id,
                  cancellation_epoch, overflow_kind, dropped_message_count,
                  dropped_byte_count, quarantine_uri, reservation_release_state,
                  projection_mutation_blocked, normalized_run_id,
                  normalized_provider_session_id, latched_at
           FROM cancel_late_output_overflow
           WHERE provider_session_id = ?1
           ORDER BY latched_at"#,
    )
    .bind(provider_session_id)
    .fetch_all(pool)
    .await
    .context("cancel_late_output_overflow: find_by_provider_session_id")?;
    Ok(rows.into_iter().map(map_overflow_row).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_within_bounds_message_count() {
        assert_eq!(
            check_cap("session", "message_count", 199, None),
            CapCheckOutcome::WithinBounds
        );
    }

    #[test]
    fn cap_at_limit_message_count_exceeds() {
        assert_eq!(
            check_cap("session", "message_count", 200, None),
            CapCheckOutcome::ExceedsCapacity {
                overflow_kind: "message_count",
                cap: CAP_SESSION_MESSAGE_COUNT
            }
        );
    }

    #[test]
    fn cap_session_bytes_within_bounds() {
        assert_eq!(
            check_cap("session", "session_bytes", 1_048_575, None),
            CapCheckOutcome::WithinBounds
        );
    }

    #[test]
    fn cap_session_bytes_exceeds() {
        assert_eq!(
            check_cap("session", "session_bytes", 1_048_576, None),
            CapCheckOutcome::ExceedsCapacity {
                overflow_kind: "session_bytes",
                cap: CAP_SESSION_BYTES
            }
        );
    }

    #[test]
    fn cap_run_bytes_exceeds() {
        assert_eq!(
            check_cap("run", "run_bytes", 8_388_608, None),
            CapCheckOutcome::ExceedsCapacity {
                overflow_kind: "run_bytes",
                cap: CAP_RUN_BYTES
            }
        );
    }

    #[test]
    fn cap_global_bytes_exceeds() {
        assert_eq!(
            check_cap("global", "global_bytes", 67_108_864, None),
            CapCheckOutcome::ExceedsCapacity {
                overflow_kind: "global_bytes",
                cap: CAP_GLOBAL_BYTES
            }
        );
    }

    #[test]
    fn cap_elapsed_time_within_bounds() {
        assert_eq!(
            check_cap("session", "elapsed_time", 0, Some(299_999)),
            CapCheckOutcome::WithinBounds
        );
    }

    #[test]
    fn cap_elapsed_time_exceeds() {
        let outcome = check_cap("session", "elapsed_time", 0, Some(300_000));
        assert_eq!(
            outcome,
            CapCheckOutcome::ExceedsCapacity {
                overflow_kind: "elapsed_time",
                cap: CAP_ELAPSED_TIME_MS
            }
        );
    }

    #[test]
    fn cap_unknown_scope_within_bounds() {
        assert_eq!(
            check_cap("unknown_scope", "message_count", 99999, None),
            CapCheckOutcome::WithinBounds
        );
    }
}
