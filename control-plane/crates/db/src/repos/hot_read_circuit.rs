use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum CircuitStatus {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "closed" => Ok(Self::Closed),
            "open" => Ok(Self::Open),
            "half_open" => Ok(Self::HalfOpen),
            _ => Err(anyhow::anyhow!("unknown circuit status: {}", s)),
        }
    }
}

pub async fn get_circuit_state(
    pool: &SqlitePool,
    governed_surface: &str,
) -> Result<(CircuitStatus, i32, i32, Option<i64>, Option<i64>, bool)> {
    let row = sqlx::query(
        "SELECT circuit_status, consecutive_successes, consecutive_failures, last_opened_at_ms, retry_after_ms, would_open FROM hot_read_circuit_states WHERE governed_surface = ?"
    )
    .bind(governed_surface)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let status: String = row.get("circuit_status");
        let successes: i32 = row.get("consecutive_successes");
        let failures: i32 = row.get("consecutive_failures");
        let last_opened: Option<i64> = row.get("last_opened_at_ms");
        let retry_after: Option<i64> = row.get("retry_after_ms");
        let would_open: i32 = row.get("would_open");
        Ok((
            CircuitStatus::from_str(&status)?,
            successes,
            failures,
            last_opened,
            retry_after,
            would_open != 0,
        ))
    } else {
        Ok((CircuitStatus::Closed, 0, 0, None, None, false))
    }
}

/// Returns (total_requests, total_would_open, last_state_change_at_ms, first_observed_at_ms) for
/// observe-to-enforce promotion budget evaluation.
pub async fn get_promotion_budget(
    pool: &SqlitePool,
    governed_surface: &str,
) -> Result<(i64, i64, Option<i64>, Option<i64>)> {
    let row = sqlx::query(
        "SELECT total_requests, total_would_open, last_state_change_at_ms, first_observed_at_ms FROM hot_read_circuit_states WHERE governed_surface = ?"
    )
    .bind(governed_surface)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        Ok((
            row.get::<i64, _>("total_requests"),
            row.get::<i64, _>("total_would_open"),
            row.get::<Option<i64>, _>("last_state_change_at_ms"),
            row.get::<Option<i64>, _>("first_observed_at_ms"),
        ))
    } else {
        Ok((0, 0, None, None))
    }
}

