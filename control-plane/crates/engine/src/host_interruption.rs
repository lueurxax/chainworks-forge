use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use acp::AcpRuntimeManager;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use db::repos::{agent_executions, artifact_contracts, scheduler, work_items};
use domain::agent::AgentStatus;
use domain::ids::RunId;
use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout, Duration as TokioDuration, MissedTickBehavior};
use tracing::warn;

use crate::work_queue::WorkQueue;

const RETRY_BATCH_PER_PROVIDER: i64 = 2;
const RETRY_JITTER_MIN_SECONDS: i64 = 5;
const RETRY_JITTER_SPAN_SECONDS: i64 = 26;
const HEARTBEAT_INTERVAL_SECONDS: u64 = 10;
const RUNTIME_CLEANUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Clone)]
pub struct HostInterruptionService {
    pool: SqlitePool,
    work_queue: WorkQueue,
    capacity_config: InvokeAgentCapacityConfig,
    runtime_cleanup: Option<Arc<dyn HostInterruptionRuntimeCleanup>>,
}

pub struct HostInterruptionDetector {
    service: HostInterruptionService,
    config: HostInterruptionDetectorConfig,
    last_clock_snapshot: Option<HostInterruptionClockSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostInterruptionDetectorConfig {
    pub wall_clock_gap_threshold_ms: i64,
}

impl Default for HostInterruptionDetectorConfig {
    fn default() -> Self {
        Self {
            wall_clock_gap_threshold_ms: 60_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostInterruptionClockSnapshot {
    pub wall_clock: DateTime<Utc>,
    pub monotonic_elapsed_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct HostInterruptionEvent {
    pub kind: HostInterruptionKind,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub monotonic_gap_ms: Option<i64>,
    pub wall_clock_gap_ms: Option<i64>,
    pub details_json: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostInterruptionKind {
    SystemSleep,
    NetworkMigration,
    WallClockGap,
}

impl HostInterruptionKind {
    fn as_str(self) -> &'static str {
        match self {
            HostInterruptionKind::SystemSleep => "system_sleep",
            HostInterruptionKind::NetworkMigration => "network_migration",
            HostInterruptionKind::WallClockGap => "wall_clock_gap",
        }
    }

    fn operator_action(self) -> &'static str {
        match self {
            HostInterruptionKind::SystemSleep | HostInterruptionKind::WallClockGap => {
                "recovering_from_system_sleep"
            }
            HostInterruptionKind::NetworkMigration => "resuming_after_network_change",
        }
    }
}

#[async_trait]
pub trait HostInterruptionRuntimeCleanup: Send + Sync {
    async fn close_session_generation(&self, generation_id: &str) -> Result<()>;
}

#[async_trait]
impl HostInterruptionRuntimeCleanup for AcpRuntimeManager {
    async fn close_session_generation(&self, generation_id: &str) -> Result<()> {
        match self.close_session(generation_id).await {
            Ok(_) => Ok(()),
            Err(error)
                if error
                    .to_string()
                    .contains("No live ACP session registered for generation id") =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

impl HostInterruptionDetector {
    pub fn new(service: HostInterruptionService) -> Self {
        Self::with_config(service, HostInterruptionDetectorConfig::default())
    }

    pub fn with_config(
        service: HostInterruptionService,
        config: HostInterruptionDetectorConfig,
    ) -> Self {
        Self {
            service,
            config,
            last_clock_snapshot: None,
        }
    }

    pub async fn observe_clock_snapshot(
        &mut self,
        snapshot: HostInterruptionClockSnapshot,
    ) -> Result<Option<HostInterruptionRecoverySummary>> {
        let Some(previous) = self.last_clock_snapshot.replace(snapshot) else {
            return Ok(None);
        };

        let wall_clock_gap_ms = (snapshot.wall_clock - previous.wall_clock).num_milliseconds();
        if wall_clock_gap_ms <= 0 {
            return Ok(None);
        }

        let monotonic_gap_ms = match (previous.monotonic_elapsed_ms, snapshot.monotonic_elapsed_ms)
        {
            (Some(previous), Some(current)) => Some((current - previous).max(0)),
            _ => None,
        };
        let observed_gap_ms = monotonic_gap_ms
            .map(|gap| wall_clock_gap_ms.saturating_sub(gap))
            .unwrap_or(wall_clock_gap_ms);

        if observed_gap_ms < self.config.wall_clock_gap_threshold_ms {
            return Ok(None);
        }

        let details_json = serde_json::to_string(&serde_json::json!({
            "source": "runtime_heartbeat",
            "observed_gap_ms": observed_gap_ms,
            "wall_clock_gap_ms": wall_clock_gap_ms,
            "monotonic_gap_ms": monotonic_gap_ms,
        }))
        .context("serialize host interruption heartbeat details")?;

        self.service
            .record_and_requeue(HostInterruptionEvent {
                kind: HostInterruptionKind::WallClockGap,
                started_at: previous.wall_clock,
                ended_at: Some(snapshot.wall_clock),
                monotonic_gap_ms,
                wall_clock_gap_ms: Some(wall_clock_gap_ms),
                details_json: Some(details_json),
            })
            .await
            .map(Some)
    }

    pub async fn record_system_sleep_wake(
        &self,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    ) -> Result<HostInterruptionRecoverySummary> {
        self.service
            .record_and_requeue(HostInterruptionEvent {
                kind: HostInterruptionKind::SystemSleep,
                started_at,
                ended_at: Some(ended_at),
                monotonic_gap_ms: None,
                wall_clock_gap_ms: Some((ended_at - started_at).num_milliseconds().max(0)),
                details_json: Some(r#"{"source":"system_sleep_wake"}"#.into()),
            })
            .await
    }

    pub async fn record_network_migration(
        &self,
        observed_at: DateTime<Utc>,
    ) -> Result<HostInterruptionRecoverySummary> {
        self.service
            .record_and_requeue(HostInterruptionEvent {
                kind: HostInterruptionKind::NetworkMigration,
                started_at: observed_at,
                ended_at: Some(observed_at),
                monotonic_gap_ms: None,
                wall_clock_gap_ms: None,
                details_json: Some(r#"{"source":"network_path_change"}"#.into()),
            })
            .await
    }
}

pub fn spawn_runtime_heartbeat_monitor(service: HostInterruptionService) -> JoinHandle<()> {
    spawn_runtime_heartbeat_monitor_with_config(
        service,
        HostInterruptionDetectorConfig::default(),
        TokioDuration::from_secs(HEARTBEAT_INTERVAL_SECONDS),
    )
}

pub fn spawn_runtime_heartbeat_monitor_with_config(
    service: HostInterruptionService,
    config: HostInterruptionDetectorConfig,
    heartbeat_interval: TokioDuration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let monotonic_started = Instant::now();
        let mut detector = HostInterruptionDetector::with_config(service, config);
        let mut ticks = interval(heartbeat_interval);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticks.tick().await;
            let monotonic_elapsed_ms = monotonic_started.elapsed().as_millis() as i64;
            let snapshot = HostInterruptionClockSnapshot {
                wall_clock: Utc::now(),
                monotonic_elapsed_ms: Some(monotonic_elapsed_ms),
            };
            if let Err(error) = detector.observe_clock_snapshot(snapshot).await {
                warn!(
                    error = %error,
                    "host interruption heartbeat recovery failed"
                );
            }
        }
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInterruptionRecoverySummary {
    pub epoch_id: String,
    pub affected_executions: usize,
    pub cancelled_executions: usize,
    pub retries_enqueued: usize,
    pub retries_deferred_capacity: usize,
    pub retries_deferred_cleanup_failed: usize,
    pub retries_missing_work_item: usize,
    pub runtime_cleanup_attempted: usize,
    pub runtime_cleanup_succeeded: usize,
    pub runtime_cleanup_failed: usize,
}

impl HostInterruptionService {
    pub fn new(pool: SqlitePool, work_queue: WorkQueue) -> Self {
        Self::with_capacity_config(pool, work_queue, InvokeAgentCapacityConfig::default())
    }

    pub fn with_capacity_config(
        pool: SqlitePool,
        work_queue: WorkQueue,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            work_queue,
            capacity_config,
            runtime_cleanup: None,
        }
    }

    pub fn with_capacity_config_and_runtime_cleanup(
        pool: SqlitePool,
        work_queue: WorkQueue,
        capacity_config: InvokeAgentCapacityConfig,
        runtime_cleanup: Arc<dyn HostInterruptionRuntimeCleanup>,
    ) -> Self {
        Self {
            pool,
            work_queue,
            capacity_config,
            runtime_cleanup: Some(runtime_cleanup),
        }
    }

    pub async fn record_and_requeue(
        &self,
        event: HostInterruptionEvent,
    ) -> Result<HostInterruptionRecoverySummary> {
        let now = Utc::now();
        let cleanup_summary = self.cleanup_runtime_sessions_for_event(&event, now).await?;
        let transaction_started = Instant::now();
        let writer_wait_started_at = Instant::now();
        let mut tx = db::pool::begin_immediate_with_retry(
            &self.pool,
            "host_interruption.record_and_requeue",
        )
        .await?;
        let writer_wait_ms = writer_wait_started_at.elapsed().as_millis() as i64;
        let epoch = scheduler::HostInterruptionEpoch {
            id: uuid::Uuid::new_v4().to_string(),
            kind: event.kind.as_str().to_string(),
            started_at: event.started_at,
            ended_at: event.ended_at,
            monotonic_gap_ms: event.monotonic_gap_ms,
            wall_clock_gap_ms: event.wall_clock_gap_ms,
            details_json: event.details_json,
            created_at: now,
        };
        scheduler::insert_host_interruption_epoch_tx(&mut tx, &epoch).await?;

        let affected_end = event.ended_at.unwrap_or(now);
        let affected = agent_executions::list_running_across_interval_tx(
            &mut tx,
            event.started_at,
            affected_end,
        )
        .await?;

        for execution in &affected {
            agent_executions::update_completed_tx(
                &mut tx,
                execution.id,
                AgentStatus::Cancelled,
                now,
            )
            .await?;
        }

        let mut slots = ActiveSlotCounts::load_tx(&mut tx).await?;
        let mut provider_batches: BTreeMap<String, i64> = BTreeMap::new();
        let mut summary = HostInterruptionRecoverySummary {
            epoch_id: epoch.id.clone(),
            affected_executions: affected.len(),
            cancelled_executions: affected.len(),
            retries_enqueued: 0,
            retries_deferred_capacity: 0,
            retries_deferred_cleanup_failed: 0,
            retries_missing_work_item: 0,
            runtime_cleanup_attempted: cleanup_summary.attempted,
            runtime_cleanup_succeeded: cleanup_summary.succeeded,
            runtime_cleanup_failed: cleanup_summary.failed,
        };

        for execution in affected {
            let action = event.kind.operator_action().to_string();
            let mut retry_enqueued_at = None;
            let cleanup_status = cleanup_summary
                .execution_status
                .get(&execution.id.to_string())
                .cloned()
                .unwrap_or_else(|| "not_required".to_string());
            let mut settlement_status = "retry_enqueued".to_string();
            let provider_family = execution.provider_family.clone();

            if cleanup_status == "failed" {
                summary.retries_deferred_cleanup_failed += 1;
                settlement_status = "retry_deferred_cleanup_failed".to_string();
            } else if let Some(provider_family) = provider_family.as_deref() {
                let family = ProviderFamily::resolve(provider_family)
                    .context("resolve affected execution provider family")?;
                let provider_batch = provider_batches
                    .entry(provider_family.to_string())
                    .or_insert(0);

                if *provider_batch >= RETRY_BATCH_PER_PROVIDER
                    || !slots.try_reserve(execution.run_id, family, &self.capacity_config)
                {
                    let scheduled_at = delayed_retry_at(now, summary.retries_deferred_capacity);
                    if requeue_retry_for_execution_tx(&mut tx, &execution, scheduled_at, &epoch.id)
                        .await?
                        .is_none()
                    {
                        summary.retries_missing_work_item += 1;
                        settlement_status = "retry_missing_work_item".to_string();
                    } else {
                        summary.retries_deferred_capacity += 1;
                        settlement_status = "retry_deferred_capacity".to_string();
                        retry_enqueued_at = Some(scheduled_at);
                    }
                } else {
                    let scheduled_at = now
                        + Duration::seconds(
                            RETRY_JITTER_MIN_SECONDS
                                + (summary.retries_enqueued as i64 % RETRY_JITTER_SPAN_SECONDS),
                        );
                    if requeue_retry_for_execution_tx(&mut tx, &execution, scheduled_at, &epoch.id)
                        .await?
                        .is_none()
                    {
                        slots.release(execution.run_id, family);
                        summary.retries_missing_work_item += 1;
                        settlement_status = "retry_missing_work_item".to_string();
                    } else {
                        *provider_batch += 1;
                        summary.retries_enqueued += 1;
                        retry_enqueued_at = Some(scheduled_at);
                    }
                }
            } else {
                let scheduled_at = delayed_retry_at(now, summary.retries_deferred_capacity);
                if requeue_retry_for_execution_tx(&mut tx, &execution, scheduled_at, &epoch.id)
                    .await?
                    .is_none()
                {
                    summary.retries_missing_work_item += 1;
                    settlement_status = "retry_missing_work_item".to_string();
                } else {
                    summary.retries_deferred_capacity += 1;
                    settlement_status = "retry_deferred_capacity".to_string();
                    retry_enqueued_at = Some(scheduled_at);
                }
            }

            scheduler::insert_host_interruption_affected_execution_tx(
                &mut tx,
                &scheduler::HostInterruptionAffectedExecution {
                    epoch_id: epoch.id.clone(),
                    agent_execution_id: execution.id.to_string(),
                    run_id: Some(execution.run_id.to_string()),
                    stage_execution_id: execution.stage_execution_id.to_string(),
                    provider_family,
                    action,
                    previous_status: "running".to_string(),
                    settlement_status,
                    cleanup_status,
                    quota_budget_effect: "not_consumed".to_string(),
                    retry_enqueued_at,
                    created_at: now,
                },
            )
            .await?;
        }

        let refresh = scheduler::refresh_queue_summaries_for_notification_tx(
            &mut tx,
            &self.capacity_config,
            now,
            "host_interruption.record_and_requeue",
            writer_wait_ms,
        )
        .await?;
        tx.commit()
            .await
            .context("commit host interruption recovery")?;
        db::pool::log_write_transaction(
            "host_interruption.record_and_requeue",
            transaction_started,
        );
        self.work_queue.publish_scheduler_notification(refresh);
        Ok(summary)
    }

    async fn cleanup_runtime_sessions_for_event(
        &self,
        event: &HostInterruptionEvent,
        now: DateTime<Utc>,
    ) -> Result<RuntimeCleanupSummary> {
        let Some(runtime_cleanup) = &self.runtime_cleanup else {
            return Ok(RuntimeCleanupSummary::default());
        };

        let affected_end = event.ended_at.unwrap_or(now);
        let affected = agent_executions::list_running_across_interval(
            &self.pool,
            event.started_at,
            affected_end,
        )
        .await?;

        let mut summary = RuntimeCleanupSummary::default();
        for execution in affected {
            let Some(generation_id) = execution.session_generation_id.as_deref() else {
                summary
                    .execution_status
                    .insert(execution.id.to_string(), "not_required".to_string());
                continue;
            };
            summary.attempted += 1;
            let execution_id = execution.id.to_string();
            match timeout(
                RUNTIME_CLEANUP_TIMEOUT,
                runtime_cleanup.close_session_generation(generation_id),
            )
            .await
            {
                Ok(Ok(())) => {
                    summary.succeeded += 1;
                    summary.execution_status.insert(execution_id, "succeeded".to_string());
                }
                Ok(Err(error)) => {
                    warn!(
                        error = %error,
                        generation_id,
                        execution_id = %execution.id,
                        "host interruption runtime cleanup failed before retry enqueue"
                    );
                    summary.failed += 1;
                    summary.execution_status.insert(execution_id, "failed".to_string());
                }
                Err(_) => {
                    warn!(
                        generation_id,
                        execution_id = %execution.id,
                        timeout_ms = RUNTIME_CLEANUP_TIMEOUT.as_millis() as i64,
                        "host interruption runtime cleanup timed out before retry enqueue"
                    );
                    summary.failed += 1;
                    summary.execution_status.insert(execution_id, "failed".to_string());
                }
            }
        }

        Ok(summary)
    }
}

#[derive(Default)]
struct RuntimeCleanupSummary {
    attempted: usize,
    succeeded: usize,
    failed: usize,
    execution_status: BTreeMap<String, String>,
}

async fn requeue_retry_for_execution_tx(
    tx: &mut Transaction<'_, Sqlite>,
    execution: &agent_executions::RunningAgentExecution,
    scheduled_at: DateTime<Utc>,
    epoch_id: &str,
) -> Result<Option<String>> {
    let requeued_work_item_ids =
        work_items::requeue_running_invoke_agent_by_stage_for_host_interruption_tx(
            tx,
            execution.run_id,
            &execution.stage_id,
            execution.stage_execution_id,
            scheduled_at,
        )
        .await?;
    let superseding_work_item_id = requeued_work_item_ids.first().cloned();
    if let Some(work_item_id) = superseding_work_item_id.as_deref() {
        artifact_contracts::mark_active_claims_superseded_pending_retry_for_stage_tx(
            tx,
            execution.run_id,
            &execution.stage_execution_id.to_string(),
            work_item_id,
            epoch_id,
        )
        .await?;
    }
    Ok(superseding_work_item_id)
}

fn delayed_retry_at(now: DateTime<Utc>, deferred_count: usize) -> DateTime<Utc> {
    now + Duration::seconds(30 + (deferred_count as i64 % RETRY_JITTER_SPAN_SECONDS))
}

#[derive(Default)]
struct ActiveSlotCounts {
    global: i64,
    by_run: BTreeMap<String, i64>,
    by_provider: BTreeMap<ProviderFamily, i64>,
}

impl ActiveSlotCounts {
    async fn load_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<Self> {
        let mut counts = Self::default();
        counts.global =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(&mut **tx)
                .await
                .context("count running executions for host interruption retry slots")?;

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running executions by run for host interruption retry slots")?;
        for row in run_rows {
            counts.by_run.insert(row.get("run_id"), row.get("count"));
        }

        let provider_rows = sqlx::query(
            r#"SELECT COALESCE(provider_family, provider) AS provider_family, COUNT(*) AS count
               FROM agent_executions
               WHERE status = 'running'
               GROUP BY COALESCE(provider_family, provider)"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running executions by provider for host interruption retry slots")?;
        for row in provider_rows {
            let provider_family: String = row.get("provider_family");
            if let Some(canonical) = ProviderFamily::canonicalize_known_alias(&provider_family) {
                counts
                    .by_provider
                    .insert(ProviderFamily::resolve(canonical)?, row.get("count"));
            }
        }

        Ok(counts)
    }

    fn try_reserve(
        &mut self,
        run_id: RunId,
        family: ProviderFamily,
        capacity: &InvokeAgentCapacityConfig,
    ) -> bool {
        let run_key = run_id.to_string();
        let run_count = self.by_run.get(&run_key).copied().unwrap_or(0);
        let provider_count = self.by_provider.get(&family).copied().unwrap_or(0);
        if self.global >= capacity.global_active_agent_executions as i64
            || run_count >= capacity.per_run_active_agent_executions as i64
            || provider_count >= capacity.provider_cap(family) as i64
        {
            return false;
        }

        self.global += 1;
        *self.by_run.entry(run_key).or_insert(0) += 1;
        *self.by_provider.entry(family).or_insert(0) += 1;
        true
    }

    fn release(&mut self, run_id: RunId, family: ProviderFamily) {
        self.global = (self.global - 1).max(0);
        let run_key = run_id.to_string();
        if let Some(count) = self.by_run.get_mut(&run_key) {
            *count = (*count - 1).max(0);
        }
        if let Some(count) = self.by_provider.get_mut(&family) {
            *count = (*count - 1).max(0);
        }
    }
}
