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

pub async fn record_violation(
    pool: &SqlitePool,
    governed_surface: &str,
    violation_kind: &str,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let (status, _, failures, _, _, _) = get_circuit_state(pool, governed_surface).await?;

    let next_failures = failures + 1;
    let (next_status, next_last_opened, next_retry_after) = if next_failures >= 3 {
        // Open circuit and set backoff (30s)
        crate::metrics::increment_counter("hot_read_circuit_open_total");
        (CircuitStatus::Open, Some(now), Some(now + 30_000))
    } else {
        (status, None, None)
    };

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, last_violation_kind, last_opened_at_ms, retry_after_ms, would_open, updated_at_ms)
         VALUES (?, ?, 0, ?, ?, ?, ?, 0, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET 
            circuit_status = excluded.circuit_status, 
            consecutive_successes = 0,
            consecutive_failures = excluded.consecutive_failures,
            last_violation_kind = excluded.last_violation_kind,
            last_opened_at_ms = COALESCE(excluded.last_opened_at_ms, last_opened_at_ms),
            retry_after_ms = excluded.retry_after_ms,
            would_open = 0,
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(next_status.as_str())
    .bind(next_failures)
    .bind(violation_kind)
    .bind(next_last_opened)
    .bind(next_retry_after)
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

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, retry_after_ms, would_open, updated_at_ms)
         VALUES (?, ?, ?, 0, NULL, 0, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET 
            circuit_status = excluded.circuit_status, 
            consecutive_successes = excluded.consecutive_successes,
            consecutive_failures = 0,
            retry_after_ms = NULL,
            would_open = 0,
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(new_status.as_str())
    .bind(new_successes)
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
    let (_status, _, failures, _, _, _) = get_circuit_state(pool, governed_surface).await?;

    let next_failures = failures + 1;
    let next_would_open = if next_failures >= 3 {
        crate::metrics::increment_counter("hot_read_circuit_would_open_total");
        1
    } else {
        0
    };

    sqlx::query(
        "INSERT INTO hot_read_circuit_states (governed_surface, circuit_status, consecutive_successes, consecutive_failures, last_violation_kind, would_open, updated_at_ms)
         VALUES (?, 'closed', 0, ?, ?, ?, ?)
         ON CONFLICT(governed_surface) DO UPDATE SET 
            consecutive_successes = 0,
            consecutive_failures = excluded.consecutive_failures,
            last_violation_kind = excluded.last_violation_kind,
            would_open = excluded.would_open,
            updated_at_ms = excluded.updated_at_ms"
    )
    .bind(governed_surface)
    .bind(next_failures)
    .bind(violation_kind)
    .bind(next_would_open)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