pub async fn record_violation(
    pool: &SqlitePool,
    governed_surface: &str,
    violation_kind: &str,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let violation_kind = public_violation_kind(violation_kind);
    let (status, _, failures, _, _, _) = get_circuit_state(pool, governed_surface).await?;

    let next_failures = failures + 1;
    let (next_status, next_last_opened, next_retry_after, circuit_opened) = if next_failures >= 3 {
        // Open circuit and set backoff (30s)
        crate::metrics::increment_counter("hot_read_circuit_open_total");
        (CircuitStatus::Open, Some(now), Some(now + 30_000), true)
    } else {
        (status, None, None, false)
    };

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, last_violation_kind, last_opened_at_ms, retry_after_ms, would_open, total_requests, last_state_change_at_ms, first_observed_at_ms, updated_at_ms)
         VALUES (?, ?, 0, ?, ?, ?, ?, 0, 1, ?, ?, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET
            circuit_status = excluded.circuit_status,
            consecutive_successes = 0,
            consecutive_failures = excluded.consecutive_failures,
            last_violation_kind = excluded.last_violation_kind,
            last_opened_at_ms = COALESCE(excluded.last_opened_at_ms, last_opened_at_ms),
            retry_after_ms = excluded.retry_after_ms,
            would_open = 0,
            total_requests = total_requests + 1,
            last_state_change_at_ms = CASE WHEN excluded.circuit_status != circuit_status THEN excluded.last_state_change_at_ms ELSE last_state_change_at_ms END,
            first_observed_at_ms = COALESCE(first_observed_at_ms, excluded.first_observed_at_ms),
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(next_status.as_str())
    .bind(next_failures)
    .bind(violation_kind)
    .bind(next_last_opened)
    .bind(next_retry_after)
    .bind(if circuit_opened { Some(now) } else { None::<i64> })
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_success(pool: &SqlitePool, governed_surface: &str) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let (status, successes, _, _, _, _) = get_circuit_state(pool, governed_surface).await?;

    let (new_status, new_successes) = match status {
        CircuitStatus::HalfOpen => {
            if successes + 1 >= 3 {
                (CircuitStatus::Closed, 0)
            } else {
                (CircuitStatus::HalfOpen, successes + 1)
            }
        }
        CircuitStatus::Closed => (CircuitStatus::Closed, successes + 1),
        CircuitStatus::Open => (CircuitStatus::HalfOpen, 1),
    };

    let state_changed = new_status != status;

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, retry_after_ms, would_open, total_requests, last_state_change_at_ms, first_observed_at_ms, updated_at_ms)
         VALUES (?, ?, ?, 0, NULL, 0, 1, ?, ?, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET
            circuit_status = excluded.circuit_status,
            consecutive_successes = excluded.consecutive_successes,
            consecutive_failures = 0,
            retry_after_ms = NULL,
            would_open = 0,
            total_requests = total_requests + 1,
            last_state_change_at_ms = CASE WHEN excluded.circuit_status != circuit_status THEN excluded.last_state_change_at_ms ELSE last_state_change_at_ms END,
            first_observed_at_ms = COALESCE(first_observed_at_ms, excluded.first_observed_at_ms),
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(new_status.as_str())
    .bind(new_successes)
    .bind(if state_changed { Some(now) } else { None::<i64> })
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_would_open(
    pool: &SqlitePool,
    governed_surface: &str,
    violation_kind: &str,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let violation_kind = public_violation_kind(violation_kind);
    let (_status, _, failures, _, _, _) = get_circuit_state(pool, governed_surface).await?;

    let next_failures = failures + 1;
    let (next_would_open, would_open_increment) = if next_failures >= 3 {
        crate::metrics::increment_counter("hot_read_circuit_would_open_total");
        (1i32, 1i64)
    } else {
        (0, 0)
    };

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, last_violation_kind, would_open, total_requests, total_would_open, first_observed_at_ms, updated_at_ms)
         VALUES (?, 'closed', 0, ?, ?, ?, 1, ?, ?, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET
            consecutive_successes = 0,
            consecutive_failures = excluded.consecutive_failures,
            last_violation_kind = excluded.last_violation_kind,
            would_open = excluded.would_open,
            total_requests = total_requests + 1,
            total_would_open = total_would_open + excluded.total_would_open,
            first_observed_at_ms = COALESCE(first_observed_at_ms, excluded.first_observed_at_ms),
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(next_failures)
    .bind(violation_kind)
    .bind(next_would_open)
    .bind(would_open_increment)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub fn public_violation_kind(raw: &str) -> &'static str {
    match raw {
        "timeout" | "hot_read_timeout" => "timeout",
        "busy" | "database_busy" | "sqlite_busy" => "busy",
        "unavailable" | "storage_unavailable" => "unavailable",
        "stale" | "storage_stale" => "stale",
        "projection_unavailable" => "projection_unavailable",
        "hot_read_circuit_open" => "hot_read_circuit_open",
        _ => {
            let lower = raw.to_ascii_lowercase();
            if lower.contains("timeout") {
                "timeout"
            } else if lower.contains("busy") {
                "busy"
            } else if lower.contains("unavailable") {
                "unavailable"
            } else {
                "unavailable"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn proposal_087_promotion_budget_tracks_total_requests_and_would_open() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let surface = "runs.list";

        // Initial state: no data
        let (total, would_open, last_change, first_observed) =
            get_promotion_budget(&pool, surface).await.unwrap();
        assert_eq!(total, 0);
        assert_eq!(would_open, 0);
        assert!(last_change.is_none());
        assert!(first_observed.is_none());

        // Record some successes; first_observed_at_ms should be set on first insert
        record_success(&pool, surface).await.unwrap();
        record_success(&pool, surface).await.unwrap();
        let (total, would_open, _, first_observed) =
            get_promotion_budget(&pool, surface).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(would_open, 0);
        assert!(
            first_observed.is_some(),
            "first_observed_at_ms must be set after first record_success"
        );

        // Record would_open violations (observe mode)
        record_would_open(&pool, surface, "timeout").await.unwrap();
        record_would_open(&pool, surface, "timeout").await.unwrap();
        record_would_open(&pool, surface, "timeout").await.unwrap();
        let (total, would_open_count, _, _) = get_promotion_budget(&pool, surface).await.unwrap();
        // 3 would_open violations should have set would_open on the 3rd
        assert!(
            total >= 5,
            "total_requests should include would_open events: {total}"
        );
        assert_eq!(
            would_open_count, 1,
            "third would_open should increment counter"
        );
    }

    #[tokio::test]
    async fn proposal_087_hot_read_violation_kind_never_persists_raw_error_text() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let raw = "timeout opening /Users/user/private/control-plane.db with token sk-secret";

        record_violation(&pool, "storage.health", raw)
            .await
            .unwrap();

        let stored: String = sqlx::query_scalar(
            "SELECT last_violation_kind FROM hot_read_circuit_states WHERE governed_surface = 'storage.health'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored, "timeout");
        assert!(!stored.contains("/Users/user"));
        assert!(!stored.contains("sk-secret"));
    }
}
