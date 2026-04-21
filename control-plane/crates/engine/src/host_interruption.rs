use std::collections::BTreeMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use db::repos::{agent_executions, scheduler, work_items};
use domain::agent::{AgentStatus, OperatorActionHint};
use domain::ids::RunId;
use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};
use sqlx::{Row, SqlitePool};

use crate::work_queue::WorkQueue;

const RETRY_BATCH_PER_PROVIDER: i64 = 2;
const RETRY_JITTER_MIN_SECONDS: i64 = 5;
const RETRY_JITTER_SPAN_SECONDS: i64 = 26;

pub struct HostInterruptionService {
    pool: SqlitePool,
    work_queue: WorkQueue,
    capacity_config: InvokeAgentCapacityConfig,
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
                OperatorActionHint::RecoveringFromSystemSleep.to_string_static()
            }
            HostInterruptionKind::NetworkMigration => {
                OperatorActionHint::ResumingAfterNetworkChange.to_string_static()
            }
        }
    }
}

trait StaticOperatorActionHint {
    fn to_string_static(&self) -> &'static str;
}

impl StaticOperatorActionHint for OperatorActionHint {
    fn to_string_static(&self) -> &'static str {
        match self {
            OperatorActionHint::RecoveringFromSystemSleep => "recovering_from_system_sleep",
            OperatorActionHint::ResumingAfterNetworkChange => "resuming_after_network_change",
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInterruptionRecoverySummary {
    pub epoch_id: String,
    pub affected_executions: usize,
    pub cancelled_executions: usize,
    pub retries_enqueued: usize,
    pub retries_deferred_capacity: usize,
    pub retries_missing_work_item: usize,
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
        }
    }

    pub async fn record_and_requeue(
        &self,
        event: HostInterruptionEvent,
    ) -> Result<HostInterruptionRecoverySummary> {
        let now = Utc::now();
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
        scheduler::insert_host_interruption_epoch(&self.pool, &epoch).await?;

        let affected_end = event.ended_at.unwrap_or(now);
        let affected = agent_executions::list_running_across_interval(
            &self.pool,
            event.started_at,
            affected_end,
        )
        .await?;

        for execution in &affected {
            agent_executions::update_completed(
                &self.pool,
                execution.id,
                AgentStatus::Cancelled,
                now,
            )
            .await?;
        }

        let mut slots = ActiveSlotCounts::load(&self.pool).await?;
        let mut provider_batches: BTreeMap<String, i64> = BTreeMap::new();
        let mut summary = HostInterruptionRecoverySummary {
            epoch_id: epoch.id.clone(),
            affected_executions: affected.len(),
            cancelled_executions: affected.len(),
            retries_enqueued: 0,
            retries_deferred_capacity: 0,
            retries_missing_work_item: 0,
        };

        for execution in affected {
            let action = event.kind.operator_action().to_string();
            let mut retry_enqueued_at = None;
            let provider_family = execution.provider_family.clone();

            if let Some(provider_family) = provider_family.as_deref() {
                let family = ProviderFamily::resolve(provider_family)
                    .context("resolve affected execution provider family")?;
                let provider_batch = provider_batches
                    .entry(provider_family.to_string())
                    .or_insert(0);

                if *provider_batch >= RETRY_BATCH_PER_PROVIDER
                    || !slots.try_reserve(execution.run_id, family, &self.capacity_config)
                {
                    let scheduled_at = delayed_retry_at(now, summary.retries_deferred_capacity);
                    let requeued =
                        work_items::requeue_running_invoke_agent_by_stage_for_host_interruption(
                            &self.pool,
                            execution.run_id,
                            &execution.stage_id,
                            execution.stage_execution_id,
                            scheduled_at,
                        )
                        .await?;
                    if requeued == 0 {
                        summary.retries_missing_work_item += 1;
                    } else {
                        summary.retries_deferred_capacity += 1;
                        retry_enqueued_at = Some(scheduled_at);
                    }
                } else {
                    let scheduled_at = now
                        + Duration::seconds(
                            RETRY_JITTER_MIN_SECONDS
                                + (summary.retries_enqueued as i64 % RETRY_JITTER_SPAN_SECONDS),
                        );
                    let requeued =
                        work_items::requeue_running_invoke_agent_by_stage_for_host_interruption(
                            &self.pool,
                            execution.run_id,
                            &execution.stage_id,
                            execution.stage_execution_id,
                            scheduled_at,
                        )
                        .await?;
                    if requeued == 0 {
                        slots.release(execution.run_id, family);
                        summary.retries_missing_work_item += 1;
                    } else {
                        *provider_batch += 1;
                        summary.retries_enqueued += 1;
                        retry_enqueued_at = Some(scheduled_at);
                    }
                }
            } else {
                let scheduled_at = delayed_retry_at(now, summary.retries_deferred_capacity);
                let requeued =
                    work_items::requeue_running_invoke_agent_by_stage_for_host_interruption(
                        &self.pool,
                        execution.run_id,
                        &execution.stage_id,
                        execution.stage_execution_id,
                        scheduled_at,
                    )
                    .await?;
                if requeued == 0 {
                    summary.retries_missing_work_item += 1;
                } else {
                    summary.retries_deferred_capacity += 1;
                    retry_enqueued_at = Some(scheduled_at);
                }
            }

            scheduler::insert_host_interruption_affected_execution(
                &self.pool,
                &scheduler::HostInterruptionAffectedExecution {
                    epoch_id: epoch.id.clone(),
                    agent_execution_id: execution.id.to_string(),
                    run_id: Some(execution.run_id.to_string()),
                    stage_execution_id: execution.stage_execution_id.to_string(),
                    provider_family,
                    action,
                    retry_enqueued_at,
                    created_at: now,
                },
            )
            .await?;
        }

        self.work_queue.refresh_scheduler_projection().await?;
        Ok(summary)
    }
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
    async fn load(pool: &SqlitePool) -> Result<Self> {
        let mut counts = Self::default();
        counts.global =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(pool)
                .await
                .context("count running executions for host interruption retry slots")?;

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(pool)
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
        .fetch_all(pool)
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
