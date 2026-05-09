use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool};

const STARTUP_RECOVERY_STALE_AFTER_MS: i64 = 60_000;

/// P066 T17: Toolchain cache fields nested inside StartupRecoverySummary.
/// Populated after the first startup recovery sweep that enumerates session-scoped roots.
/// Pre-migration rows have all fields None — surface as "no sweep yet".
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ToolchainCacheRecoveryReadback {
    pub session_scoped_roots_seen: Option<i64>,
    pub session_scoped_roots_reclaimed: Option<i64>,
    pub session_scoped_cleanup_failures: Option<i64>,
    pub orphan_threshold_minutes: Option<i64>,
    pub last_sweep_started_at: Option<DateTime<Utc>>,
}

impl ToolchainCacheRecoveryReadback {
    pub fn is_empty(&self) -> bool {
        self.session_scoped_roots_seen.is_none() && self.last_sweep_started_at.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupRecoveryReadback {
    pub id: String,
    pub recovered_item_count: i64,
    pub queued_under_startup_recovery_backpressure_count: i64,
    pub oldest_recovered_queued_age_ms: Option<i64>,
    pub affected_run_count: i64,
    pub next_retry_or_backoff_time: Option<DateTime<Utc>>,
    pub stale_after_ms: i64,
    pub updated_at: DateTime<Utc>,
    /// P066 T17: toolchainCache extension. None fields = no sweep yet.
    pub toolchain_cache: ToolchainCacheRecoveryReadback,
}

impl StartupRecoveryReadback {
    pub fn is_stale_at(&self, now: DateTime<Utc>) -> bool {
        self.updated_at + chrono::Duration::milliseconds(self.stale_after_ms) <= now
    }
}

/// Record a startup repair action for audit.
pub async fn record(
    pool: &SqlitePool,
    id: &str,
    run_id: &str,
    repair_kind: &str,
    repaired_at: DateTime<Utc>,
    notes: Option<&str>,
) -> Result<()> {
    record_tx(pool, id, run_id, repair_kind, repaired_at, notes).await
}

pub async fn record_tx<'a, E>(
    executor: E,
    id: &str,
    run_id: &str,
    repair_kind: &str,
    repaired_at: DateTime<Utc>,
    notes: Option<&str>,
) -> Result<()>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    sqlx::query(
        r#"INSERT INTO startup_repairs (id, run_id, repair_kind, repaired_at, notes)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
    )
    .bind(id)
    .bind(run_id)
    .bind(repair_kind)
    .bind(repaired_at.to_rfc3339())
    .bind(notes)
    .execute(executor)
    .await
    .context("record startup repair")?;
    Ok(())
}

pub async fn build_startup_recovery_readback(
    pool: &SqlitePool,
    recovered_item_count: i64,
    affected_run_count: i64,
    updated_at: DateTime<Utc>,
) -> Result<StartupRecoveryReadback> {
    let queued_under_startup_recovery_backpressure_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(queued_count), 0)
           FROM scheduler_queue_summaries
           WHERE scope = 'global'
             AND top_reason = 'startup_recovery_backpressure'"#,
    )
    .fetch_one(pool)
    .await
    .context("count startup recovery backpressured queue summaries")?;

    let oldest_recovered_queued_age_ms: Option<i64> = sqlx::query_scalar(
        r#"SELECT MAX(oldest_queued_age_ms)
           FROM scheduler_queue_summaries
           WHERE top_reason = 'startup_recovery_backpressure'"#,
    )
    .fetch_one(pool)
    .await
    .context("load oldest startup recovery queued age")?;

    let next_retry_or_backoff_time = sqlx::query_scalar::<_, Option<String>>(
        r#"SELECT MIN(scheduled_at)
           FROM work_items
           WHERE status = 'pending'
             AND (
               payload_json LIKE '%p061_startup_recovery%'
               OR payload_json LIKE '%startup_repair%'
               OR payload_json LIKE '%startup_catchup%'
             )"#,
    )
    .fetch_one(pool)
    .await
    .context("load next startup recovery retry/backoff time")?
    .map(|raw| parse_datetime(&raw))
    .transpose()?;

    let recovered_work_affected_run_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(DISTINCT run_id)
           FROM work_items
           WHERE status = 'pending'
             AND run_id IS NOT NULL
             AND (
               payload_json LIKE '%p061_startup_recovery%'
               OR payload_json LIKE '%startup_repair%'
               OR payload_json LIKE '%startup_catchup%'
             )"#,
    )
    .fetch_one(pool)
    .await
    .context("count startup recovery affected runs from requeued work")?;
    let affected_run_count = affected_run_count.max(recovered_work_affected_run_count);

    Ok(StartupRecoveryReadback {
        id: uuid::Uuid::new_v4().to_string(),
        recovered_item_count,
        queued_under_startup_recovery_backpressure_count,
        oldest_recovered_queued_age_ms,
        affected_run_count,
        next_retry_or_backoff_time,
        stale_after_ms: STARTUP_RECOVERY_STALE_AFTER_MS,
        updated_at,
        toolchain_cache: ToolchainCacheRecoveryReadback::default(),
    })
}

