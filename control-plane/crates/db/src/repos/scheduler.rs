use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::pool::begin_immediate_with_retry;
use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};

const STALE_AFTER_MS: i64 = 60_000;
const SUSTAINED_BACKPRESSURE_THRESHOLD_MS: i64 = 5 * 60_000;
const SUSTAINED_BACKPRESSURE_CLEAR_MS: i64 = 2 * 60_000;
const DB_WRITER_OBSERVATION_LIMIT: i64 = 200;
const COMMAND_LATENCY_OBSERVATION_LIMIT: i64 = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerServiceState {
    pub scope: String,
    pub scope_id: String,
    pub last_served_at: Option<DateTime<Utc>>,
    pub last_claimed_work_item_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerQueueSummary {
    pub scope: String,
    pub scope_id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub provider_family: Option<String>,
    pub top_reason: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub stale_after_ms: i64,
    pub updated_at: DateTime<Utc>,
}

impl SchedulerQueueSummary {
    pub fn is_stale_at(&self, now: DateTime<Utc>) -> bool {
        self.updated_at + Duration::milliseconds(self.stale_after_ms) <= now
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerHealthSnapshot {
    pub id: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub active_agent_executions: i64,
    pub db_writer_wait_p95_ms: Option<i64>,
    pub command_latency_p95_ms_json: Option<String>,
    pub last_host_interruption_epoch_id: Option<String>,
    pub sustained_backpressure_state: String,
    pub fire_consecutive_snapshots: i64,
    pub clear_consecutive_snapshots: i64,
    pub stale_after_ms: i64,
    pub updated_at: DateTime<Utc>,
}

impl SchedulerHealthSnapshot {
    pub fn is_stale_at(&self, now: DateTime<Utc>) -> bool {
        self.updated_at + Duration::milliseconds(self.stale_after_ms) <= now
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerProviderActiveCount {
    pub provider_family: String,
    pub active_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerQueuePositionHint {
    pub scope: String,
    pub scope_id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub queue_position: i64,
    pub global_queue_depth: i64,
    pub queued_ahead_count: i64,
    pub scoped_queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInterruptionEpoch {
    pub id: String,
    pub kind: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub monotonic_gap_ms: Option<i64>,
    pub wall_clock_gap_ms: Option<i64>,
    pub details_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInterruptionAffectedExecution {
    pub epoch_id: String,
    pub agent_execution_id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: String,
    pub provider_family: Option<String>,
    pub action: String,
    pub previous_status: String,
    pub settlement_status: String,
    pub cleanup_status: String,
    pub quota_budget_effect: String,
    pub retry_enqueued_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInterruptionEpochReadback {
    pub epoch: HostInterruptionEpoch,
    pub affected_executions: Vec<HostInterruptionAffectedExecution>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerBackpressureNotification {
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub provider_family: Option<String>,
    pub top_reason: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub state: String,
    pub updated_at: DateTime<Utc>,
    pub stale_after_ms: i64,
}

impl SchedulerBackpressureNotification {
    pub fn is_stale_at(&self, now: DateTime<Utc>) -> bool {
        self.updated_at + Duration::milliseconds(self.stale_after_ms) <= now
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RefreshQueueSummariesResult {
    pub notification: Option<SchedulerBackpressureNotification>,
}

pub async fn upsert_service_state(pool: &SqlitePool, state: &SchedulerServiceState) -> Result<()> {
    let mut tx = begin_immediate_with_retry(pool, "scheduler.upsert_service_state").await?;
    upsert_service_state_tx(&mut tx, state).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_service_state_tx(
    tx: &mut Transaction<'_, Sqlite>,
    state: &SchedulerServiceState,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_service_state
           (scope, scope_id, last_served_at, last_claimed_work_item_id, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5)
           ON CONFLICT(scope, scope_id) DO UPDATE SET
             last_served_at = excluded.last_served_at,
             last_claimed_work_item_id = excluded.last_claimed_work_item_id,
             updated_at = excluded.updated_at"#,
    )
    .bind(&state.scope)
    .bind(&state.scope_id)
    .bind(state.last_served_at.map(|value| value.to_rfc3339()))
    .bind(&state.last_claimed_work_item_id)
    .bind(state.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("upsert scheduler service state")?;
    Ok(())
}

pub async fn get_service_state(
    pool: &SqlitePool,
    scope: &str,
    scope_id: &str,
) -> Result<Option<SchedulerServiceState>> {
    let mut tx = pool.begin().await?;
    let state = get_service_state_tx(&mut tx, scope, scope_id).await?;
    tx.commit().await?;
    Ok(state)
}

pub async fn get_service_state_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scope: &str,
    scope_id: &str,
) -> Result<Option<SchedulerServiceState>> {
    let row = sqlx::query(
        r#"SELECT scope, scope_id, last_served_at, last_claimed_work_item_id, updated_at
           FROM scheduler_service_state
           WHERE scope = ?1 AND scope_id = ?2"#,
    )
    .bind(scope)
    .bind(scope_id)
    .fetch_optional(&mut **tx)
    .await
    .context("get scheduler service state")?;

    row.map(parse_service_state_row).transpose()
}

pub async fn list_queue_summaries(pool: &SqlitePool) -> Result<Vec<SchedulerQueueSummary>> {
    let rows = sqlx::query(
        r#"SELECT scope, scope_id, run_id, stage_execution_id, provider_family, top_reason,
                  queued_count, oldest_queued_age_ms, global_queue_depth, stale_after_ms, updated_at
           FROM scheduler_queue_summaries
           ORDER BY updated_at DESC, scope ASC, scope_id ASC"#,
    )
    .fetch_all(pool)
    .await
    .context("list scheduler queue summaries")?;

    rows.into_iter().map(parse_queue_summary_row).collect()
}

pub async fn upsert_queue_summary(
    pool: &SqlitePool,
    summary: &SchedulerQueueSummary,
) -> Result<()> {
    let provider_family = summary.provider_family.as_deref().unwrap_or("");

    if summary.queued_count <= 0 {
        sqlx::query(
            r#"DELETE FROM scheduler_queue_summaries
               WHERE scope = ?1
                 AND scope_id = ?2
                 AND provider_family = ?3
                 AND top_reason = ?4"#,
        )
        .bind(&summary.scope)
        .bind(&summary.scope_id)
        .bind(provider_family)
        .bind(&summary.top_reason)
        .execute(pool)
        .await
        .context("clear zero-count scheduler queue summary")?;
        return Ok(());
    }

    sqlx::query(
        r#"INSERT INTO scheduler_queue_summaries
           (scope, scope_id, run_id, stage_execution_id, provider_family, top_reason,
            queued_count, oldest_queued_age_ms, global_queue_depth, stale_after_ms, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
           ON CONFLICT(scope, scope_id, provider_family, top_reason) DO UPDATE SET
             run_id = excluded.run_id,
             stage_execution_id = excluded.stage_execution_id,
             queued_count = excluded.queued_count,
             oldest_queued_age_ms = excluded.oldest_queued_age_ms,
             global_queue_depth = excluded.global_queue_depth,
             stale_after_ms = excluded.stale_after_ms,
             updated_at = excluded.updated_at"#,
    )
    .bind(&summary.scope)
    .bind(&summary.scope_id)
    .bind(&summary.run_id)
    .bind(&summary.stage_execution_id)
    .bind(provider_family)
    .bind(&summary.top_reason)
    .bind(summary.queued_count)
    .bind(summary.oldest_queued_age_ms)
    .bind(summary.global_queue_depth)
    .bind(summary.stale_after_ms)
    .bind(summary.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .context("upsert scheduler queue summary")?;
    Ok(())
}

pub async fn list_queue_summaries_by_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<SchedulerQueueSummary>> {
    let rows = sqlx::query(
        r#"SELECT scope, scope_id, run_id, stage_execution_id, provider_family, top_reason,
                  queued_count, oldest_queued_age_ms, global_queue_depth, stale_after_ms, updated_at
           FROM scheduler_queue_summaries
           WHERE run_id = ?1
           ORDER BY updated_at DESC, scope ASC, scope_id ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("list scheduler queue summaries by run")?;

    rows.into_iter().map(parse_queue_summary_row).collect()
}

pub async fn list_queue_summaries_by_stage(
    pool: &SqlitePool,
    stage_execution_id: &str,
) -> Result<Vec<SchedulerQueueSummary>> {
    let rows = sqlx::query(
        r#"SELECT scope, scope_id, run_id, stage_execution_id, provider_family, top_reason,
                  queued_count, oldest_queued_age_ms, global_queue_depth, stale_after_ms, updated_at
           FROM scheduler_queue_summaries
           WHERE stage_execution_id = ?1
           ORDER BY updated_at DESC, scope ASC, scope_id ASC"#,
    )
    .bind(stage_execution_id)
    .fetch_all(pool)
    .await
    .context("list scheduler queue summaries by stage")?;

    rows.into_iter().map(parse_queue_summary_row).collect()
}

pub async fn insert_health_snapshot(
    pool: &SqlitePool,
    snapshot: &SchedulerHealthSnapshot,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_health_snapshots
           (id, queued_count, oldest_queued_age_ms, global_queue_depth, active_agent_executions,
            db_writer_wait_p95_ms, command_latency_p95_ms_json, last_host_interruption_epoch_id,
            sustained_backpressure_state, fire_consecutive_snapshots, clear_consecutive_snapshots,
            stale_after_ms, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
    )
    .bind(&snapshot.id)
    .bind(snapshot.queued_count)
    .bind(snapshot.oldest_queued_age_ms)
    .bind(snapshot.global_queue_depth)
    .bind(snapshot.active_agent_executions)
    .bind(snapshot.db_writer_wait_p95_ms)
    .bind(&snapshot.command_latency_p95_ms_json)
    .bind(&snapshot.last_host_interruption_epoch_id)
    .bind(&snapshot.sustained_backpressure_state)
    .bind(snapshot.fire_consecutive_snapshots)
    .bind(snapshot.clear_consecutive_snapshots)
    .bind(snapshot.stale_after_ms)
    .bind(snapshot.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .context("insert scheduler health snapshot")?;
    Ok(())
}

pub async fn latest_health_snapshot(pool: &SqlitePool) -> Result<Option<SchedulerHealthSnapshot>> {
    let row = sqlx::query(
        r#"SELECT id, queued_count, oldest_queued_age_ms, global_queue_depth,
                  active_agent_executions, db_writer_wait_p95_ms, command_latency_p95_ms_json,
                  last_host_interruption_epoch_id, sustained_backpressure_state,
                  fire_consecutive_snapshots, clear_consecutive_snapshots, stale_after_ms, updated_at
           FROM scheduler_health_snapshots
           ORDER BY updated_at DESC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .context("latest scheduler health snapshot")?;

    row.map(parse_health_snapshot_row).transpose()
}

pub async fn latest_backpressure_notification(
    pool: &SqlitePool,
) -> Result<Option<SchedulerBackpressureNotification>> {
    let Some(snapshot) = latest_health_snapshot(pool).await? else {
        return Ok(None);
    };

    let selected_summary = select_notification_summary(list_queue_summaries(pool).await?);
    Ok(Some(backpressure_notification_from_parts(
        &snapshot,
        selected_summary.as_ref(),
    )))
}

pub async fn record_db_writer_wait_observation(
    pool: &SqlitePool,
    operation: &str,
    wait_ms: i64,
    observed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_db_writer_observations
           (id, operation, wait_ms, observed_at)
           VALUES (?1, ?2, ?3, ?4)"#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(operation)
    .bind(wait_ms.max(0))
    .bind(observed_at.to_rfc3339())
    .execute(pool)
    .await
    .context("record scheduler DB writer wait observation")?;
    Ok(())
}

pub async fn latest_db_writer_wait_p95_ms(pool: &SqlitePool) -> Result<Option<i64>> {
    let rows = sqlx::query(
        r#"SELECT wait_ms
           FROM scheduler_db_writer_observations
           ORDER BY observed_at DESC
           LIMIT ?1"#,
    )
    .bind(DB_WRITER_OBSERVATION_LIMIT)
    .fetch_all(pool)
    .await
    .context("load scheduler DB writer wait observations")?;

    Ok(p95_ms(
        rows.into_iter().map(|row| row.get("wait_ms")).collect(),
    ))
}

pub async fn command_latency_p95_ms_json(pool: &SqlitePool) -> Result<Option<String>> {
    let rows = sqlx::query(
        r#"SELECT command_type, created_at, completed_at
           FROM command_journal
           WHERE completed_at IS NOT NULL
             AND command_type IN ('ApproveStage', 'RetryStage', 'CancelRun')
           ORDER BY completed_at DESC
           LIMIT ?1"#,
    )
    .bind(COMMAND_LATENCY_OBSERVATION_LIMIT)
    .fetch_all(pool)
    .await
    .context("load command latency observations")?;

    command_latency_json_from_rows(rows)
}

pub async fn list_active_execution_counts_by_provider(
    pool: &SqlitePool,
) -> Result<Vec<SchedulerProviderActiveCount>> {
    let counts = ActiveCounts::load(pool).await?;
    Ok(counts
        .by_provider
        .into_iter()
        .map(
            |(provider_family, active_count)| SchedulerProviderActiveCount {
                provider_family,
                active_count,
            },
        )
        .collect())
}

pub async fn queue_position_hint_by_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<SchedulerQueuePositionHint>> {
    queue_position_hint_by_scope(pool, QueuePositionScope::Run(run_id)).await
}

pub async fn queue_position_hint_by_stage(
    pool: &SqlitePool,
    stage_execution_id: &str,
) -> Result<Option<SchedulerQueuePositionHint>> {
    queue_position_hint_by_scope(pool, QueuePositionScope::Stage(stage_execution_id)).await
}

pub async fn insert_host_interruption_epoch(
    pool: &SqlitePool,
    epoch: &HostInterruptionEpoch,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_host_interruption_epoch_tx(&mut tx, epoch).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_host_interruption_epoch_tx(
    tx: &mut Transaction<'_, Sqlite>,
    epoch: &HostInterruptionEpoch,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO host_interruption_epochs
           (id, kind, started_at, ended_at, monotonic_gap_ms, wall_clock_gap_ms, details_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(&epoch.id)
    .bind(&epoch.kind)
    .bind(epoch.started_at.to_rfc3339())
    .bind(epoch.ended_at.map(|value| value.to_rfc3339()))
    .bind(epoch.monotonic_gap_ms)
    .bind(epoch.wall_clock_gap_ms)
    .bind(&epoch.details_json)
    .bind(epoch.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("insert host interruption epoch")?;
    Ok(())
}

pub async fn insert_host_interruption_affected_execution(
    pool: &SqlitePool,
    affected: &HostInterruptionAffectedExecution,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_host_interruption_affected_execution_tx(&mut tx, affected).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_host_interruption_affected_execution_tx(
    tx: &mut Transaction<'_, Sqlite>,
    affected: &HostInterruptionAffectedExecution,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO host_interruption_affected_executions
           (epoch_id, agent_execution_id, run_id, stage_execution_id, provider_family,
            action, previous_status, settlement_status, cleanup_status, quota_budget_effect,
            retry_enqueued_at, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
    )
    .bind(&affected.epoch_id)
    .bind(&affected.agent_execution_id)
    .bind(&affected.run_id)
    .bind(&affected.stage_execution_id)
    .bind(&affected.provider_family)
    .bind(&affected.action)
    .bind(&affected.previous_status)
    .bind(&affected.settlement_status)
    .bind(&affected.cleanup_status)
    .bind(&affected.quota_budget_effect)
    .bind(affected.retry_enqueued_at.map(|value| value.to_rfc3339()))
    .bind(affected.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("insert host interruption affected execution")?;
    Ok(())
}

pub async fn list_host_interruption_epochs_by_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<HostInterruptionEpochReadback>> {
    let epoch_rows = sqlx::query(
        r#"SELECT DISTINCT e.id, e.kind, e.started_at, e.ended_at, e.monotonic_gap_ms,
                  e.wall_clock_gap_ms, e.details_json, e.created_at
           FROM host_interruption_epochs e
           INNER JOIN host_interruption_affected_executions a ON a.epoch_id = e.id
           WHERE a.run_id = ?1
           ORDER BY e.started_at DESC, e.id ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("list host interruption epochs by run")?;

    let mut readbacks = Vec::with_capacity(epoch_rows.len());
    for row in epoch_rows {
        let epoch = parse_host_interruption_epoch_row(row)?;
        let affected_rows = sqlx::query(
            r#"SELECT epoch_id, agent_execution_id, run_id, stage_execution_id, provider_family,
                      action, previous_status, settlement_status, cleanup_status,
                      quota_budget_effect, retry_enqueued_at, created_at
               FROM host_interruption_affected_executions
               WHERE epoch_id = ?1 AND run_id = ?2
               ORDER BY created_at ASC, agent_execution_id ASC"#,
        )
        .bind(&epoch.id)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("list host interruption affected executions by epoch and run")?;
        let affected_executions = affected_rows
            .into_iter()
            .map(parse_host_interruption_affected_execution_row)
            .collect::<Result<Vec<_>>>()?;
        readbacks.push(HostInterruptionEpochReadback {
            epoch,
            affected_executions,
        });
    }

    Ok(readbacks)
}

pub async fn list_host_interruption_affected_executions_by_epoch(
    pool: &SqlitePool,
    epoch_id: &str,
) -> Result<Vec<HostInterruptionAffectedExecution>> {
    let rows = sqlx::query(
        r#"SELECT epoch_id, agent_execution_id, run_id, stage_execution_id, provider_family,
                  action, previous_status, settlement_status, cleanup_status,
                  quota_budget_effect, retry_enqueued_at, created_at
           FROM host_interruption_affected_executions
           WHERE epoch_id = ?1
           ORDER BY created_at ASC, agent_execution_id ASC"#,
    )
    .bind(epoch_id)
    .fetch_all(pool)
    .await
    .context("list host interruption affected executions by epoch")?;

    rows.into_iter()
        .map(parse_host_interruption_affected_execution_row)
        .collect()
}

pub async fn refresh_queue_summaries(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<()> {
    refresh_queue_summaries_for_notification(pool, capacity)
        .await
        .map(|_| ())
}

pub async fn refresh_queue_summaries_for_notification(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<RefreshQueueSummariesResult> {
    let now = Utc::now();
    let writer_wait_started_at = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "scheduler.refresh_queue_summaries")
        .await
        .context("begin refresh scheduler queue summaries")?;
    let writer_wait_ms = elapsed_millis_i64(writer_wait_started_at);
    let result = refresh_queue_summaries_for_notification_tx(
        &mut tx,
        capacity,
        now,
        "refresh_queue_summaries",
        writer_wait_ms,
    )
    .await?;
    tx.commit()
        .await
        .context("commit refresh scheduler queue summaries")?;
    Ok(result)
}

pub async fn refresh_queue_summaries_for_notification_tx(
    tx: &mut Transaction<'_, Sqlite>,
    capacity: &InvokeAgentCapacityConfig,
    now: DateTime<Utc>,
    operation: &str,
    writer_wait_ms: i64,
) -> Result<RefreshQueueSummariesResult> {
    let pending = load_pending_invoke_agent_work_tx(tx, now).await?;
    let active = ActiveCounts::load_tx(tx).await?;
    let global_queue_depth = pending.len() as i64;
    let mut grouped: BTreeMap<SummaryKey, SummaryAccumulator> = BTreeMap::new();

    for item in &pending {
        let reason = top_reason_for_item(item, &active, capacity);
        let age_ms = (now - item.scheduled_at).num_milliseconds().max(0);

        let global_key = SummaryKey {
            scope: "global".to_string(),
            scope_id: String::new(),
            run_id: None,
            stage_execution_id: None,
            provider_family: item.provider_family.clone(),
            top_reason: reason.clone(),
        };
        grouped
            .entry(global_key)
            .or_default()
            .add(age_ms, global_queue_depth);

        if let Some(run_id) = item.run_id.clone() {
            let run_key = SummaryKey {
                scope: "run".to_string(),
                scope_id: run_id.clone(),
                run_id: Some(run_id),
                stage_execution_id: None,
                provider_family: item.provider_family.clone(),
                top_reason: reason.clone(),
            };
            grouped
                .entry(run_key)
                .or_default()
                .add(age_ms, global_queue_depth);
        }

        if let Some(stage_execution_id) = item.stage_execution_id.clone() {
            let stage_key = SummaryKey {
                scope: "stage".to_string(),
                scope_id: stage_execution_id.clone(),
                run_id: item.run_id.clone(),
                stage_execution_id: Some(stage_execution_id),
                provider_family: item.provider_family.clone(),
                top_reason: reason,
            };
            grouped
                .entry(stage_key)
                .or_default()
                .add(age_ms, global_queue_depth);
        }
    }

    sqlx::query("DELETE FROM scheduler_queue_summaries")
        .execute(&mut **tx)
        .await
        .context("clear scheduler queue summaries")?;
    insert_db_writer_wait_observation_tx(tx, operation, writer_wait_ms, now)
        .await
        .context("record scheduler refresh DB writer wait")?;

    let mut refreshed_summaries = Vec::with_capacity(grouped.len());
    for (key, accumulator) in grouped {
        let summary = SchedulerQueueSummary {
            scope: key.scope,
            scope_id: key.scope_id,
            run_id: key.run_id,
            stage_execution_id: key.stage_execution_id,
            provider_family: key.provider_family,
            top_reason: key.top_reason,
            queued_count: accumulator.queued_count,
            oldest_queued_age_ms: accumulator.oldest_queued_age_ms,
            global_queue_depth: accumulator.global_queue_depth,
            stale_after_ms: STALE_AFTER_MS,
            updated_at: now,
        };
        insert_queue_summary_tx(&mut *tx, &summary)
            .await
            .context("insert refreshed scheduler queue summary")?;
        refreshed_summaries.push(summary);
    }

    let oldest_queued_age_ms = pending
        .iter()
        .map(|item| (now - item.scheduled_at).num_milliseconds().max(0))
        .max()
        .unwrap_or(0);
    let previous_snapshot = latest_health_snapshot_tx(&mut *tx).await?;
    let previous_state = previous_snapshot
        .as_ref()
        .map(|snapshot| snapshot.sustained_backpressure_state.as_str())
        .unwrap_or("clear");
    let hysteresis = next_sustained_backpressure_state(
        previous_snapshot.as_ref(),
        global_queue_depth,
        oldest_queued_age_ms,
    );
    let snapshot = SchedulerHealthSnapshot {
        id: uuid::Uuid::new_v4().to_string(),
        queued_count: global_queue_depth,
        oldest_queued_age_ms,
        global_queue_depth,
        active_agent_executions: active.global,
        db_writer_wait_p95_ms: db_writer_wait_p95_ms_tx(&mut *tx).await?,
        command_latency_p95_ms_json: command_latency_p95_ms_json_tx(&mut *tx).await?,
        last_host_interruption_epoch_id: latest_host_interruption_epoch_id_tx(&mut *tx).await?,
        sustained_backpressure_state: hysteresis.state,
        fire_consecutive_snapshots: hysteresis.fire_consecutive_snapshots,
        clear_consecutive_snapshots: hysteresis.clear_consecutive_snapshots,
        stale_after_ms: STALE_AFTER_MS,
        updated_at: now,
    };
    insert_health_snapshot_tx(&mut *tx, &snapshot)
        .await
        .context("insert refreshed scheduler health snapshot")?;

    let notification =
        notification_for_state_transition(&previous_state, &snapshot, refreshed_summaries);
    Ok(RefreshQueueSummariesResult { notification })
}

async fn latest_host_interruption_epoch_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<Option<String>> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM host_interruption_epochs
           ORDER BY started_at DESC, created_at DESC, id ASC
           LIMIT 1"#,
    )
    .fetch_optional(&mut **tx)
    .await
    .context("load latest host interruption epoch id")
}

async fn latest_health_snapshot_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<Option<SchedulerHealthSnapshot>> {
    let row = sqlx::query(
        r#"SELECT id, queued_count, oldest_queued_age_ms, global_queue_depth,
                  active_agent_executions, db_writer_wait_p95_ms, command_latency_p95_ms_json,
                  last_host_interruption_epoch_id, sustained_backpressure_state,
                  fire_consecutive_snapshots, clear_consecutive_snapshots, stale_after_ms, updated_at
           FROM scheduler_health_snapshots
           ORDER BY updated_at DESC
           LIMIT 1"#,
    )
    .fetch_optional(&mut **tx)
    .await
    .context("latest scheduler health snapshot in transaction")?;

    row.map(parse_health_snapshot_row).transpose()
}

async fn insert_queue_summary_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    summary: &SchedulerQueueSummary,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_queue_summaries
           (scope, scope_id, run_id, stage_execution_id, provider_family, top_reason,
            queued_count, oldest_queued_age_ms, global_queue_depth, stale_after_ms, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(&summary.scope)
    .bind(&summary.scope_id)
    .bind(&summary.run_id)
    .bind(&summary.stage_execution_id)
    .bind(summary.provider_family.as_deref().unwrap_or(""))
    .bind(&summary.top_reason)
    .bind(summary.queued_count)
    .bind(summary.oldest_queued_age_ms)
    .bind(summary.global_queue_depth)
    .bind(summary.stale_after_ms)
    .bind(summary.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_health_snapshot_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    snapshot: &SchedulerHealthSnapshot,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_health_snapshots
           (id, queued_count, oldest_queued_age_ms, global_queue_depth, active_agent_executions,
            db_writer_wait_p95_ms, command_latency_p95_ms_json, last_host_interruption_epoch_id,
            sustained_backpressure_state, fire_consecutive_snapshots, clear_consecutive_snapshots,
            stale_after_ms, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
    )
    .bind(&snapshot.id)
    .bind(snapshot.queued_count)
    .bind(snapshot.oldest_queued_age_ms)
    .bind(snapshot.global_queue_depth)
    .bind(snapshot.active_agent_executions)
    .bind(snapshot.db_writer_wait_p95_ms)
    .bind(&snapshot.command_latency_p95_ms_json)
    .bind(&snapshot.last_host_interruption_epoch_id)
    .bind(&snapshot.sustained_backpressure_state)
    .bind(snapshot.fire_consecutive_snapshots)
    .bind(snapshot.clear_consecutive_snapshots)
    .bind(snapshot.stale_after_ms)
    .bind(snapshot.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_db_writer_wait_observation_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation: &str,
    wait_ms: i64,
    observed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_db_writer_observations
           (id, operation, wait_ms, observed_at)
           VALUES (?1, ?2, ?3, ?4)"#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(operation)
    .bind(wait_ms.max(0))
    .bind(observed_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn db_writer_wait_p95_ms_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<Option<i64>> {
    let rows = sqlx::query(
        r#"SELECT wait_ms
           FROM scheduler_db_writer_observations
           ORDER BY observed_at DESC
           LIMIT ?1"#,
    )
    .bind(DB_WRITER_OBSERVATION_LIMIT)
    .fetch_all(&mut **tx)
    .await
    .context("load scheduler DB writer wait observations")?;

    Ok(p95_ms(
        rows.into_iter().map(|row| row.get("wait_ms")).collect(),
    ))
}

async fn command_latency_p95_ms_json_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<Option<String>> {
    let rows = sqlx::query(
        r#"SELECT command_type, created_at, completed_at
           FROM command_journal
           WHERE completed_at IS NOT NULL
             AND command_type IN ('ApproveStage', 'RetryStage', 'CancelRun')
           ORDER BY completed_at DESC
           LIMIT ?1"#,
    )
    .bind(COMMAND_LATENCY_OBSERVATION_LIMIT)
    .fetch_all(&mut **tx)
    .await
    .context("load command latency observations")?;

    command_latency_json_from_rows(rows)
}

#[derive(Clone, Debug)]
struct PendingInvokeAgentWork {
    run_id: Option<String>,
    stage_execution_id: Option<String>,
    provider_family: Option<String>,
    scheduled_at: DateTime<Utc>,
    startup_recovery_requeued: bool,
}

#[derive(Clone, Debug)]
struct QueuePositionCandidate {
    run_id: Option<String>,
    stage_execution_id: Option<String>,
    scheduled_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug)]
enum QueuePositionScope<'a> {
    Run(&'a str),
    Stage(&'a str),
}

impl QueuePositionScope<'_> {
    fn scope_name(&self) -> &'static str {
        match self {
            Self::Run(_) => "run",
            Self::Stage(_) => "stage",
        }
    }

    fn scope_id(&self) -> &str {
        match self {
            Self::Run(run_id) => run_id,
            Self::Stage(stage_execution_id) => stage_execution_id,
        }
    }

    fn matches(&self, candidate: &QueuePositionCandidate) -> bool {
        match self {
            Self::Run(run_id) => candidate.run_id.as_deref() == Some(*run_id),
            Self::Stage(stage_execution_id) => {
                candidate.stage_execution_id.as_deref() == Some(*stage_execution_id)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SummaryKey {
    scope: String,
    scope_id: String,
    run_id: Option<String>,
    stage_execution_id: Option<String>,
    provider_family: Option<String>,
    top_reason: String,
}

#[derive(Clone, Debug, Default)]
struct SummaryAccumulator {
    queued_count: i64,
    oldest_queued_age_ms: i64,
    global_queue_depth: i64,
}

impl SummaryAccumulator {
    fn add(&mut self, age_ms: i64, global_queue_depth: i64) {
        self.queued_count += 1;
        self.oldest_queued_age_ms = self.oldest_queued_age_ms.max(age_ms);
        self.global_queue_depth = global_queue_depth;
    }
}

#[derive(Clone, Debug, Default)]
struct ActiveCounts {
    global: i64,
    by_run: BTreeMap<String, i64>,
    by_provider: BTreeMap<String, i64>,
}

impl ActiveCounts {
    async fn load(pool: &SqlitePool) -> Result<Self> {
        let mut counts = Self::default();

        let global_agent_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(pool)
                .await
                .context("count running agent executions")?;
        counts.global = global_agent_count;

        let provider_rows = sqlx::query(
            r#"SELECT COALESCE(provider_family, '') AS provider_family, COUNT(*) AS count
               FROM agent_executions
               WHERE status = 'running'
               GROUP BY COALESCE(provider_family, '')"#,
        )
        .fetch_all(pool)
        .await
        .context("count running agent executions by provider")?;
        for row in provider_rows {
            let provider_family: String = row.get("provider_family");
            if !provider_family.is_empty() {
                counts
                    .by_provider
                    .insert(provider_family, row.get::<i64, _>("count"));
            }
        }

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(pool)
        .await
        .context("count running agent executions by run")?;
        for row in run_rows {
            counts
                .by_run
                .insert(row.get("run_id"), row.get::<i64, _>("count"));
        }

        let running_work = load_running_invoke_agent_work(pool).await?;
        let mut running_work_by_run = BTreeMap::new();
        let mut running_work_by_provider = BTreeMap::new();
        counts.global = counts.global.max(running_work.len() as i64);
        for item in running_work {
            if let Some(run_id) = item.run_id {
                *running_work_by_run.entry(run_id).or_insert(0) += 1;
            }
            if let Some(provider_family) = item.provider_family {
                *running_work_by_provider.entry(provider_family).or_insert(0) += 1;
            }
        }
        for (run_id, count) in running_work_by_run {
            let entry = counts.by_run.entry(run_id).or_insert(0);
            *entry = (*entry).max(count);
        }
        for (provider_family, count) in running_work_by_provider {
            let entry = counts.by_provider.entry(provider_family).or_insert(0);
            *entry = (*entry).max(count);
        }

        Ok(counts)
    }

    async fn load_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<Self> {
        let mut counts = Self::default();

        let global_agent_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(&mut **tx)
                .await
                .context("count running agent executions")?;
        counts.global = global_agent_count;

        let provider_rows = sqlx::query(
            r#"SELECT COALESCE(provider_family, '') AS provider_family, COUNT(*) AS count
               FROM agent_executions
               WHERE status = 'running'
               GROUP BY COALESCE(provider_family, '')"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running agent executions by provider")?;
        for row in provider_rows {
            let provider_family: String = row.get("provider_family");
            if !provider_family.is_empty() {
                counts
                    .by_provider
                    .insert(provider_family, row.get::<i64, _>("count"));
            }
        }

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running agent executions by run")?;
        for row in run_rows {
            counts
                .by_run
                .insert(row.get("run_id"), row.get::<i64, _>("count"));
        }

        let running_work = load_running_invoke_agent_work_tx(tx).await?;
        let mut running_work_by_run = BTreeMap::new();
        let mut running_work_by_provider = BTreeMap::new();
        counts.global = counts.global.max(running_work.len() as i64);
        for item in running_work {
            if let Some(run_id) = item.run_id {
                *running_work_by_run.entry(run_id).or_insert(0) += 1;
            }
            if let Some(provider_family) = item.provider_family {
                *running_work_by_provider.entry(provider_family).or_insert(0) += 1;
            }
        }
        for (run_id, count) in running_work_by_run {
            let entry = counts.by_run.entry(run_id).or_insert(0);
            *entry = (*entry).max(count);
        }
        for (provider_family, count) in running_work_by_provider {
            let entry = counts.by_provider.entry(provider_family).or_insert(0);
            *entry = (*entry).max(count);
        }

        Ok(counts)
    }
}

fn top_reason_for_item(
    item: &PendingInvokeAgentWork,
    active: &ActiveCounts,
    capacity: &InvokeAgentCapacityConfig,
) -> String {
    if item.startup_recovery_requeued {
        return "startup_recovery_backpressure".to_string();
    }

    if let Some(run_id) = item.run_id.as_deref() {
        if active.by_run.get(run_id).copied().unwrap_or(0)
            >= capacity.per_run_active_agent_executions as i64
        {
            return "run_capacity".to_string();
        }
    }

    if let Some(provider_family) = item.provider_family.as_deref() {
        if let Ok(family) = provider_family.parse::<ProviderFamily>() {
            if active
                .by_provider
                .get(provider_family)
                .copied()
                .unwrap_or(0)
                >= capacity.provider_cap(family) as i64
            {
                return "provider_capacity".to_string();
            }
        }
    }

    if active.global >= capacity.global_active_agent_executions as i64 {
        return "global_capacity".to_string();
    }

    "queued".to_string()
}

struct BackpressureHysteresis {
    state: String,
    fire_consecutive_snapshots: i64,
    clear_consecutive_snapshots: i64,
}

fn next_sustained_backpressure_state(
    previous_snapshot: Option<&SchedulerHealthSnapshot>,
    queued_count: i64,
    oldest_queued_age_ms: i64,
) -> BackpressureHysteresis {
    let pressure_high =
        queued_count > 0 && oldest_queued_age_ms >= SUSTAINED_BACKPRESSURE_THRESHOLD_MS;
    let pressure_clear =
        queued_count == 0 || oldest_queued_age_ms < SUSTAINED_BACKPRESSURE_CLEAR_MS;
    let previous_state = previous_snapshot
        .map(|snapshot| snapshot.sustained_backpressure_state.as_str())
        .unwrap_or("clear");
    let previous_fire = previous_snapshot
        .map(|snapshot| snapshot.fire_consecutive_snapshots)
        .unwrap_or(0);
    let previous_clear = previous_snapshot
        .map(|snapshot| snapshot.clear_consecutive_snapshots)
        .unwrap_or(0);

    if pressure_high {
        let fire_consecutive_snapshots = if matches!(previous_state, "pending_active" | "active") {
            (previous_fire + 1).min(2)
        } else {
            1
        };
        let state = if fire_consecutive_snapshots >= 2 {
            "active"
        } else {
            "pending_active"
        };
        BackpressureHysteresis {
            state: state.to_string(),
            fire_consecutive_snapshots,
            clear_consecutive_snapshots: 0,
        }
    } else if pressure_clear {
        if matches!(previous_state, "active" | "pending_clear") {
            let clear_consecutive_snapshots = (previous_clear + 1).min(2);
            let state = if clear_consecutive_snapshots >= 2 {
                "clear"
            } else {
                "pending_clear"
            };
            BackpressureHysteresis {
                state: state.to_string(),
                fire_consecutive_snapshots: 0,
                clear_consecutive_snapshots,
            }
        } else {
            BackpressureHysteresis {
                state: "clear".to_string(),
                fire_consecutive_snapshots: 0,
                clear_consecutive_snapshots: 2,
            }
        }
    } else {
        if matches!(previous_state, "active" | "pending_clear") {
            BackpressureHysteresis {
                state: "active".to_string(),
                fire_consecutive_snapshots: 2,
                clear_consecutive_snapshots: 0,
            }
        } else {
            BackpressureHysteresis {
                state: "clear".to_string(),
                fire_consecutive_snapshots: 0,
                clear_consecutive_snapshots: 0,
            }
        }
    }
}

fn select_notification_summary(
    summaries: Vec<SchedulerQueueSummary>,
) -> Option<SchedulerQueueSummary> {
    summaries.into_iter().max_by(|left, right| {
        reason_priority(&left.top_reason)
            .cmp(&reason_priority(&right.top_reason))
            .then_with(|| left.oldest_queued_age_ms.cmp(&right.oldest_queued_age_ms))
            .then_with(|| left.queued_count.cmp(&right.queued_count))
            .then_with(|| scope_priority(&left.scope).cmp(&scope_priority(&right.scope)))
            .then_with(|| right.scope_id.cmp(&left.scope_id))
    })
}

fn reason_priority(reason: &str) -> i64 {
    match reason {
        "run_capacity" => 5,
        "provider_capacity" => 4,
        "global_capacity" => 3,
        "startup_recovery_backpressure" => 2,
        "db_writer_capacity" => 1,
        _ => 0,
    }
}

fn scope_priority(scope: &str) -> i64 {
    match scope {
        "stage" => 2,
        "run" => 1,
        _ => 0,
    }
}

fn backpressure_notification_from_parts(
    snapshot: &SchedulerHealthSnapshot,
    summary: Option<&SchedulerQueueSummary>,
) -> SchedulerBackpressureNotification {
    SchedulerBackpressureNotification {
        run_id: summary.and_then(|value| value.run_id.clone()),
        stage_execution_id: summary.and_then(|value| value.stage_execution_id.clone()),
        provider_family: summary.and_then(|value| value.provider_family.clone()),
        top_reason: summary
            .map(|value| value.top_reason.clone())
            .unwrap_or_else(|| "clear".to_string()),
        queued_count: summary
            .map(|value| value.queued_count)
            .unwrap_or(snapshot.queued_count),
        oldest_queued_age_ms: snapshot.oldest_queued_age_ms,
        global_queue_depth: snapshot.global_queue_depth,
        state: snapshot.sustained_backpressure_state.clone(),
        updated_at: snapshot.updated_at,
        stale_after_ms: snapshot.stale_after_ms,
    }
}

fn notification_for_state_transition(
    previous_state: &str,
    snapshot: &SchedulerHealthSnapshot,
    summaries: Vec<SchedulerQueueSummary>,
) -> Option<SchedulerBackpressureNotification> {
    let next_state = snapshot.sustained_backpressure_state.as_str();
    let crossed_notification_boundary = (next_state == "active"
        && previous_state == "pending_active"
        && snapshot.fire_consecutive_snapshots >= 2)
        || (next_state == "clear"
            && previous_state == "pending_clear"
            && snapshot.clear_consecutive_snapshots >= 2);
    if !crossed_notification_boundary {
        return None;
    }

    let selected_summary = select_notification_summary(summaries);
    Some(backpressure_notification_from_parts(
        snapshot,
        selected_summary.as_ref(),
    ))
}

async fn load_pending_invoke_agent_work_tx(
    tx: &mut Transaction<'_, Sqlite>,
    now: DateTime<Utc>,
) -> Result<Vec<PendingInvokeAgentWork>> {
    let rows = sqlx::query(
        r#"SELECT payload_json, run_id, scheduled_at
           FROM work_items
           WHERE kind = 'invoke_agent' AND status = 'pending' AND scheduled_at <= ?1
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(now.to_rfc3339())
    .fetch_all(&mut **tx)
    .await
    .context("load pending InvokeAgent work for scheduler summaries")?;

    rows.into_iter().map(parse_invoke_agent_work_row).collect()
}

async fn queue_position_hint_by_scope(
    pool: &SqlitePool,
    scope: QueuePositionScope<'_>,
) -> Result<Option<SchedulerQueuePositionHint>> {
    let now = Utc::now();
    let rows = sqlx::query(
        r#"SELECT payload_json, run_id, scheduled_at
           FROM work_items
           WHERE kind = 'invoke_agent' AND status = 'pending' AND scheduled_at <= ?1
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(now.to_rfc3339())
    .fetch_all(pool)
    .await
    .context("load pending InvokeAgent queue for position hint")?;

    let global_queue_depth = rows.len() as i64;
    let mut first_position = None;
    let mut scoped_queued_count = 0_i64;
    let mut oldest_queued_age_ms = 0_i64;
    let mut run_id = None;
    let mut stage_execution_id = None;

    for (index, row) in rows.into_iter().enumerate() {
        let candidate = parse_queue_position_candidate_row(row)?;
        if !scope.matches(&candidate) {
            continue;
        }

        let age_ms = (now - candidate.scheduled_at).num_milliseconds().max(0);
        oldest_queued_age_ms = oldest_queued_age_ms.max(age_ms);
        scoped_queued_count += 1;
        run_id = run_id.or(candidate.run_id.clone());
        stage_execution_id = stage_execution_id.or(candidate.stage_execution_id.clone());
        first_position.get_or_insert(index as i64 + 1);
    }

    let Some(queue_position) = first_position else {
        return Ok(None);
    };

    Ok(Some(SchedulerQueuePositionHint {
        scope: scope.scope_name().to_string(),
        scope_id: scope.scope_id().to_string(),
        run_id,
        stage_execution_id,
        queue_position,
        global_queue_depth,
        queued_ahead_count: queue_position - 1,
        scoped_queued_count,
        oldest_queued_age_ms,
        updated_at: now,
    }))
}

async fn load_running_invoke_agent_work(pool: &SqlitePool) -> Result<Vec<PendingInvokeAgentWork>> {
    let rows = sqlx::query(
        r#"SELECT payload_json, run_id, scheduled_at
           FROM work_items
           WHERE kind = 'invoke_agent' AND status = 'running'"#,
    )
    .fetch_all(pool)
    .await
    .context("load running InvokeAgent work for scheduler active counts")?;

    rows.into_iter().map(parse_invoke_agent_work_row).collect()
}

async fn load_running_invoke_agent_work_tx(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<PendingInvokeAgentWork>> {
    let rows = sqlx::query(
        r#"SELECT payload_json, run_id, scheduled_at
           FROM work_items
           WHERE kind = 'invoke_agent' AND status = 'running'"#,
    )
    .fetch_all(&mut **tx)
    .await
    .context("load running InvokeAgent work for scheduler active counts")?;

    rows.into_iter().map(parse_invoke_agent_work_row).collect()
}

fn parse_queue_position_candidate_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<QueuePositionCandidate> {
    let payload_json: String = row.get("payload_json");
    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).context("parse InvokeAgent work payload")?;
    let stage_execution_id = payload["stage_execution_id"].as_str().map(String::from);
    let scheduled_at = parse_datetime(row.get("scheduled_at"))?;

    Ok(QueuePositionCandidate {
        run_id: row.get("run_id"),
        stage_execution_id,
        scheduled_at,
    })
}

fn parse_invoke_agent_work_row(row: sqlx::sqlite::SqliteRow) -> Result<PendingInvokeAgentWork> {
    let payload_json: String = row.get("payload_json");
    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).context("parse InvokeAgent work payload")?;
    let provider_family = payload["provider"]
        .as_str()
        .and_then(ProviderFamily::canonicalize_known_alias);
    let stage_execution_id = payload["stage_execution_id"].as_str().map(String::from);
    let scheduled_at = parse_datetime(row.get("scheduled_at"))?;

    Ok(PendingInvokeAgentWork {
        run_id: row.get("run_id"),
        stage_execution_id,
        provider_family,
        scheduled_at,
        startup_recovery_requeued: payload.get("p061_startup_recovery").is_some(),
    })
}

fn parse_service_state_row(row: sqlx::sqlite::SqliteRow) -> Result<SchedulerServiceState> {
    Ok(SchedulerServiceState {
        scope: row.get("scope"),
        scope_id: row.get("scope_id"),
        last_served_at: parse_optional_datetime(row.get("last_served_at"))?,
        last_claimed_work_item_id: row.get("last_claimed_work_item_id"),
        updated_at: parse_datetime(row.get("updated_at"))?,
    })
}

fn parse_queue_summary_row(row: sqlx::sqlite::SqliteRow) -> Result<SchedulerQueueSummary> {
    let provider_family: String = row.get("provider_family");
    Ok(SchedulerQueueSummary {
        scope: row.get("scope"),
        scope_id: row.get("scope_id"),
        run_id: row.get("run_id"),
        stage_execution_id: row.get("stage_execution_id"),
        provider_family: if provider_family.is_empty() {
            None
        } else {
            Some(provider_family)
        },
        top_reason: row.get("top_reason"),
        queued_count: row.get("queued_count"),
        oldest_queued_age_ms: row.get("oldest_queued_age_ms"),
        global_queue_depth: row.get("global_queue_depth"),
        stale_after_ms: row.get("stale_after_ms"),
        updated_at: parse_datetime(row.get("updated_at"))?,
    })
}

fn parse_health_snapshot_row(row: sqlx::sqlite::SqliteRow) -> Result<SchedulerHealthSnapshot> {
    Ok(SchedulerHealthSnapshot {
        id: row.get("id"),
        queued_count: row.get("queued_count"),
        oldest_queued_age_ms: row.get("oldest_queued_age_ms"),
        global_queue_depth: row.get("global_queue_depth"),
        active_agent_executions: row.get("active_agent_executions"),
        db_writer_wait_p95_ms: row.get("db_writer_wait_p95_ms"),
        command_latency_p95_ms_json: row.get("command_latency_p95_ms_json"),
        last_host_interruption_epoch_id: row.get("last_host_interruption_epoch_id"),
        sustained_backpressure_state: row.get("sustained_backpressure_state"),
        fire_consecutive_snapshots: row.get("fire_consecutive_snapshots"),
        clear_consecutive_snapshots: row.get("clear_consecutive_snapshots"),
        stale_after_ms: row.get("stale_after_ms"),
        updated_at: parse_datetime(row.get("updated_at"))?,
    })
}

fn parse_host_interruption_epoch_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<HostInterruptionEpoch> {
    Ok(HostInterruptionEpoch {
        id: row.get("id"),
        kind: row.get("kind"),
        started_at: parse_datetime(row.get("started_at"))?,
        ended_at: parse_optional_datetime(row.get("ended_at"))?,
        monotonic_gap_ms: row.get("monotonic_gap_ms"),
        wall_clock_gap_ms: row.get("wall_clock_gap_ms"),
        details_json: row.get("details_json"),
        created_at: parse_datetime(row.get("created_at"))?,
    })
}

fn parse_host_interruption_affected_execution_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<HostInterruptionAffectedExecution> {
    Ok(HostInterruptionAffectedExecution {
        epoch_id: row.get("epoch_id"),
        agent_execution_id: row.get("agent_execution_id"),
        run_id: row.get("run_id"),
        stage_execution_id: row.get("stage_execution_id"),
        provider_family: row.get("provider_family"),
        action: row.get("action"),
        previous_status: row.get("previous_status"),
        settlement_status: row.get("settlement_status"),
        cleanup_status: row.get("cleanup_status"),
        quota_budget_effect: row.get("quota_budget_effect"),
        retry_enqueued_at: parse_optional_datetime(row.get("retry_enqueued_at"))?,
        created_at: parse_datetime(row.get("created_at"))?,
    })
}

fn command_latency_json_from_rows(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Option<String>> {
    let mut grouped: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for row in rows {
        let command_type: String = row.get("command_type");
        let Some(label) = command_latency_label(&command_type) else {
            continue;
        };
        let created_at = parse_datetime(row.get("created_at"))?;
        let Some(completed_at) = parse_optional_datetime(row.get("completed_at"))? else {
            continue;
        };
        let latency_ms = (completed_at - created_at).num_milliseconds().max(0);
        grouped
            .entry(label.to_string())
            .or_default()
            .push(latency_ms);
    }

    let mut summary = BTreeMap::new();
    for (command_type, values) in grouped {
        if let Some(p95) = p95_ms(values) {
            summary.insert(command_type, p95);
        }
    }

    if summary.is_empty() {
        return Ok(None);
    }

    Ok(Some(serde_json::to_string(&summary)?))
}

fn command_latency_label(command_type: &str) -> Option<&'static str> {
    match command_type {
        "ApproveStage" => Some("approve_stage"),
        "RetryStage" => Some("retry_stage"),
        "CancelRun" => Some("cancel_run"),
        _ => None,
    }
}

fn p95_ms(mut values: Vec<i64>) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let rank = ((values.len() * 95).div_ceil(100)).max(1);
    values.get(rank - 1).copied()
}

fn elapsed_millis_i64(started_at: Instant) -> i64 {
    i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn parse_optional_datetime(value: Option<String>) -> Result<Option<DateTime<Utc>>> {
    value.map(parse_datetime).transpose()
}

fn parse_datetime(value: String) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(&value)
        .context("parse scheduler datetime")?
        .with_timezone(&Utc))
}
