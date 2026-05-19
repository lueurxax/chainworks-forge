use anyhow::Result;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::fmt;

pub const MAX_INVALIDATION_ROWS: i64 = 50_000;
pub const MAX_INVALIDATION_BYTES: i64 = 64 * 1024 * 1024; // 64 MiB
pub const COALESCE_THRESHOLD_PERCENT: i64 = 80;
const BACKLOG_THROTTLE_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionInvalidationThrottle {
    pub retry_after_ms: i64,
}

impl ProjectionInvalidationThrottle {
    pub const CODE: &'static str = "projection_invalidation_backlog_exceeded";
}

impl fmt::Display for ProjectionInvalidationThrottle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{};retry_after_ms={}", Self::CODE, self.retry_after_ms)
    }
}

impl std::error::Error for ProjectionInvalidationThrottle {}

pub async fn record_invalidation(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
    primary_key: &str,
    kind: &str,
    payload_json: Option<serde_json::Value>,
) -> Result<()> {
    let op_name = match kind {
        "upsert" => "projection.invalidation.upsert",
        "delete" => "projection.invalidation.delete",
        _ => "projection.invalidation.generic",
    };

    let mut tx = crate::writer::begin_repository_transaction(pool, op_name).await?;
    let result = record_invalidation_internal(
        &mut **tx,
        projection_name,
        source_name,
        primary_key,
        kind,
        payload_json,
    )
    .await;

    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(error)
            if error
                .downcast_ref::<ProjectionInvalidationThrottle>()
                .is_some() =>
        {
            tx.commit().await?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

pub async fn record_invalidation_internal(
    conn: &mut sqlx::SqliteConnection,
    projection_name: &str,
    source_name: &str,
    primary_key: &str,
    kind: &str,
    payload_json: Option<serde_json::Value>,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();

    let payload_str = payload_json
        .map(|p| serde_json::to_string(&p))
        .transpose()?;
    let size_bytes = payload_str.as_ref().map(|s| s.len() as i64).unwrap_or(0);

    let existing = sqlx::query(
        "SELECT COUNT(*) as count, SUM(size_bytes) as total_bytes
         FROM projection_invalidation_log
         WHERE projection_name = ? AND source_name = ? AND primary_key = ? AND is_consumed = 0",
    )
    .bind(projection_name)
    .bind(source_name)
    .bind(primary_key)
    .fetch_one(&mut *conn)
    .await?;
    let existing_count: i64 = existing.get("count");
    let existing_bytes: i64 = existing.get::<Option<i64>, _>("total_bytes").unwrap_or(0);

    // 1. Check current backlog size
    let stats = sqlx::query(
        "SELECT COUNT(*) as count, SUM(size_bytes) as total_bytes 
         FROM projection_invalidation_log 
         WHERE projection_name = ? AND source_name = ? AND is_consumed = 0",
    )
    .bind(projection_name)
    .bind(source_name)
    .fetch_one(&mut *conn)
    .await?;

    let count: i64 = stats.get("count");
    let total_bytes: i64 = stats.get::<Option<i64>, _>("total_bytes").unwrap_or(0);
    let final_count = count - existing_count + 1;
    let final_bytes = total_bytes - existing_bytes + size_bytes;

    crate::metrics::record_projection_backlog(
        projection_name,
        source_name,
        count as u64,
        total_bytes as u64,
    );

    // 2. Handle 100% capacity (throttle/freeze)
    if final_count > MAX_INVALIDATION_ROWS || final_bytes > MAX_INVALIDATION_BYTES {
        crate::metrics::increment_counter("projection_invalidation_backlog_exceeded_total");
        sqlx::query(
            "INSERT INTO projection_cursors (projection_name, source_name, watermark_ms, is_poisoned, last_error, updated_at_ms, throttled_until_ms, first_healthy_at_ms)
             VALUES (?, ?, ?, 1, 'projection_invalidation_backlog_exceeded', ?, ?, NULL)
             ON CONFLICT(projection_name, source_name) DO UPDATE SET
                is_poisoned = 1,
                last_error = 'projection_invalidation_backlog_exceeded',
                throttled_until_ms = excluded.throttled_until_ms,
                updated_at_ms = excluded.updated_at_ms,
                first_healthy_at_ms = NULL"
        )
        .bind(projection_name)
        .bind(source_name)
        .bind(now)
        .bind(now)
        .bind(now + BACKLOG_THROTTLE_MS)
        .execute(&mut *conn)
        .await?;

        return Err(ProjectionInvalidationThrottle {
            retry_after_ms: BACKLOG_THROTTLE_MS,
        }
        .into());
    }

    // Coalesce only after the final footprint is accepted. That preserves an
    // existing valid invalidation when an oversized replacement is rejected.
    sqlx::query(
        "DELETE FROM projection_invalidation_log
         WHERE projection_name = ? AND source_name = ? AND primary_key = ? AND is_consumed = 0",
    )
    .bind(projection_name)
    .bind(source_name)
    .bind(primary_key)
    .execute(&mut *conn)
    .await?;

    // 3. Handle 80% capacity (coalesce) - P087: same-key rows were coalesced
    // above before the final capacity check.
    if count >= (MAX_INVALIDATION_ROWS * COALESCE_THRESHOLD_PERCENT / 100) {
        crate::metrics::increment_counter("projection_invalidation_coalesce_near_capacity_total");
    }

    // 4. Record new invalidation
    sqlx::query(
        "INSERT INTO projection_invalidation_log 
         (projection_name, source_name, primary_key, invalidation_kind, payload_json, size_bytes, created_at_ms, is_consumed)
         VALUES (?, ?, ?, ?, ?, ?, ?, 0)"
    )
    .bind(projection_name)
    .bind(source_name)
    .bind(primary_key)
    .bind(kind)
    .bind(payload_str)
    .bind(size_bytes)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn clear_backlog(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
    caller_principal_id: Option<&str>,
    caller_principal_class: Option<&str>,
    request_id: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "projection.invalidation.clear").await?;

    let journal_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "projection_name": projection_name,
        "source_name": source_name,
    });
    crate::repos::command_journal::record_tx(
        &mut tx,
        &journal_id,
        "StorageProjectionsClearBacklog",
        &payload.to_string(),
        None,
        now,
        Some("mcp"),
        caller_principal_id,
        caller_principal_class,
        Some("storage.projections.clear_backlog"),
        request_id,
    )
    .await?;

    // P087: Clear only unconsumed backlog rows.
    sqlx::query(
        "DELETE FROM projection_invalidation_log WHERE projection_name = ? AND source_name = ? AND is_consumed = 0",
    )
    .bind(projection_name)
    .bind(source_name)
    .execute(&mut **tx)
    .await?;

    // Reset first_healthy_at_ms: after clearing backlog, the cursor is no longer
    // poisoned but lag is unknown. The 48-hour freshness window restarts from zero.
    sqlx::query(
        "UPDATE projection_cursors
         SET is_poisoned = 0, last_error = NULL, throttled_until_ms = NULL,
             updated_at_ms = ?, first_healthy_at_ms = NULL
         WHERE projection_name = ? AND source_name = ?",
    )
    .bind(now_ms)
    .bind(projection_name)
    .bind(source_name)
    .execute(&mut **tx)
    .await?;

    crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn clear_poison(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
    caller_principal_id: Option<&str>,
    caller_principal_class: Option<&str>,
    request_id: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "projection.invalidation.clear_poison")
            .await?;

    let journal_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "projection_name": projection_name,
        "source_name": source_name,
    });
    crate::repos::command_journal::record_tx(
        &mut tx,
        &journal_id,
        "StorageProjectionsClearPoison",
        &payload.to_string(),
        None,
        now,
        Some("mcp"),
        caller_principal_id,
        caller_principal_class,
        Some("storage.projections.clear_poison"),
        request_id,
    )
    .await?;

    // Reset first_healthy_at_ms: after clearing poison, the cursor recovers but
    // the 48-hour freshness window restarts from zero to prove sustained health.
    sqlx::query(
        "UPDATE projection_cursors
         SET is_poisoned = 0, last_error = NULL, throttled_until_ms = NULL,
             updated_at_ms = ?, first_healthy_at_ms = NULL
         WHERE projection_name = ? AND source_name = ?",
    )
    .bind(now_ms)
    .bind(projection_name)
    .bind(source_name)
    .execute(&mut **tx)
    .await?;

    crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
    tx.commit().await?;
    Ok(())
}