pub async fn record_startup_recovery_readback(
    pool: &SqlitePool,
    readback: &StartupRecoveryReadback,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "startup_repairs.record_startup_recovery_readback",
        sqlx::query(
            r#"INSERT INTO startup_recovery_readbacks
           (id, recovered_item_count, queued_under_startup_recovery_backpressure_count,
            oldest_recovered_queued_age_ms, affected_run_count, next_retry_or_backoff_time,
            stale_after_ms, updated_at,
            toolchain_session_scoped_roots_seen,
            toolchain_session_scoped_roots_reclaimed,
            toolchain_session_scoped_cleanup_failures,
            toolchain_orphan_threshold_minutes,
            toolchain_last_sweep_started_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        )
        .bind(&readback.id)
        .bind(readback.recovered_item_count)
        .bind(readback.queued_under_startup_recovery_backpressure_count)
        .bind(readback.oldest_recovered_queued_age_ms)
        .bind(readback.affected_run_count)
        .bind(
            readback
                .next_retry_or_backoff_time
                .map(|value| value.to_rfc3339()),
        )
        .bind(readback.stale_after_ms)
        .bind(readback.updated_at.to_rfc3339())
        .bind(readback.toolchain_cache.session_scoped_roots_seen)
        .bind(readback.toolchain_cache.session_scoped_roots_reclaimed)
        .bind(readback.toolchain_cache.session_scoped_cleanup_failures)
        .bind(readback.toolchain_cache.orphan_threshold_minutes)
        .bind(
            readback
                .toolchain_cache
                .last_sweep_started_at
                .map(|t| t.to_rfc3339()),
        )
    )
    .context("record startup recovery readback")?;
    Ok(())
}

pub async fn latest_startup_recovery_readback(
    pool: &SqlitePool,
) -> Result<Option<StartupRecoveryReadback>> {
    let row = sqlx::query(
        r#"SELECT id, recovered_item_count, queued_under_startup_recovery_backpressure_count,
                  oldest_recovered_queued_age_ms, affected_run_count, next_retry_or_backoff_time,
                  stale_after_ms, updated_at,
                  toolchain_session_scoped_roots_seen,
                  toolchain_session_scoped_roots_reclaimed,
                  toolchain_session_scoped_cleanup_failures,
                  toolchain_orphan_threshold_minutes,
                  toolchain_last_sweep_started_at
           FROM startup_recovery_readbacks
           ORDER BY updated_at DESC, id ASC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .context("load latest startup recovery readback")?;

    row.map(parse_startup_recovery_readback_row).transpose()
}

/// Record a recovery recommendation for a run.
pub async fn recommend(
    pool: &SqlitePool,
    id: &str,
    run_id: &str,
    stage_id: Option<&str>,
    recommendation_kind: &str,
    reason: &str,
    created_at: DateTime<Utc>,
) -> Result<()> {
    recommend_tx(
        pool,
        id,
        run_id,
        stage_id,
        recommendation_kind,
        reason,
        created_at,
    )
    .await
}

pub async fn recommend_tx<'a, E>(
    executor: E,
    id: &str,
    run_id: &str,
    stage_id: Option<&str>,
    recommendation_kind: &str,
    reason: &str,
    created_at: DateTime<Utc>,
) -> Result<()>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    sqlx::query(
        r#"INSERT INTO recovery_recommendations (id, run_id, stage_id, recommendation_kind, reason, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
    )
    .bind(id)
    .bind(run_id)
    .bind(stage_id)
    .bind(recommendation_kind)
    .bind(reason)
    .bind(created_at.to_rfc3339())
    .execute(executor)
    .await
    .context("record recovery recommendation")?;
    Ok(())
}

fn parse_startup_recovery_readback_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<StartupRecoveryReadback> {
    let next_retry_or_backoff_time: Option<String> = row.get("next_retry_or_backoff_time");
    let toolchain_last_sweep: Option<String> = row
        .try_get("toolchain_last_sweep_started_at")
        .unwrap_or(None);
    Ok(StartupRecoveryReadback {
        id: row.get("id"),
        recovered_item_count: row.get("recovered_item_count"),
        queued_under_startup_recovery_backpressure_count: row
            .get("queued_under_startup_recovery_backpressure_count"),
        oldest_recovered_queued_age_ms: row.get("oldest_recovered_queued_age_ms"),
        affected_run_count: row.get("affected_run_count"),
        next_retry_or_backoff_time: next_retry_or_backoff_time
            .map(|raw| parse_datetime(&raw))
            .transpose()?,
        stale_after_ms: row.get("stale_after_ms"),
        updated_at: parse_datetime(row.get("updated_at"))?,
        toolchain_cache: ToolchainCacheRecoveryReadback {
            session_scoped_roots_seen: row
                .try_get("toolchain_session_scoped_roots_seen")
                .unwrap_or(None),
            session_scoped_roots_reclaimed: row
                .try_get("toolchain_session_scoped_roots_reclaimed")
                .unwrap_or(None),
            session_scoped_cleanup_failures: row
                .try_get("toolchain_session_scoped_cleanup_failures")
                .unwrap_or(None),
            orphan_threshold_minutes: row
                .try_get("toolchain_orphan_threshold_minutes")
                .unwrap_or(None),
            last_sweep_started_at: toolchain_last_sweep
                .map(|raw| parse_datetime(&raw))
                .transpose()?,
        },
    })
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("parse datetime: {raw}"))?
        .with_timezone(&Utc))
}
