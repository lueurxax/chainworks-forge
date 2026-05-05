use async_graphql::*;
use chrono::{DateTime, Utc};

use db::repos::scheduler::{
    HostInterruptionAffectedExecution, HostInterruptionEpochReadback,
    SchedulerBackpressureNotification, SchedulerHealthSnapshot, SchedulerProviderActiveCount,
    SchedulerQueuePositionHint, SchedulerQueueSummary,
};
use db::repos::startup_repairs::{StartupRecoveryReadback, ToolchainCacheRecoveryReadback};
use db::repos::toolchain_cache_housekeeping::ToolchainCacheHousekeepingReadback;

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlSchedulerQueueSummary {
    pub scope: String,
    pub scope_id: String,
    pub run_id: Option<ID>,
    pub stage_execution_id: Option<ID>,
    pub provider_family: Option<String>,
    pub top_reason: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
}

impl From<SchedulerQueueSummary> for GqlSchedulerQueueSummary {
    fn from(summary: SchedulerQueueSummary) -> Self {
        let now = Utc::now();
        let is_stale = summary.is_stale_at(now);
        Self {
            scope: summary.scope,
            scope_id: summary.scope_id,
            run_id: summary.run_id.map(ID),
            stage_execution_id: summary.stage_execution_id.map(ID),
            provider_family: summary.provider_family,
            top_reason: summary.top_reason,
            queued_count: summary.queued_count,
            oldest_queued_age_ms: summary.oldest_queued_age_ms,
            global_queue_depth: summary.global_queue_depth,
            stale_after_ms: summary.stale_after_ms,
            updated_at: summary.updated_at.to_rfc3339(),
            is_stale,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlSchedulerHealthSummary {
    pub id: ID,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub active_agent_executions: i64,
    pub db_writer_wait_p95_ms: Option<i64>,
    pub command_latency_p95_ms_json: Option<String>,
    pub last_host_interruption_epoch_id: Option<ID>,
    pub sustained_backpressure_state: String,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlCommandLatencySummary {
    pub command_latency_p95_ms_json: Option<String>,
    pub updated_at: String,
    pub stale_after_ms: i64,
    pub is_stale: bool,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlDbWriterContentionSummary {
    pub db_writer_wait_p95_ms: Option<i64>,
    pub updated_at: String,
    pub stale_after_ms: i64,
    pub is_stale: bool,
}

impl From<SchedulerHealthSnapshot> for GqlSchedulerHealthSummary {
    fn from(snapshot: SchedulerHealthSnapshot) -> Self {
        let now = Utc::now();
        let is_stale = snapshot.is_stale_at(now);
        Self {
            id: ID(snapshot.id),
            queued_count: snapshot.queued_count,
            oldest_queued_age_ms: snapshot.oldest_queued_age_ms,
            global_queue_depth: snapshot.global_queue_depth,
            active_agent_executions: snapshot.active_agent_executions,
            db_writer_wait_p95_ms: snapshot.db_writer_wait_p95_ms,
            command_latency_p95_ms_json: snapshot.command_latency_p95_ms_json,
            last_host_interruption_epoch_id: snapshot.last_host_interruption_epoch_id.map(ID),
            sustained_backpressure_state: snapshot.sustained_backpressure_state,
            stale_after_ms: snapshot.stale_after_ms,
            updated_at: snapshot.updated_at.to_rfc3339(),
            is_stale,
        }
    }
}

/// P066 T17: Toolchain cache subsection of startupRecoverySummary.
/// Fields are None when no sweep has run yet (pre-P066 rows).
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ToolchainCacheRecoverySummary", rename_fields = "camelCase")]
pub struct GqlToolchainCacheRecoverySummary {
    pub session_scoped_roots_seen: Option<i64>,
    pub session_scoped_roots_reclaimed: Option<i64>,
    pub session_scoped_cleanup_failures: Option<i64>,
    pub orphan_threshold_minutes: Option<i64>,
    pub last_sweep_started_at: Option<String>,
}

impl From<ToolchainCacheRecoveryReadback> for GqlToolchainCacheRecoverySummary {
    fn from(r: ToolchainCacheRecoveryReadback) -> Self {
        Self {
            session_scoped_roots_seen: r.session_scoped_roots_seen,
            session_scoped_roots_reclaimed: r.session_scoped_roots_reclaimed,
            session_scoped_cleanup_failures: r.session_scoped_cleanup_failures,
            orphan_threshold_minutes: r.orphan_threshold_minutes,
            last_sweep_started_at: r.last_sweep_started_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlStartupRecoverySummary {
    pub id: ID,
    pub recovered_item_count: i64,
    pub queued_under_startup_recovery_backpressure_count: i64,
    pub oldest_recovered_queued_age_ms: Option<i64>,
    pub affected_run_count: i64,
    pub next_retry_or_backoff_time: Option<String>,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
    /// P066 T17: Toolchain cache cleanup evidence from startup recovery sweeps.
    pub toolchain_cache: GqlToolchainCacheRecoverySummary,
}

impl From<StartupRecoveryReadback> for GqlStartupRecoverySummary {
    fn from(summary: StartupRecoveryReadback) -> Self {
        let now = Utc::now();
        let is_stale = summary.is_stale_at(now);
        Self {
            id: ID(summary.id),
            recovered_item_count: summary.recovered_item_count,
            queued_under_startup_recovery_backpressure_count: summary
                .queued_under_startup_recovery_backpressure_count,
            oldest_recovered_queued_age_ms: summary.oldest_recovered_queued_age_ms,
            affected_run_count: summary.affected_run_count,
            next_retry_or_backoff_time: summary
                .next_retry_or_backoff_time
                .map(|value| value.to_rfc3339()),
            stale_after_ms: summary.stale_after_ms,
            updated_at: summary.updated_at.to_rfc3339(),
            is_stale,
            toolchain_cache: GqlToolchainCacheRecoverySummary::from(summary.toolchain_cache),
        }
    }
}

/// P066 T18: Durable housekeeping summary for periodic run-root pruning.
/// One row per sweep. Used as promotion gate evidence for Phase 3 catalog backfill.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(
    name = "ToolchainCacheHousekeepingSummary",
    rename_fields = "camelCase"
)]
pub struct GqlToolchainCacheHousekeepingSummary {
    pub id: ID,
    pub last_sweep_started_at: String,
    pub run_scoped_roots_pruned: i64,
    pub run_scoped_prune_failures: i64,
    pub oldest_eligible_root_age_days: Option<f64>,
    pub disk_pressure_blocks: i64,
    pub quarantined_roots_created: i64,
    pub created_at: String,
}

impl From<ToolchainCacheHousekeepingReadback> for GqlToolchainCacheHousekeepingSummary {
    fn from(r: ToolchainCacheHousekeepingReadback) -> Self {
        Self {
            id: ID(r.id),
            last_sweep_started_at: r.last_sweep_started_at.to_rfc3339(),
            run_scoped_roots_pruned: r.run_scoped_roots_pruned,
            run_scoped_prune_failures: r.run_scoped_prune_failures,
            oldest_eligible_root_age_days: r.oldest_eligible_root_age_days,
            disk_pressure_blocks: r.disk_pressure_blocks,
            quarantined_roots_created: r.quarantined_roots_created,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlSchedulerProviderActiveCount {
    pub provider_family: String,
    pub active_count: i64,
}

impl From<SchedulerProviderActiveCount> for GqlSchedulerProviderActiveCount {
    fn from(count: SchedulerProviderActiveCount) -> Self {
        Self {
            provider_family: count.provider_family,
            active_count: count.active_count,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlSchedulerQueuePositionHint {
    pub scope: String,
    pub scope_id: String,
    pub run_id: Option<ID>,
    pub stage_execution_id: Option<ID>,
    pub queue_position: i64,
    pub global_queue_depth: i64,
    pub queued_ahead_count: i64,
    pub scoped_queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub updated_at: String,
}

impl From<SchedulerQueuePositionHint> for GqlSchedulerQueuePositionHint {
    fn from(hint: SchedulerQueuePositionHint) -> Self {
        Self {
            scope: hint.scope,
            scope_id: hint.scope_id,
            run_id: hint.run_id.map(ID),
            stage_execution_id: hint.stage_execution_id.map(ID),
            queue_position: hint.queue_position,
            global_queue_depth: hint.global_queue_depth,
            queued_ahead_count: hint.queued_ahead_count,
            scoped_queued_count: hint.scoped_queued_count,
            oldest_queued_age_ms: hint.oldest_queued_age_ms,
            updated_at: hint.updated_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlHostInterruptionAffectedExecution {
    pub epoch_id: ID,
    pub agent_execution_id: ID,
    pub run_id: Option<ID>,
    pub stage_execution_id: ID,
    pub provider_family: Option<String>,
    pub action: String,
    pub previous_status: String,
    pub settlement_status: String,
    pub cleanup_status: String,
    pub quota_budget_effect: String,
    pub retry_enqueued_at: Option<String>,
    pub created_at: String,
}

impl From<HostInterruptionAffectedExecution> for GqlHostInterruptionAffectedExecution {
    fn from(affected: HostInterruptionAffectedExecution) -> Self {
        Self {
            epoch_id: ID(affected.epoch_id),
            agent_execution_id: ID(affected.agent_execution_id),
            run_id: affected.run_id.map(ID),
            stage_execution_id: ID(affected.stage_execution_id),
            provider_family: affected.provider_family,
            action: affected.action,
            previous_status: affected.previous_status,
            settlement_status: affected.settlement_status,
            cleanup_status: affected.cleanup_status,
            quota_budget_effect: affected.quota_budget_effect,
            retry_enqueued_at: affected.retry_enqueued_at.map(|value| value.to_rfc3339()),
            created_at: affected.created_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlHostInterruptionEpoch {
    pub id: ID,
    pub kind: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub monotonic_gap_ms: Option<i64>,
    pub wall_clock_gap_ms: Option<i64>,
    pub details_json: Option<String>,
    pub created_at: String,
    pub affected_executions: Vec<GqlHostInterruptionAffectedExecution>,
}

impl From<HostInterruptionEpochReadback> for GqlHostInterruptionEpoch {
    fn from(readback: HostInterruptionEpochReadback) -> Self {
        Self {
            id: ID(readback.epoch.id),
            kind: readback.epoch.kind,
            started_at: readback.epoch.started_at.to_rfc3339(),
            ended_at: readback.epoch.ended_at.map(|value| value.to_rfc3339()),
            monotonic_gap_ms: readback.epoch.monotonic_gap_ms,
            wall_clock_gap_ms: readback.epoch.wall_clock_gap_ms,
            details_json: readback.epoch.details_json,
            created_at: readback.epoch.created_at.to_rfc3339(),
            affected_executions: readback
                .affected_executions
                .into_iter()
                .map(GqlHostInterruptionAffectedExecution::from)
                .collect(),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlSchedulerBackpressureNotification {
    pub run_id: Option<ID>,
    pub stage_execution_id: Option<ID>,
    pub provider_family: Option<String>,
    pub top_reason: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub state: String,
    pub updated_at: String,
    pub stale_after_ms: i64,
    pub is_stale: bool,
}

impl From<SchedulerBackpressureNotification> for GqlSchedulerBackpressureNotification {
    fn from(notification: SchedulerBackpressureNotification) -> Self {
        let now = Utc::now();
        let is_stale = notification.is_stale_at(now);
        Self {
            run_id: notification.run_id.map(ID),
            stage_execution_id: notification.stage_execution_id.map(ID),
            provider_family: notification.provider_family,
            top_reason: notification.top_reason,
            queued_count: notification.queued_count,
            oldest_queued_age_ms: notification.oldest_queued_age_ms,
            global_queue_depth: notification.global_queue_depth,
            state: notification.state,
            updated_at: notification.updated_at.to_rfc3339(),
            stale_after_ms: notification.stale_after_ms,
            is_stale,
        }
    }
}

impl GqlSchedulerBackpressureNotification {
    pub fn from_event(
        run_id: Option<String>,
        stage_execution_id: Option<String>,
        provider_family: Option<String>,
        top_reason: String,
        queued_count: i64,
        oldest_queued_age_ms: i64,
        global_queue_depth: i64,
        state: String,
        updated_at: DateTime<Utc>,
        stale_after_ms: i64,
    ) -> Self {
        let notification = SchedulerBackpressureNotification {
            run_id,
            stage_execution_id,
            provider_family,
            top_reason,
            queued_count,
            oldest_queued_age_ms,
            global_queue_depth,
            state,
            updated_at,
            stale_after_ms,
        };
        notification.into()
    }
}
