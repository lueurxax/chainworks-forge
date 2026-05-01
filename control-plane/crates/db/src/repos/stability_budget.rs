/// P073: Stability budget materializer — single authoritative writer.
///
/// Full-recomputation rule: every `materialize_full_snapshot` call recomputes
/// all 12 metrics from live DB state and inserts a complete new snapshot row
/// set. There are no per-metric merge updates.
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Write a fresh full snapshot of all 12 P073 stability metrics.
/// Returns the new `snapshot_id`.
pub async fn materialize_full_snapshot(pool: &SqlitePool) -> sqlx::Result<String> {
    let snapshot_id = Uuid::new_v4().to_string();
    let captured_at = Utc::now().to_rfc3339();
    let phase = "phase_0";

    let metrics = compute_metrics(pool).await?;

    for m in &metrics {
        let id = Uuid::new_v4().to_string();
        sqlx::query(r#"
            INSERT OR REPLACE INTO stability_budget_snapshots
                (id, snapshot_id, captured_at, phase, metric_id, metric_classification,
                 blocking_mode, measurement_status, current_value, baseline_value,
                 target_threshold, latest_by_instrumentation_date, missing_data_policy, notes)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
        "#)
        .bind(&id)
        .bind(&snapshot_id)
        .bind(&captured_at)
        .bind(phase)
        .bind(m.metric_id)
        .bind(m.classification)
        .bind(m.blocking_mode)
        .bind(&m.measurement_status)
        .bind(m.current_value)
        .bind::<Option<f64>>(None)
        .bind(m.target_threshold)
        .bind(m.latest_by_instrumentation_date)
        .bind(m.missing_data_policy)
        .bind(m.notes)
        .execute(pool)
        .await?;
    }

    Ok(snapshot_id)
}

/// Promote a snapshot to the durable baseline by writing `baseline_value`
/// from `current_value` for all rows where `baseline_value IS NULL`.
pub async fn promote_to_baseline(pool: &SqlitePool, snapshot_id: &str) -> sqlx::Result<u64> {
    let result = sqlx::query(r#"
        UPDATE stability_budget_snapshots
        SET baseline_value = current_value
        WHERE snapshot_id = ?1 AND baseline_value IS NULL AND current_value IS NOT NULL
    "#)
    .bind(snapshot_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

struct MetricRow {
    metric_id: &'static str,
    classification: &'static str,
    blocking_mode: &'static str,
    measurement_status: String,
    current_value: Option<f64>,
    target_threshold: &'static str,
    latest_by_instrumentation_date: Option<&'static str>,
    missing_data_policy: &'static str,
    notes: &'static str,
}

async fn compute_metrics(pool: &SqlitePool) -> sqlx::Result<Vec<MetricRow>> {
    let sb01 = compute_sb01_failed_run_rate(pool).await?;
    let sb02 = compute_sb02_stale_active_executions(pool).await?;
    let sb05 = compute_sb05_artifact_count_per_run(pool).await?;
    let sb12 = compute_sb12_approval_settlement_latency(pool).await?;

    Ok(vec![
        sb01,
        sb02,
        MetricRow {
            metric_id: "SB-03",
            classification: "server_native",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "0 stale projection rows",
            latest_by_instrumentation_date: Some("2026-05-09"),
            missing_data_policy: "advisory_only_until_instrumentation_landing",
            notes: "Projection lag counter pending phase_1 instrumentation.",
        },
        MetricRow {
            metric_id: "SB-04",
            classification: "client_observed",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "< 5 reconnects per 24 h",
            latest_by_instrumentation_date: None,
            missing_data_policy: "advisory_only_entire_p073_window",
            notes: "Client-observed; requires Swift-side instrumentation.",
        },
        sb05,
        MetricRow {
            metric_id: "SB-06",
            classification: "server_native",
            blocking_mode: "advisory_until_p038",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "compacted_count / total_count >= 0.5 per run",
            latest_by_instrumentation_date: Some("2026-05-15"),
            missing_data_policy: "advisory_only_until_p038_seam",
            notes: "Awaiting P038 compaction seam.",
        },
        MetricRow {
            metric_id: "SB-07",
            classification: "server_native",
            blocking_mode: "blocking_after_condition",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "0 leaked processes or stale sessions",
            latest_by_instrumentation_date: Some("2026-05-09"),
            missing_data_policy: "advisory_only_until_instrumentation_landing",
            notes: "Xcode bridge pool leak count; pending P051 stabilization.",
        },
        MetricRow {
            metric_id: "SB-08",
            classification: "client_observed",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "p95 < 3000 ms",
            latest_by_instrumentation_date: None,
            missing_data_policy: "advisory_only_entire_p073_window",
            notes: "Client-observed; requires ACP adapter timing instrumentation.",
        },
        MetricRow {
            metric_id: "SB-09",
            classification: "client_observed",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "< 0.05 failure rate",
            latest_by_instrumentation_date: None,
            missing_data_policy: "advisory_only_entire_p073_window",
            notes: "Client-observed; requires MCP call-site error counting.",
        },
        MetricRow {
            metric_id: "SB-10",
            classification: "client_observed",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "< 3 degraded-state occurrences per day",
            latest_by_instrumentation_date: None,
            missing_data_policy: "advisory_only_entire_p073_window",
            notes: "Client-observed; requires UI degraded-state counter.",
        },
        MetricRow {
            metric_id: "SB-11",
            classification: "client_observed",
            blocking_mode: "advisory",
            measurement_status: "missing".into(),
            current_value: None,
            target_threshold: "p95 < 500 ms",
            latest_by_instrumentation_date: None,
            missing_data_policy: "advisory_only_entire_p073_window",
            notes: "Client-observed; requires GraphQL query timing on Swift side.",
        },
        sb12,
    ])
}

/// SB-01: Failed/blocked run rate = failed_count / (failed + completed) in last 30 days.
async fn compute_sb01_failed_run_rate(pool: &SqlitePool) -> sqlx::Result<MetricRow> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_count
        FROM runs
        WHERE status IN ('completed', 'failed')
          AND started_at >= datetime('now', '-30 days')
        "#,
    )
    .fetch_one(pool)
    .await?;

    let total: i64 = row.try_get("total")?;
    let failed: i64 = row.try_get("failed_count").unwrap_or(0);

    let (status, value) = if total == 0 {
        ("missing".to_string(), None)
    } else {
        let rate = failed as f64 / total as f64;
        ("present".to_string(), Some(rate))
    };

    Ok(MetricRow {
        metric_id: "SB-01",
        classification: "derived",
        blocking_mode: "blocking",
        measurement_status: status,
        current_value: value,
        target_threshold: "< 0.20 (20% failure rate)",
        latest_by_instrumentation_date: None,
        missing_data_policy: "treat_as_green_when_absent_no_recent_runs",
        notes: "Count of failed terminal runs / all terminal runs in last 30 days.",
    })
}

/// SB-02: Stale active execution count — agent_executions running > 2 h.
async fn compute_sb02_stale_active_executions(pool: &SqlitePool) -> sqlx::Result<MetricRow> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM agent_executions
        WHERE status = 'running'
          AND started_at < datetime('now', '-2 hours')
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(MetricRow {
        metric_id: "SB-02",
        classification: "derived",
        blocking_mode: "blocking",
        measurement_status: "present".into(),
        current_value: Some(count as f64),
        target_threshold: "= 0",
        latest_by_instrumentation_date: None,
        missing_data_policy: "treat_as_green_when_absent",
        notes: "Count of agent_executions with status=running and started_at > 2 h ago.",
    })
}

/// SB-05: Average artifact count per recently-completed run.
async fn compute_sb05_artifact_count_per_run(pool: &SqlitePool) -> sqlx::Result<MetricRow> {
    let avg: Option<f64> = sqlx::query_scalar(
        r#"
        SELECT AVG(cnt) FROM (
            SELECT COUNT(*) AS cnt
            FROM artifacts
            WHERE run_id IN (
                SELECT id FROM runs
                WHERE status = 'completed'
                ORDER BY started_at DESC
                LIMIT 20
            )
            GROUP BY run_id
        )
        "#,
    )
    .fetch_one(pool)
    .await?;

    let (status, value) = match avg {
        Some(v) => ("present".to_string(), Some(v)),
        None => ("missing".to_string(), None),
    };

    Ok(MetricRow {
        metric_id: "SB-05",
        classification: "derived",
        blocking_mode: "advisory",
        measurement_status: status,
        current_value: value,
        target_threshold: "< 50 artifacts per run",
        latest_by_instrumentation_date: None,
        missing_data_policy: "advisory_only_entire_p073_window",
        notes: "Average artifact count across last 20 completed runs.",
    })
}

/// SB-12: Approval settlement latency — average ms from requested to resolved.
async fn compute_sb12_approval_settlement_latency(pool: &SqlitePool) -> sqlx::Result<MetricRow> {
    let avg_ms: Option<f64> = sqlx::query_scalar(
        r#"
        SELECT AVG(
            (julianday(decided_at) - julianday(requested_at)) * 86400000.0
        )
        FROM approvals
        WHERE decision != 'pending'
          AND decided_at IS NOT NULL
          AND requested_at IS NOT NULL
          AND requested_at >= datetime('now', '-30 days')
        "#,
    )
    .fetch_one(pool)
    .await?;

    let (status, value) = match avg_ms {
        Some(v) => ("present".to_string(), Some(v)),
        None => ("missing".to_string(), None),
    };

    Ok(MetricRow {
        metric_id: "SB-12",
        classification: "derived",
        blocking_mode: "advisory",
        measurement_status: status,
        current_value: value,
        target_threshold: "< 300000 ms (5 min average)",
        latest_by_instrumentation_date: None,
        missing_data_policy: "advisory_only_entire_p073_window",
        notes: "Average ms from approval requested_at to resolved_at in last 30 days.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn test_pool() -> SqlitePool {
        crate::pool::create_pool("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn stability_budget_materialize_writes_12_rows() {
        let pool = test_pool().await;
        let snapshot_id = materialize_full_snapshot(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stability_budget_snapshots WHERE snapshot_id = ?1",
        )
        .bind(&snapshot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 12, "materializer must write exactly 12 metric rows");
    }

    #[tokio::test]
    async fn stability_budget_metric_ids_are_sb01_through_sb12() {
        let pool = test_pool().await;
        let snapshot_id = materialize_full_snapshot(&pool).await.unwrap();
        let mut ids: Vec<String> = sqlx::query_scalar(
            "SELECT metric_id FROM stability_budget_snapshots WHERE snapshot_id = ?1 ORDER BY metric_id",
        )
        .bind(&snapshot_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        ids.sort();
        let expected: Vec<String> = (1..=12).map(|n| format!("SB-{n:02}")).collect();
        assert_eq!(ids, expected);
    }

    #[tokio::test]
    async fn stability_budget_promote_to_baseline_sets_baseline_values() {
        let pool = test_pool().await;
        let snapshot_id = materialize_full_snapshot(&pool).await.unwrap();
        let affected = promote_to_baseline(&pool, &snapshot_id).await.unwrap();
        // At minimum the present metrics should have baseline promoted.
        assert!(affected > 0, "promote_to_baseline must set at least one baseline_value");
        // Second call is idempotent — no more rows to update.
        let second = promote_to_baseline(&pool, &snapshot_id).await.unwrap();
        assert_eq!(second, 0, "second promote must update 0 rows (idempotent)");
    }
}