/// P087: Mark invalidation log rows as consumed after successful projection update.
pub async fn mark_consumed_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    projection_name: &str,
    source_name: &str,
    before_ms: i64,
) -> Result<u64> {
    let now_ms = Utc::now().timestamp_millis();
    let result = sqlx::query(
        "UPDATE projection_invalidation_log 
         SET is_consumed = 1, consumed_at_ms = ? 
         WHERE projection_name = ? AND source_name = ? AND created_at_ms <= ? AND is_consumed = 0",
    )
    .bind(now_ms)
    .bind(projection_name)
    .bind(source_name)
    .bind(before_ms)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

/// P087: Mark invalidation log rows for a specific entity as consumed.
pub async fn mark_consumed_entity_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    projection_name: &str,
    source_name: &str,
    primary_key: &str,
    before_ms: i64,
) -> Result<u64> {
    let now_ms = Utc::now().timestamp_millis();
    let result = sqlx::query(
        "UPDATE projection_invalidation_log 
         SET is_consumed = 1, consumed_at_ms = ? 
         WHERE projection_name = ? AND source_name = ? AND primary_key = ? AND created_at_ms <= ? AND is_consumed = 0",
    )
    .bind(now_ms)
    .bind(projection_name)
    .bind(source_name)
    .bind(primary_key)
    .bind(before_ms)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

/// P087: Returns (projection_name, source_name) pairs ordered by oldest unconsumed
/// invalidation watermark — oldest first = highest drain priority.
/// Consumers call this when the backlog is at or near capacity to pick which
/// projection/source pair to drain first, satisfying the proposal's requirement that
/// consumers prioritize the oldest source watermark.
pub async fn get_drain_priority_queue(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT projection_name, source_name, MIN(created_at_ms) AS oldest_ms
         FROM projection_invalidation_log
         WHERE is_consumed = 0
         GROUP BY projection_name, source_name
         ORDER BY oldest_ms ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.get::<String, _>("projection_name"),
                r.get::<String, _>("source_name"),
            )
        })
        .collect())
}

/// P087: Freeze a cursor after consumer retry exhaustion.
/// Frozen cursors preserve unconsumed rows and watermark tombstones until an operator
/// calls clear_backlog or clear_poison followed by a successful replay.
pub async fn freeze_cursor_after_retry_exhaustion(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
    retry_count: u32,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "projection.invalidation.freeze_cursor")
            .await?;

    sqlx::query(
        "INSERT INTO projection_cursors
             (projection_name, source_name, watermark_ms, is_poisoned, last_error, updated_at_ms, first_healthy_at_ms)
         VALUES (?, ?, ?, 1, 'projection_invalidation_consumer_retry_exhausted', ?, NULL)
         ON CONFLICT(projection_name, source_name) DO UPDATE SET
            is_poisoned = 1,
            last_error = 'projection_invalidation_consumer_retry_exhausted',
            updated_at_ms = excluded.updated_at_ms,
            first_healthy_at_ms = NULL",
    )
    .bind(projection_name)
    .bind(source_name)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    crate::metrics::increment_counter("projection_cursor_freeze_retry_exhaustion_total");
    tracing::warn!(
        event = "projection_cursor_freeze_retry_exhaustion",
        projection_name = %projection_name,
        source_name = %source_name,
        retry_count = retry_count,
        "Freezing projection cursor after consumer retry exhaustion; operator clear required"
    );

    tx.commit().await?;
    Ok(())
}

/// P087: Periodic reaper for consumed invalidation log rows.
/// Retains consumed rows for 24 hours to support historical diagnostics.
pub async fn reap_consumed_log(pool: &SqlitePool) -> Result<u64> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "projection.invalidation.reap").await?;
    let result = reap_consumed_log_tx(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn reap_consumed_log_tx(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<u64> {
    let now_ms = Utc::now().timestamp_millis();
    let cutoff = now_ms - 24 * 60 * 60 * 1000;

    // Retain consumed rows for 24 hours AND until one successful checkpoint:
    // a non-poisoned cursor with watermark_ms >= consumed_at_ms proves the
    // projection has rebuilt past this invalidation.
    let result = sqlx::query(
        "DELETE FROM projection_invalidation_log
         WHERE is_consumed = 1 AND consumed_at_ms < ?
           AND EXISTS (
             SELECT 1 FROM projection_cursors pc
             WHERE pc.projection_name = projection_invalidation_log.projection_name
               AND pc.source_name = projection_invalidation_log.source_name
               AND pc.is_poisoned = 0
               AND pc.watermark_ms >= projection_invalidation_log.consumed_at_ms
           )",
    )
    .bind(cutoff)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn proposal_087_projection_invalidation_records_and_clears_rows() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        record_invalidation(
            &pool,
            "runs_home",
            "runs",
            "run-p087",
            "upsert",
            Some(serde_json::json!({"status": "running"})),
        )
        .await
        .expect("projection invalidation writes must be registered and executable");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log \
             WHERE projection_name = 'runs_home' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        clear_backlog(&pool, "runs_home", "runs", None, None, None)
            .await
            .expect("projection invalidation clear must be registered and executable");
        let count_after_clear: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log \
             WHERE projection_name = 'runs_home' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count_after_clear, 0);
    }

    #[tokio::test]
    async fn proposal_087_projection_invalidation_oversize_payload_fails_closed_with_throttle() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let payload = serde_json::json!({
            "blob": "x".repeat((MAX_INVALIDATION_BYTES as usize) + 1)
        });

        let err = record_invalidation(
            &pool,
            "runs_home",
            "runs",
            "run-oversize",
            "upsert",
            Some(payload),
        )
        .await
        .expect_err("oversize invalidation payload must fail closed");
        let typed = err
            .downcast_ref::<ProjectionInvalidationThrottle>()
            .expect("backlog throttle must be a typed retry-after error");
        assert_eq!(typed.retry_after_ms, BACKLOG_THROTTLE_MS);
        let message = err.to_string();
        assert!(message.contains("projection_invalidation_backlog_exceeded"));
        assert!(message.contains("retry_after_ms=60000"));

        let row = sqlx::query(
            "SELECT is_poisoned, last_error, throttled_until_ms FROM projection_cursors \
             WHERE projection_name = 'runs_home' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<i32, _>("is_poisoned"), 1);
        assert_eq!(
            row.get::<Option<String>, _>("last_error").as_deref(),
            Some("projection_invalidation_backlog_exceeded")
        );
        assert!(
            row.get::<Option<i64>, _>("throttled_until_ms").is_some(),
            "poisoned cursor must expose retry-after throttle readback"
        );

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projection_invalidation_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "oversize payload must not be inserted");
    }

    #[tokio::test]
    async fn proposal_087_projection_invalidation_lifecycle() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();

        // 1. Record invalidation
        record_invalidation(&pool, "p1", "s1", "k1", "upsert", None)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log WHERE is_consumed = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        // 2. Mark consumed
        let mut tx = pool.begin().await.unwrap();
        mark_consumed_tx(&mut tx, "p1", "s1", now + 1000)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log WHERE is_consumed = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        let count_consumed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log WHERE is_consumed = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count_consumed, 1);
    }

    #[tokio::test]
    async fn proposal_087_projection_invalidation_coalesces_same_key_before_capacity_check() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        for idx in 0..MAX_INVALIDATION_ROWS {
            sqlx::query(
                "INSERT INTO projection_invalidation_log \
                 (projection_name, source_name, primary_key, invalidation_kind, size_bytes, created_at_ms) \
                 VALUES ('runs_home', 'runs', ?, 'upsert', 1, ?)",
            )
            .bind(format!("run-{idx}"))
            .bind(idx)
            .execute(&pool)
            .await
            .unwrap();
        }

        record_invalidation(
            &pool,
            "runs_home",
            "runs",
            "run-7",
            "upsert",
            Some(serde_json::json!({"status": "completed"})),
        )
        .await
        .expect("same-key update at row cap must coalesce before capacity check");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log \
             WHERE projection_name = 'runs_home' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, MAX_INVALIDATION_ROWS);
    }

    #[tokio::test]
    async fn proposal_087_projection_invalidation_oversized_replacement_preserves_prior_row() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        record_invalidation(
            &pool,
            "runs_home",
            "runs",
            "run-7",
            "upsert",
            Some(serde_json::json!({"status": "pending"})),
        )
        .await
        .unwrap();

        let payload = serde_json::json!({
            "blob": "x".repeat((MAX_INVALIDATION_BYTES as usize) + 1)
        });
        let err = record_invalidation(&pool, "runs_home", "runs", "run-7", "upsert", Some(payload))
            .await
            .expect_err("oversized replacement must fail closed");
        assert!(err
            .downcast_ref::<ProjectionInvalidationThrottle>()
            .is_some());

        let row = sqlx::query(
            "SELECT invalidation_kind, payload_json, is_consumed
             FROM projection_invalidation_log
             WHERE projection_name = 'runs_home' AND source_name = 'runs' AND primary_key = 'run-7'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("invalidation_kind"), "upsert");
        assert_eq!(row.get::<i64, _>("is_consumed"), 0);
        assert!(row
            .get::<Option<String>, _>("payload_json")
            .unwrap()
            .contains("pending"));
    }

    #[tokio::test]
    async fn proposal_087_drain_priority_queue_orders_by_oldest_watermark() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        // Insert rows for two (projection, source) pairs with distinct ages.
        // "slow_proj" has an older entry (created_at_ms = 100) than "fast_proj" (1000).
        sqlx::query(
            "INSERT INTO projection_invalidation_log
             (projection_name, source_name, primary_key, invalidation_kind, size_bytes, created_at_ms)
             VALUES ('slow_proj', 's1', 'k1', 'upsert', 1, 100)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO projection_invalidation_log
             (projection_name, source_name, primary_key, invalidation_kind, size_bytes, created_at_ms)
             VALUES ('fast_proj', 's1', 'k1', 'upsert', 1, 1000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let queue = get_drain_priority_queue(&pool).await.unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue[0].0, "slow_proj",
            "oldest entry must be first in drain queue"
        );
        assert_eq!(queue[1].0, "fast_proj");
    }

    #[tokio::test]
    async fn proposal_087_drain_priority_queue_excludes_consumed_rows() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        // Consumed row must not appear in the drain queue.
        sqlx::query(
            "INSERT INTO projection_invalidation_log
             (projection_name, source_name, primary_key, invalidation_kind, size_bytes, created_at_ms, is_consumed)
             VALUES ('p1', 's1', 'k1', 'upsert', 1, 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let queue = get_drain_priority_queue(&pool).await.unwrap();
        assert!(
            queue.is_empty(),
            "consumed rows must not appear in drain priority queue"
        );
    }

    #[tokio::test]
    async fn proposal_087_freeze_cursor_after_retry_exhaustion_marks_poisoned() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        freeze_cursor_after_retry_exhaustion(&pool, "run_summaries", "runs", 3)
            .await
            .expect("freeze_cursor must be registered and executable");

        let row = sqlx::query(
            "SELECT is_poisoned, last_error FROM projection_cursors \
             WHERE projection_name = 'run_summaries' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.get::<i32, _>("is_poisoned"), 1);
        assert_eq!(
            row.get::<Option<String>, _>("last_error").as_deref(),
            Some("projection_invalidation_consumer_retry_exhausted")
        );
    }

    #[tokio::test]
    async fn proposal_087_terminal_write_commits_even_when_backlog_is_at_capacity() {
        // Fill the invalidation log to capacity for "run_summaries"/"runs",
        // then call invalidate_projections_terminal and assert the cursor watermark
        // was written (i.e. the outer transaction would have committed successfully).
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        // Fill to row cap
        for idx in 0..MAX_INVALIDATION_ROWS {
            sqlx::query(
                "INSERT INTO projection_invalidation_log \
                 (projection_name, source_name, primary_key, invalidation_kind, size_bytes, created_at_ms) \
                 VALUES ('run_summaries', 'runs', ?, 'upsert', 1, ?)",
            )
            .bind(format!("run-{idx}"))
            .bind(idx)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Now call invalidate_projections_terminal via a transaction.
        // It must absorb the backlog throttle and return Ok.
        let mut tx = pool.begin().await.unwrap();
        crate::repos::projections::invalidate_projections_terminal(
            &mut tx,
            domain::ids::RunId::new(),
        )
        .await
        .expect("terminal invalidation must not abort when backlog is at capacity");
        tx.commit().await.unwrap();

        // The cursor watermark must have been written despite the backlog being full.
        let cursor_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_cursors \
             WHERE projection_name = 'run_summaries' AND source_name = 'runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            cursor_count, 1,
            "cursor watermark must be written even when invalidation backlog is full"
        );
    }
}
