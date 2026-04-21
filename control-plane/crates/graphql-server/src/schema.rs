use std::sync::Arc;

use async_graphql::futures_util::StreamExt;
use async_graphql::*;
use sqlx::SqlitePool;
use tokio_stream::wrappers::BroadcastStream;

use db::repos::{approvals, ideas, projections, runs, scheduler, steward as steward_repo};
use domain::commands::{
    ApproveStageCmd, CallerContext, CancelRunCmd, Command, RejectStageCmd, RetryStageCmd,
    StartRunCmd,
};
use domain::events::DomainEvent;
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;

use crate::types::approval::GqlApproval;
use crate::types::artifact::GqlArtifact;
use crate::types::idea::GqlIdea;
use crate::types::run::GqlRun;
use crate::types::stage::{GqlAgentExecution, GqlStageExecution};
use crate::types::steward::{
    GqlStewardAnalysis, GqlStewardAnalysisRunLink, GqlStewardRecommendation,
};

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .data(principal_table)
        .finish()
}

pub struct QueryRoot;

fn require_scheduler_read(ctx: &Context<'_>) -> Result<()> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized: no principal in context"))?;
    if auth::filter_tools(principal, &[domain::CapabilityToolId::ReportsGet]).len() == 1 {
        Ok(())
    } else {
        Err(Error::new("forbidden"))
    }
}

#[derive(SimpleObject)]
pub struct GqlSchedulerHealthSummary {
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub active_agent_executions: i64,
    pub db_writer_wait_p95_ms: Option<i64>,
    pub command_latency_p95_ms_json: Option<String>,
    pub last_host_interruption_epoch_id: Option<String>,
    pub sustained_backpressure_state: String,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject)]
pub struct GqlDbWriterContentionSummary {
    pub db_writer_wait_p95_ms: Option<i64>,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject)]
pub struct GqlCommandLatencySummary {
    pub command_latency_p95_ms_json: Option<String>,
    pub stale_after_ms: i64,
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject)]
pub struct GqlSchedulerQueueSummary {
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
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject)]
pub struct GqlSchedulerProviderActiveCount {
    pub provider_family: String,
    pub active_count: i64,
}

#[derive(SimpleObject)]
pub struct GqlSchedulerQueuePositionHint {
    pub scope: String,
    pub scope_id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub queue_position: i64,
    pub global_queue_depth: i64,
    pub queued_ahead_count: i64,
    pub scoped_queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlSchedulerBackpressureEvent {
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub provider_family: Option<String>,
    pub top_reason: String,
    pub queued_count: i64,
    pub oldest_queued_age_ms: i64,
    pub global_queue_depth: i64,
    pub state: String,
    pub updated_at: String,
    pub is_stale: bool,
}

#[derive(SimpleObject)]
pub struct GqlHostInterruptionAffectedExecution {
    pub epoch_id: String,
    pub agent_execution_id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: String,
    pub provider_family: Option<String>,
    pub action: String,
    pub retry_enqueued_at: Option<String>,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct GqlHostInterruptionEpoch {
    pub id: String,
    pub kind: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub monotonic_gap_ms: Option<i64>,
    pub wall_clock_gap_ms: Option<i64>,
    pub details_json: Option<String>,
    pub created_at: String,
    pub affected_executions: Vec<GqlHostInterruptionAffectedExecution>,
}

impl From<scheduler::SchedulerProviderActiveCount> for GqlSchedulerProviderActiveCount {
    fn from(count: scheduler::SchedulerProviderActiveCount) -> Self {
        Self {
            provider_family: count.provider_family,
            active_count: count.active_count,
        }
    }
}

fn gql_scheduler_health(snapshot: scheduler::SchedulerHealthSnapshot) -> GqlSchedulerHealthSummary {
    let now = chrono::Utc::now();
    let is_stale = snapshot.is_stale_at(now);
    GqlSchedulerHealthSummary {
        queued_count: snapshot.queued_count,
        oldest_queued_age_ms: snapshot.oldest_queued_age_ms,
        global_queue_depth: snapshot.global_queue_depth,
        active_agent_executions: snapshot.active_agent_executions,
        db_writer_wait_p95_ms: snapshot.db_writer_wait_p95_ms,
        command_latency_p95_ms_json: snapshot.command_latency_p95_ms_json,
        last_host_interruption_epoch_id: snapshot.last_host_interruption_epoch_id,
        sustained_backpressure_state: snapshot.sustained_backpressure_state,
        stale_after_ms: snapshot.stale_after_ms,
        updated_at: snapshot.updated_at.to_rfc3339(),
        is_stale,
    }
}

fn gql_db_writer_contention(
    snapshot: scheduler::SchedulerHealthSnapshot,
) -> GqlDbWriterContentionSummary {
    let now = chrono::Utc::now();
    let is_stale = snapshot.is_stale_at(now);
    GqlDbWriterContentionSummary {
        db_writer_wait_p95_ms: snapshot.db_writer_wait_p95_ms,
        stale_after_ms: snapshot.stale_after_ms,
        updated_at: snapshot.updated_at.to_rfc3339(),
        is_stale,
    }
}

fn gql_command_latency(snapshot: scheduler::SchedulerHealthSnapshot) -> GqlCommandLatencySummary {
    let now = chrono::Utc::now();
    let is_stale = snapshot.is_stale_at(now);
    GqlCommandLatencySummary {
        command_latency_p95_ms_json: snapshot.command_latency_p95_ms_json,
        stale_after_ms: snapshot.stale_after_ms,
        updated_at: snapshot.updated_at.to_rfc3339(),
        is_stale,
    }
}

fn gql_scheduler_queue_summary(
    summary: scheduler::SchedulerQueueSummary,
) -> GqlSchedulerQueueSummary {
    let now = chrono::Utc::now();
    let is_stale = summary.is_stale_at(now);
    GqlSchedulerQueueSummary {
        scope: summary.scope,
        scope_id: summary.scope_id,
        run_id: summary.run_id,
        stage_execution_id: summary.stage_execution_id,
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

fn gql_scheduler_queue_position_hint(
    hint: scheduler::SchedulerQueuePositionHint,
) -> GqlSchedulerQueuePositionHint {
    GqlSchedulerQueuePositionHint {
        scope: hint.scope,
        scope_id: hint.scope_id,
        run_id: hint.run_id,
        stage_execution_id: hint.stage_execution_id,
        queue_position: hint.queue_position,
        global_queue_depth: hint.global_queue_depth,
        queued_ahead_count: hint.queued_ahead_count,
        scoped_queued_count: hint.scoped_queued_count,
        oldest_queued_age_ms: hint.oldest_queued_age_ms,
        updated_at: hint.updated_at.to_rfc3339(),
    }
}

fn gql_host_interruption_epoch(
    readback: scheduler::HostInterruptionEpochReadback,
) -> GqlHostInterruptionEpoch {
    GqlHostInterruptionEpoch {
        id: readback.epoch.id,
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
            .map(|affected| GqlHostInterruptionAffectedExecution {
                epoch_id: affected.epoch_id,
                agent_execution_id: affected.agent_execution_id,
                run_id: affected.run_id,
                stage_execution_id: affected.stage_execution_id,
                provider_family: affected.provider_family,
                action: affected.action,
                retry_enqueued_at: affected.retry_enqueued_at.map(|value| value.to_rfc3339()),
                created_at: affected.created_at.to_rfc3339(),
            })
            .collect(),
    }
}

#[Object]
impl QueryRoot {
    async fn ideas(
        &self,
        ctx: &Context<'_>,
        include_archived: Option<bool>,
    ) -> Result<Vec<GqlIdea>> {
        let pool = ctx.data::<SqlitePool>()?;
        let include = include_archived.unwrap_or(false);
        let items = ideas::list(pool, include).await?;
        Ok(items.into_iter().map(GqlIdea::from).collect())
    }

    async fn idea(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlIdea>> {
        let pool = ctx.data::<SqlitePool>()?;
        let idea_id: IdeaId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = ideas::find_by_id(pool, idea_id).await?;
        Ok(item.map(GqlIdea::from))
    }

    async fn runs(&self, ctx: &Context<'_>, idea_id: Option<ID>) -> Result<Vec<GqlRun>> {
        let pool = ctx.data::<SqlitePool>()?;
        if let Some(id) = idea_id {
            let items = projections::list_by_idea_projection(pool, id.as_str()).await?;
            Ok(items.into_iter().map(GqlRun::from).collect())
        } else {
            let items = projections::list_active_projection(pool).await?;
            Ok(items.into_iter().map(GqlRun::from).collect())
        }
    }

    async fn run(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlRun>> {
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = runs::find_by_id(pool, run_id).await?;
        Ok(item.map(GqlRun::from))
    }

    async fn approval_inbox(&self, ctx: &Context<'_>) -> Result<Vec<GqlApproval>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_pending_inbox_projection(pool).await?;
        Ok(items.into_iter().map(GqlApproval::from).collect())
    }

    async fn artifacts(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlArtifact>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_artifacts_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlArtifact::from).collect())
    }

    async fn stages(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlStageExecution>> {
        let pool = ctx.data::<SqlitePool>()?;
        let parsed_run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let items = db::repos::stages::list_by_run(pool, parsed_run_id).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
    }

    async fn stage(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlStageExecution>> {
        let pool = ctx.data::<SqlitePool>()?;
        let stage_execution_id: domain::ids::StageExecutionId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = db::repos::stages::find_by_id(pool, stage_execution_id).await?;
        Ok(item.map(GqlStageExecution::from))
    }

    async fn agent_executions(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<Vec<GqlAgentExecution>> {
        let pool = ctx.data::<SqlitePool>()?;
        let stage_execution_id: domain::ids::StageExecutionId = stage_execution_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let items = db::repos::agent_executions::find_by_stage(pool, stage_execution_id).await?;
        Ok(items.into_iter().map(GqlAgentExecution::from).collect())
    }

    async fn steward_analyses(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        status: Option<String>,
    ) -> Result<Vec<GqlStewardAnalysis>> {
        let pool = ctx.data::<SqlitePool>()?;
        let parsed_status = status
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(Error::new)?;
        let items =
            steward_repo::list_analyses(pool, limit.unwrap_or(50) as i64, parsed_status).await?;
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let analysis_id = item.id.clone();
            let links = steward_repo::list_run_links(pool, &analysis_id).await?;
            let recommendations = steward_repo::list_recommendations(pool, &analysis_id).await?;
            result.push(GqlStewardAnalysis::from_parts(item, links, recommendations));
        }
        Ok(result)
    }

    async fn steward_analysis(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<GqlStewardAnalysis>> {
        let pool = ctx.data::<SqlitePool>()?;
        let item = steward_repo::find_analysis(pool, id.as_str()).await?;
        if let Some(item) = item {
            let links = steward_repo::list_run_links(pool, id.as_str()).await?;
            let recommendations = steward_repo::list_recommendations(pool, id.as_str()).await?;
            Ok(Some(GqlStewardAnalysis::from_parts(
                item,
                links,
                recommendations,
            )))
        } else {
            Ok(None)
        }
    }

    async fn steward_analysis_run_links(
        &self,
        ctx: &Context<'_>,
        analysis_id: ID,
    ) -> Result<Vec<GqlStewardAnalysisRunLink>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = steward_repo::list_run_links(pool, analysis_id.as_str()).await?;
        Ok(items
            .into_iter()
            .map(GqlStewardAnalysisRunLink::from)
            .collect())
    }

    async fn steward_recommendations(
        &self,
        ctx: &Context<'_>,
        analysis_id: ID,
    ) -> Result<Vec<GqlStewardRecommendation>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = steward_repo::list_recommendations(pool, analysis_id.as_str()).await?;
        Ok(items
            .into_iter()
            .map(GqlStewardRecommendation::from)
            .collect())
    }

    async fn scheduler_health_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlSchedulerHealthSummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::latest_health_snapshot(pool)
            .await?
            .map(gql_scheduler_health))
    }

    async fn db_writer_contention_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlDbWriterContentionSummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::latest_health_snapshot(pool)
            .await?
            .map(gql_db_writer_contention))
    }

    async fn command_latency_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlCommandLatencySummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::latest_health_snapshot(pool)
            .await?
            .map(gql_command_latency))
    }

    async fn active_execution_counts_by_provider(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<GqlSchedulerProviderActiveCount>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::list_active_execution_counts_by_provider(pool)
            .await?
            .into_iter()
            .map(GqlSchedulerProviderActiveCount::from)
            .collect())
    }

    async fn queued_backpressured_counts_by_provider_and_reason(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<GqlSchedulerQueueSummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::list_queue_summaries(pool)
            .await?
            .into_iter()
            .filter(|summary| summary.scope == "global")
            .map(gql_scheduler_queue_summary)
            .collect())
    }

    async fn run_queue_summary(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Vec<GqlSchedulerQueueSummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(
            scheduler::list_queue_summaries_by_run(pool, run_id.as_str())
                .await?
                .into_iter()
                .map(gql_scheduler_queue_summary)
                .collect(),
        )
    }

    async fn stage_queue_summary(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<Vec<GqlSchedulerQueueSummary>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(
            scheduler::list_queue_summaries_by_stage(pool, stage_execution_id.as_str())
                .await?
                .into_iter()
                .map(gql_scheduler_queue_summary)
                .collect(),
        )
    }

    async fn run_queue_position_hint(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Option<GqlSchedulerQueuePositionHint>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(scheduler::queue_position_hint_by_run(pool, run_id.as_str())
            .await?
            .map(gql_scheduler_queue_position_hint))
    }

    async fn stage_queue_position_hint(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<Option<GqlSchedulerQueuePositionHint>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(
            scheduler::queue_position_hint_by_stage(pool, stage_execution_id.as_str())
                .await?
                .map(gql_scheduler_queue_position_hint),
        )
    }

    async fn host_interruption_epochs(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Vec<GqlHostInterruptionEpoch>> {
        require_scheduler_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        Ok(
            scheduler::list_host_interruption_epochs_by_run(pool, run_id.as_str())
                .await?
                .into_iter()
                .map(gql_host_interruption_epoch)
                .collect(),
        )
    }
}

pub struct MutationRoot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationName {
    StartRun,
    ApproveStage,
    RejectStage,
    RetryStage,
    CancelRun,
}

pub fn capability_id_for(mutation: MutationName) -> domain::CapabilityToolId {
    match mutation {
        MutationName::StartRun => domain::CapabilityToolId::RunsStart,
        MutationName::ApproveStage | MutationName::RejectStage => {
            domain::CapabilityToolId::ApprovalsResolve
        }
        MutationName::RetryStage => domain::CapabilityToolId::StagesRetry,
        MutationName::CancelRun => domain::CapabilityToolId::RunsCancel,
    }
}

fn mutation_allowed(principal: &auth::Principal, mutation: MutationName) -> bool {
    auth::filter_tools(principal, &[capability_id_for(mutation)]).len() == 1
}

#[derive(SimpleObject)]
pub struct GqlDeliveryPreflight {
    pub passed: bool,
    pub timestamp: String,
    pub checks: Vec<GqlDeliveryPreflightCheck>,
}

#[derive(SimpleObject)]
pub struct GqlDeliveryPreflightCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl From<engine::preflight::DeliveryPreflightResult> for GqlDeliveryPreflight {
    fn from(preflight: engine::preflight::DeliveryPreflightResult) -> Self {
        GqlDeliveryPreflight {
            passed: preflight.passed,
            timestamp: preflight.timestamp.to_rfc3339(),
            checks: preflight
                .checks
                .into_iter()
                .map(|check| GqlDeliveryPreflightCheck {
                    id: check.id,
                    label: check.label,
                    passed: check.passed,
                    detail: check.detail,
                })
                .collect(),
        }
    }
}

// ── P029 payload wrappers ──────────────────────────────────────────────
// Dedicated types for each mutation so journal_id doesn't pollute shared
// Run/Approval types used by read queries.

#[derive(SimpleObject)]
pub struct StartRunStartedPayload {
    pub run: GqlRun,
    pub journal_id: ID,
}

#[derive(SimpleObject)]
pub struct StartRunBlockedPayload {
    pub delivery_preflight: GqlDeliveryPreflight,
    pub journal_id: ID,
}

#[derive(Union)]
pub enum StartRunPayload {
    Started(StartRunStartedPayload),
    Blocked(StartRunBlockedPayload),
}

#[derive(SimpleObject)]
pub struct ApproveStagePayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
}

#[derive(SimpleObject)]
pub struct RejectStagePayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
}

#[derive(SimpleObject)]
pub struct RetryStagePayload {
    pub retried: bool,
    pub journal_id: ID,
}

#[derive(SimpleObject)]
pub struct CancelRunPayload {
    pub cancelled: bool,
    pub journal_id: ID,
}

#[Object]
impl MutationRoot {
    async fn start_run(
        &self,
        ctx: &Context<'_>,
        idea_id: ID,
        workflow_id: String,
        workflow_title: String,
        workspace_root: String,
        artifact_root: String,
        delivery_configuration_json: Option<String>,
        workflow_yaml_path: String,
        agent_catalog_yaml_path: String,
    ) -> Result<StartRunPayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::StartRun) {
            return Err(Error::new("forbidden"));
        }

        let caller = CallerContext::graphql(&principal.id, &principal.class, "startRun");

        let iid: IdeaId = idea_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::StartRun(StartRunCmd {
            idea_id: iid,
            workflow_id,
            workflow_title,
            workspace_root,
            artifact_root,
            delivery_configuration_json,
            workflow_yaml_path,
            agent_catalog_yaml_path,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        let jid = ID::from(commanded.journal_id);
        match commanded.result {
            engine::command_handler::CommandResult::RunStarted { run_id } => {
                let run = runs::find_by_id(pool, run_id)
                    .await?
                    .ok_or_else(|| Error::new("Run not found after creation"))?;
                Ok(StartRunPayload::Started(StartRunStartedPayload {
                    run: GqlRun::from(run),
                    journal_id: jid,
                }))
            }
            engine::command_handler::CommandResult::StartRunBlockedByDeliveryPreflight(blocked) => {
                Ok(StartRunPayload::Blocked(StartRunBlockedPayload {
                    delivery_preflight: blocked.delivery_preflight.into(),
                    journal_id: jid,
                }))
            }
            _ => Err(Error::new("Unexpected command result")),
        }
    }

    async fn approve_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<ApproveStagePayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::ApproveStage) {
            return Err(Error::new("forbidden"));
        }

        let caller = CallerContext::graphql(&principal.id, &principal.class, "approveStage");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        let jid = ID::from(commanded.journal_id);
        let approval_id = match commanded.result {
            engine::command_handler::CommandResult::StageApproved { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(ApproveStagePayload {
            approval: GqlApproval::from(approval),
            journal_id: jid,
        })
    }

    async fn reject_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<RejectStagePayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::RejectStage) {
            return Err(Error::new("forbidden"));
        }

        let caller = CallerContext::graphql(&principal.id, &principal.class, "rejectStage");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RejectStage(RejectStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        let jid = ID::from(commanded.journal_id);
        let approval_id = match commanded.result {
            engine::command_handler::CommandResult::StageRejected { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(RejectStagePayload {
            approval: GqlApproval::from(approval),
            journal_id: jid,
        })
    }

    async fn retry_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
    ) -> Result<RetryStagePayload> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::RetryStage) {
            return Err(Error::new("forbidden"));
        }

        let caller = CallerContext::graphql(&principal.id, &principal.class, "retryStage");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RetryStage(RetryStageCmd {
            run_id: rid,
            stage_id,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        Ok(RetryStagePayload {
            retried: true,
            journal_id: ID::from(commanded.journal_id),
        })
    }

    async fn cancel_run(&self, ctx: &Context<'_>, run_id: ID) -> Result<CancelRunPayload> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::CancelRun) {
            return Err(Error::new("forbidden"));
        }

        let caller = CallerContext::graphql(&principal.id, &principal.class, "cancelRun");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::CancelRun(CancelRunCmd { run_id: rid });
        let commanded = cmd_handler.handle(cmd, caller).await?;
        Ok(CancelRunPayload {
            cancelled: true,
            journal_id: ID::from(commanded.journal_id),
        })
    }
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn run_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRun>>>> {
        // P029 §4.1.c: principal is injected by on_connection_init during WS handshake.
        let _principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized: no principal in subscription context"))?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::RunStatusChanged { run_id, .. }
                    | DomainEvent::RunStarted { run_id, .. } => {
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        let run = runs::find_by_id(&pool, run_id).await.ok()??;
                        Some(Ok(Some(GqlRun::from(run))))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    async fn stage_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlStageExecution>>>>
    {
        let _principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized: no principal in subscription context"))?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: RunId = run_id.parse().unwrap_or_else(|_| RunId::new());

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        ..
                    } => {
                        if run_id != filter_run_id {
                            return None;
                        }
                        let stage = db::repos::stages::find_by_id(&pool, stage_execution_id)
                            .await
                            .ok()??;
                        Some(Ok(Some(GqlStageExecution::from(stage))))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    async fn approval_requested(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>>> {
        let _principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized: no principal in subscription context"))?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalRequested { approval_id, .. } => {
                        let approval = approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    async fn scheduler_backpressure_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<
        impl async_graphql::futures_util::Stream<Item = Result<GqlSchedulerBackpressureEvent>>,
    > {
        require_scheduler_read(ctx)?;
        let events = ctx.data::<EventSender>()?.clone();

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::SchedulerBackpressureChanged {
                        run_id,
                        stage_execution_id,
                        provider_family,
                        top_reason,
                        queued_count,
                        oldest_queued_age_ms,
                        global_queue_depth,
                        state,
                        updated_at,
                        is_stale,
                    } => Some(Ok(GqlSchedulerBackpressureEvent {
                        run_id,
                        stage_execution_id,
                        provider_family,
                        top_reason,
                        queued_count,
                        oldest_queued_age_ms,
                        global_queue_depth,
                        state,
                        updated_at,
                        is_stale,
                    })),
                    _ => None,
                }
            };
            fut
        }))
    }

    /// Live stream of ACP runtime/session lifecycle events.
    /// Emits on session_started, session_completed, and session_failed.
    /// Required for the SwiftUI thin-client's runtime health surface (P027 §8.1).
    async fn runtime_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRuntimeEvent>>>>
    {
        let _principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized: no principal in subscription context"))?;

        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id,
                        agent_id,
                        provider,
                        event_kind,
                    } => {
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        Some(Ok(Some(GqlRuntimeEvent {
                            run_id: ID(run_id.to_string()),
                            stage_id,
                            agent_id,
                            provider,
                            event_kind,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        })))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }
}

/// Runtime lifecycle event surfaced to GraphQL subscribers.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlRuntimeEvent {
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub provider: String,
    /// "session_started" | "session_completed" | "session_failed"
    pub event_kind: String,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_graphql::Request;
    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifacts, ideas, projections, runs, scheduler, steward};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::steward::{
        CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
        StewardRecommendation,
    };
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::sync::Arc;

    #[test]
    fn mutation_name_converter_covers_command_mutations() {
        assert_eq!(
            capability_id_for(MutationName::StartRun),
            domain::CapabilityToolId::RunsStart
        );
        assert_eq!(
            capability_id_for(MutationName::ApproveStage),
            domain::CapabilityToolId::ApprovalsResolve
        );
        assert_eq!(
            capability_id_for(MutationName::RejectStage),
            domain::CapabilityToolId::ApprovalsResolve
        );
        assert_eq!(
            capability_id_for(MutationName::RetryStage),
            domain::CapabilityToolId::StagesRetry
        );
        assert_eq!(
            capability_id_for(MutationName::CancelRun),
            domain::CapabilityToolId::RunsCancel
        );
    }

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> domain::run::Run {
        domain::run::Run {
            id,
            idea_id,
            status: domain::run::RunStatus::Ready,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: Some(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
        }
    }

    fn test_workflow_yaml_path() -> String {
        format!(
            "{}/../../../examples/workflows/workflow.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn test_agent_catalog_yaml_path() -> String {
        format!(
            "{}/../../../examples/agents/agents.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        Arc::new(CommandHandler::new(pool, events, work_queue))
    }

    async fn seed_validation_attempt(
        pool: &sqlx::SqlitePool,
        run_id: RunId,
    ) -> (domain::ids::StageExecutionId, domain::ids::AgentExecutionId) {
        let stage_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        db::repos::stages::insert(
            pool,
            &domain::stage::StageExecution {
                id: stage_id,
                run_id,
                stage_id: "stage_1".to_string(),
                label: "Stage 1".to_string(),
                status: domain::stage::StageStatus::Failed,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: Some("validation_agent".to_string()),
                provider: Some("system".to_string()),
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id: stage_id,
                agent_id: "validation_agent".to_string(),
                provider: "system".to_string(),
                model: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                status: domain::agent::AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: Some("operator_reset".into()),
                backend_profile_id: Some("codex_with_mcp".into()),
                requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                denied_mcp_extensions_json: Some("[]".into()),
                mcp_blocking_issues_json: Some("[]".into()),
                actual_mcp_observation_json: Some(
                    r#"{"source":"provider_session_new_response"}"#.into(),
                ),
                mcp_session_startup_latency_ms: Some(17),
            },
        )
        .await
        .unwrap();
        (stage_id, agent_execution_id)
    }

    fn validation_failure_payload(run_id: RunId) -> serde_json::Value {
        serde_json::json!({
            "id": "33333333-3333-3333-3333-333333333333",
            "timestamp": "2026-04-15T09:30:00Z",
            "agentID": "validation_agent",
            "stageID": "stage_1",
            "runID": run_id.to_string(),
            "outputResults": [{
                "outputName": "report",
                "contractID": "report_v1",
                "status": "failed",
                "missingFields": ["summary"],
                "validationError": "Missing required fields: summary",
                "rawPayloadSize": 17
            }],
            "failureSummary": "report: Missing required fields: summary",
            "failureClass": "output_contract_mismatch",
            "contractMetadata": [{
                "outputName": "report",
                "contractID": "report_v1",
                "machineFormat": "json",
                "validationMode": "strict_structured",
                "requiredFieldCount": 1,
                "rawArtifactName": "report_raw",
                "normalizedArtifactName": "report"
            }],
            "rawOutputExists": true,
            "receiptExists": false,
            "transcriptExists": true,
            "recoveryRecommendation": {
                "action": "retry_failed_agent",
                "explanation": "Retry the agent with the same inputs.",
                "source": "runtime_policy"
            }
        })
    }

    fn validation_failure_record(
        artifact_id: ArtifactId,
        run_id: RunId,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) -> ValidationFailureRecord {
        ValidationFailureRecord {
            id: "33333333-3333-3333-3333-333333333333".to_string(),
            artifact_id,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_id: "validation_agent".to_string(),
            stage_id: "stage_1".to_string(),
            stage_execution_id,
            agent_execution_id,
            run_id,
            output_results: vec![OutputValidationResult {
                output_name: "report".to_string(),
                contract_id: Some("report_v1".to_string()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary".to_string()],
                validation_error: Some("Missing required fields: summary".to_string()),
                raw_payload_size: 17,
            }],
            failure_summary: "report: Missing required fields: summary".to_string(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "report".to_string(),
                contract_id: "report_v1".to_string(),
                machine_format: "json".to_string(),
                validation_mode: "strict_structured".to_string(),
                required_field_count: 1,
                raw_artifact_name: Some("report_raw".to_string()),
                normalized_artifact_name: Some("report".to_string()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the agent with the same inputs.".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn start_run_accepts_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-1","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(
                r#"
                mutation StartRun {
                  startRun(
                    ideaId: "IDEA_ID",
                    workflowId: "wf-start",
                    workflowTitle: "Start Run",
                    workspaceRoot: "/tmp/ws",
                    artifactRoot: "/tmp/art",
                    workflowYamlPath: "WORKFLOW_YAML_PATH",
                    agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                    deliveryConfigurationJson: DELIVERY_CONFIG
                  ) {
                    ... on StartRunStartedPayload { run { id } journalId }
                    ... on StartRunBlockedPayload { deliveryPreflight { passed checks { id passed detail } } journalId }
                  }
                }
                "#
                .replace("IDEA_ID", &idea_id.to_string())
                .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path())
                .replace(
                    "DELIVERY_CONFIG",
                    &serde_json::to_string(&delivery_json).unwrap(),
                ),
            ).data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "mutation must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let run_id = json["startRun"]["run"]["id"].as_str().unwrap();
        let run = runs::find_by_id(&pool, run_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(run.delivery_configuration_json, Some(delivery_json));
    }

    #[tokio::test]
    async fn start_run_blocked_preflight_returns_typed_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(
                r#"
                mutation StartRun {
                  startRun(
                    ideaId: "IDEA_ID",
                    workflowId: "wf-blocked",
                    workflowTitle: "Blocked Run",
                    workspaceRoot: "/tmp/ws",
                    artifactRoot: "/tmp/art",
                    workflowYamlPath: "WORKFLOW_YAML_PATH",
                    agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                    deliveryConfigurationJson: "{\"repo_identifier\":\"repo-blocked\",\"repo_root\":\"/definitely/missing/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp\",\"target_branch\":\"cw/release\",\"release_target_id\":\"app-store\"}"
                  ) {
                    ... on StartRunStartedPayload { run { id } journalId }
                    ... on StartRunBlockedPayload {
                      deliveryPreflight {
                        passed
                        timestamp
                        checks { id label passed detail }
                      }
                      journalId
                    }
                  }
                }
                "#
                .replace("IDEA_ID", &idea_id.to_string())
                .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path()),
            ).data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "mutation must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let preflight = &json["startRun"]["deliveryPreflight"];
        assert_eq!(preflight["passed"], serde_json::json!(false));
        assert!(
            preflight["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|check| check["id"] == serde_json::json!("repo_root_exists")
                    && check["passed"] == serde_json::json!(false)),
            "typed preflight checks must include failing repo_root_exists: {preflight:?}"
        );
    }

    #[tokio::test]
    async fn run_query_exposes_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-2","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let start = schema
            .execute(Request::new(
                r#"
                mutation StartRun {
                  startRun(
                    ideaId: "IDEA_ID",
                    workflowId: "wf-query",
                    workflowTitle: "Query Run",
                    workspaceRoot: "/tmp/ws",
                    artifactRoot: "/tmp/art",
                    workflowYamlPath: "WORKFLOW_YAML_PATH",
                    agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                    deliveryConfigurationJson: DELIVERY_CONFIG
                  ) {
                    ... on StartRunStartedPayload { run { id } journalId }
                    ... on StartRunBlockedPayload { deliveryPreflight { passed checks { id passed detail } } journalId }
                  }
                }
                "#
                .replace("IDEA_ID", &idea_id.to_string())
                .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path())
                .replace(
                    "DELIVERY_CONFIG",
                    &serde_json::to_string(&delivery_json).unwrap(),
                ),
            ).data(test_principal()))
            .await;

        assert!(start.errors.is_empty(), "mutation must succeed: {start:?}");
        let run_id = start.data.into_json().unwrap()["startRun"]["run"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = schema
            .execute(Request::new(format!(
                r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryConfigurationJson
                  }}
                }}
                "#
            )))
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["run"]["deliveryConfigurationJson"],
            serde_json::json!(delivery_json)
        );
    }

    #[tokio::test]
    async fn delivery_preflight_graphql_readback_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-graphql-preflight","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let start = schema
            .execute(
                Request::new(
                    r#"
                mutation StartRun {
                  startRun(
                    ideaId: "IDEA_ID",
                    workflowId: "wf-preflight-readback",
                    workflowTitle: "Preflight Readback",
                    workspaceRoot: "/tmp/ws",
                    artifactRoot: "/tmp/art",
                    workflowYamlPath: "WORKFLOW_YAML_PATH",
                    agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                    deliveryConfigurationJson: DELIVERY_CONFIG
                  ) {
                    ... on StartRunStartedPayload { run { id deliveryPreflightJson } }
                    ... on StartRunBlockedPayload { deliveryPreflight { passed } }
                  }
                }
                "#
                    .replace("IDEA_ID", &idea_id.to_string())
                    .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                    .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path())
                    .replace(
                        "DELIVERY_CONFIG",
                        &serde_json::to_string(&delivery_json).unwrap(),
                    ),
                )
                .data(test_principal()),
            )
            .await;

        assert!(start.errors.is_empty(), "mutation must succeed: {start:?}");
        let start_json = start.data.into_json().unwrap();
        let run_id = start_json["startRun"]["run"]["id"].as_str().unwrap();
        assert!(start_json["startRun"]["run"]["deliveryPreflightJson"]
            .as_str()
            .unwrap()
            .contains(r#""passed":true"#));

        let response = schema
            .execute(Request::new(format!(
                r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryPreflightJson
                  }}
                }}
                "#
            )))
            .await;
        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert!(json["run"]["deliveryPreflightJson"]
            .as_str()
            .unwrap()
            .contains("repo_root_exists"));
    }

    #[tokio::test]
    async fn proposal_061_scheduler_graphql_readback_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let now = Utc::now();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        scheduler::insert_health_snapshot(
            &pool,
            &scheduler::SchedulerHealthSnapshot {
                id: "scheduler-health-graphql".into(),
                queued_count: 3,
                oldest_queued_age_ms: 310_000,
                global_queue_depth: 3,
                active_agent_executions: 20,
                db_writer_wait_p95_ms: Some(37),
                command_latency_p95_ms_json: Some(
                    r#"{"approve_stage":120,"retry_stage":180,"cancel_run":90}"#.into(),
                ),
                last_host_interruption_epoch_id: Some("graphql-host-epoch".into()),
                sustained_backpressure_state: "active".into(),
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::upsert_queue_summary(
            &pool,
            &scheduler::SchedulerQueueSummary {
                scope: "global".into(),
                scope_id: "".into(),
                run_id: None,
                stage_execution_id: None,
                provider_family: Some("codex".into()),
                top_reason: "provider_capacity".into(),
                queued_count: 2,
                oldest_queued_age_ms: 210_000,
                global_queue_depth: 3,
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::upsert_queue_summary(
            &pool,
            &scheduler::SchedulerQueueSummary {
                scope: "run".into(),
                scope_id: run_id.to_string(),
                run_id: Some(run_id.to_string()),
                stage_execution_id: None,
                provider_family: Some("claude".into()),
                top_reason: "run_capacity".into(),
                queued_count: 1,
                oldest_queued_age_ms: 180_000,
                global_queue_depth: 3,
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::upsert_queue_summary(
            &pool,
            &scheduler::SchedulerQueueSummary {
                scope: "stage".into(),
                scope_id: stage_execution_id.to_string(),
                run_id: Some(run_id.to_string()),
                stage_execution_id: Some(stage_execution_id.to_string()),
                provider_family: Some("claude".into()),
                top_reason: "run_capacity".into(),
                queued_count: 1,
                oldest_queued_age_ms: 180_000,
                global_queue_depth: 3,
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        let ahead_payload = serde_json::json!({
            "run_id": RunId::new().to_string(),
            "stage_id": "stage-ahead",
            "stage_execution_id": domain::ids::StageExecutionId::new().to_string(),
            "agent_id": "claude-agent",
            "provider": "claude",
        });
        sqlx::query(
            r#"INSERT INTO work_items
               (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at)
               VALUES ('graphql-ahead-work', 'invoke_agent', ?1, 'pending', ?2, 'stage-ahead', ?3, ?4)"#,
        )
        .bind(ahead_payload.to_string())
        .bind(ahead_payload["run_id"].as_str().unwrap())
        .bind((now - chrono::Duration::seconds(30)).to_rfc3339())
        .bind((now - chrono::Duration::seconds(30)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let target_payload = serde_json::json!({
            "run_id": run_id.to_string(),
            "stage_id": "stage-p061",
            "stage_execution_id": stage_execution_id.to_string(),
            "agent_id": "codex-agent",
            "provider": "codex",
        });
        sqlx::query(
            r#"INSERT INTO work_items
               (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at)
               VALUES ('graphql-target-work', 'invoke_agent', ?1, 'pending', ?2, 'stage-p061', ?3, ?4)"#,
        )
        .bind(target_payload.to_string())
        .bind(run_id.to_string())
        .bind((now - chrono::Duration::seconds(15)).to_rfc3339())
        .bind((now - chrono::Duration::seconds(15)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        sqlx::query(
            r#"INSERT INTO stage_executions
               (id, run_id, stage_id, label, status, started_at)
               VALUES (?1, ?2, 'stage-host', 'Host interruption stage', 'running', ?3)"#,
        )
        .bind(stage_execution_id.to_string())
        .bind(run_id.to_string())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
               VALUES (?1, ?2, 'agent-host', 'codex', 'codex', 'running', ?3)"#,
        )
        .bind(agent_execution_id.to_string())
        .bind(stage_execution_id.to_string())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        scheduler::insert_host_interruption_epoch(
            &pool,
            &scheduler::HostInterruptionEpoch {
                id: "graphql-host-epoch".into(),
                kind: "network_migration".into(),
                started_at: now - chrono::Duration::seconds(70),
                ended_at: Some(now - chrono::Duration::seconds(20)),
                monotonic_gap_ms: Some(50_000),
                wall_clock_gap_ms: Some(70_000),
                details_json: Some(r#"{"path":"wifi"}"#.into()),
                created_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::insert_host_interruption_affected_execution(
            &pool,
            &scheduler::HostInterruptionAffectedExecution {
                epoch_id: "graphql-host-epoch".into(),
                agent_execution_id: agent_execution_id.to_string(),
                run_id: Some(run_id.to_string()),
                stage_execution_id: stage_execution_id.to_string(),
                provider_family: Some("codex".into()),
                action: "resuming_after_network_change".into(),
                retry_enqueued_at: None,
                created_at: now,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query SchedulerReadback {{
                  schedulerHealthSummary {{
                    queuedCount
                    oldestQueuedAgeMs
                    dbWriterWaitP95Ms
                    commandLatencyP95MsJson
                    lastHostInterruptionEpochId
                    sustainedBackpressureState
                    isStale
                  }}
                  dbWriterContentionSummary {{
                    dbWriterWaitP95Ms
                    isStale
                  }}
                  commandLatencySummary {{
                    commandLatencyP95MsJson
                    isStale
                  }}
                  queuedBackpressuredCountsByProviderAndReason {{
                    scope
                    providerFamily
                    topReason
                    queuedCount
                  }}
                  runQueueSummary(runId: "{run_id}") {{
                    scope
                    topReason
                    queuedCount
                  }}
                  stageQueueSummary(stageExecutionId: "{stage_execution_id}") {{
                    scope
                    stageExecutionId
                    topReason
                  }}
                  runQueuePositionHint(runId: "{run_id}") {{
                    scope
                    queuePosition
                    queuedAheadCount
                    globalQueueDepth
                  }}
                  stageQueuePositionHint(stageExecutionId: "{stage_execution_id}") {{
                    scope
                    stageExecutionId
                    queuePosition
                  }}
                  activeExecutionCountsByProvider {{
                    providerFamily
                    activeCount
                  }}
                  hostInterruptionEpochs(runId: "{run_id}") {{
                    kind
                    wallClockGapMs
                    affectedExecutions {{
                      agentExecutionId
                      providerFamily
                      action
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["schedulerHealthSummary"]["sustainedBackpressureState"],
            serde_json::json!("active")
        );
        assert_eq!(
            json["schedulerHealthSummary"]["dbWriterWaitP95Ms"],
            serde_json::json!(37)
        );
        assert_eq!(
            json["schedulerHealthSummary"]["lastHostInterruptionEpochId"],
            serde_json::json!("graphql-host-epoch")
        );
        assert_eq!(
            json["dbWriterContentionSummary"]["dbWriterWaitP95Ms"],
            serde_json::json!(37)
        );
        assert_eq!(
            json["commandLatencySummary"]["commandLatencyP95MsJson"],
            serde_json::json!(r#"{"approve_stage":120,"retry_stage":180,"cancel_run":90}"#)
        );
        assert_eq!(
            json["queuedBackpressuredCountsByProviderAndReason"][0]["providerFamily"],
            serde_json::json!("codex")
        );
        assert_eq!(
            json["runQueueSummary"][0]["topReason"],
            serde_json::json!("run_capacity")
        );
        assert_eq!(
            json["stageQueueSummary"][0]["stageExecutionId"],
            serde_json::json!(stage_execution_id.to_string())
        );
        assert_eq!(
            json["runQueuePositionHint"]["queuePosition"],
            serde_json::json!(2)
        );
        assert_eq!(
            json["runQueuePositionHint"]["queuedAheadCount"],
            serde_json::json!(1)
        );
        assert_eq!(
            json["stageQueuePositionHint"]["stageExecutionId"],
            serde_json::json!(stage_execution_id.to_string())
        );
        assert_eq!(
            json["activeExecutionCountsByProvider"],
            serde_json::json!([{
                "providerFamily": "codex",
                "activeCount": 1
            }])
        );
        assert_eq!(
            json["hostInterruptionEpochs"][0]["kind"],
            serde_json::json!("network_migration")
        );
        assert_eq!(
            json["hostInterruptionEpochs"][0]["affectedExecutions"][0]["action"],
            serde_json::json!("resuming_after_network_change")
        );
    }

    #[tokio::test]
    async fn proposal_061_scheduler_backpressure_subscription_maps_domain_event() {
        let pool = test_pool().await;
        let events = event_bus::new_bus(16);
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            events.clone(),
            auth::PrincipalTable::test_fixture(),
        );
        let mut stream = Box::pin(
            schema.execute_stream(
                Request::new(
                    r#"
                subscription SchedulerBackpressureChanged {
                  schedulerBackpressureChanged {
                    runId
                    stageExecutionId
                    providerFamily
                    topReason
                    queuedCount
                    oldestQueuedAgeMs
                    globalQueueDepth
                    state
                    updatedAt
                    isStale
                  }
                }
                "#,
                )
                .data(test_principal()),
            ),
        );
        let run_id = RunId::new().to_string();
        let stage_execution_id = domain::ids::StageExecutionId::new().to_string();
        let updated_at = Utc::now().to_rfc3339();
        let events_for_task = events.clone();
        let run_id_for_task = run_id.clone();
        let stage_execution_id_for_task = stage_execution_id.clone();
        let updated_at_for_task = updated_at.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = events_for_task.send(DomainEvent::SchedulerBackpressureChanged {
                run_id: Some(run_id_for_task),
                stage_execution_id: Some(stage_execution_id_for_task),
                provider_family: Some("codex".into()),
                top_reason: "provider_capacity".into(),
                queued_count: 2,
                oldest_queued_age_ms: 310_000,
                global_queue_depth: 5,
                state: "active".into(),
                updated_at: updated_at_for_task,
                is_stale: false,
            });
        });

        let response = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("subscription response should arrive")
            .expect("subscription stream should yield");
        assert!(
            response.errors.is_empty(),
            "subscription must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let event = &json["schedulerBackpressureChanged"];
        assert_eq!(event["runId"], serde_json::json!(run_id));
        assert_eq!(
            event["stageExecutionId"],
            serde_json::json!(stage_execution_id)
        );
        assert_eq!(event["providerFamily"], serde_json::json!("codex"));
        assert_eq!(event["topReason"], serde_json::json!("provider_capacity"));
        assert_eq!(event["queuedCount"], serde_json::json!(2));
        assert_eq!(event["oldestQueuedAgeMs"], serde_json::json!(310_000));
        assert_eq!(event["globalQueueDepth"], serde_json::json!(5));
        assert_eq!(event["state"], serde_json::json!("active"));
        assert_eq!(event["updatedAt"], serde_json::json!(updated_at));
        assert_eq!(event["isStale"], serde_json::json!(false));
    }

    #[tokio::test]
    async fn execution_mcp_truth_contract_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(format!(
                r#"
                query StageExecutions {{
                  stages(runId: "{run_id}") {{
                    id
                    executions {{
                      backendProfileId
                      requestedMcpExtensionsJson
                      predictedMcpRuntimeIdsJson
                      actualMcpRuntimeIdsJson
                      mcpBlockingIssuesJson
                    }}
                  }}
                  agentExecutions(stageExecutionId: "{stage_execution_id}") {{
                    backendProfileId
                    actualMcpObservationJson
                  }}
                }}
                "#
            )))
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let execution = &json["stages"][0]["executions"][0];
        assert_eq!(
            execution["backendProfileId"],
            serde_json::json!("codex_with_mcp")
        );
        assert_eq!(
            execution["requestedMcpExtensionsJson"],
            serde_json::json!(r#"["filesystem"]"#)
        );
        assert_eq!(
            execution["actualMcpRuntimeIdsJson"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
        assert_eq!(
            json["agentExecutions"][0]["actualMcpObservationJson"],
            serde_json::json!(r#"{"source":"provider_session_new_response"}"#)
        );
    }

    #[tokio::test]
    async fn run_query_exposes_cancellation_settlement_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(
            &pool,
            &domain::run::Run {
                cancellation_settlement_log: Some(
                    serde_json::json!([
                        {
                            "agent_execution_id": "ae-1",
                            "agent_id": "writer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": true,
                            "settled_at": "2026-04-15T10:00:00Z"
                        }
                    ])
                    .to_string(),
                ),
                ..make_run(run_id, idea_id)
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(format!(
                r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    cancellationSettlementLog
                  }}
                }}
                "#
            )))
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(json["run"]["cancellationSettlementLog"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            parsed,
            serde_json::json!([
                {
                    "agent_execution_id": "ae-1",
                    "agent_id": "writer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": true,
                    "settled_at": "2026-04-15T10:00:00Z"
                }
            ])
        );
    }

    #[tokio::test]
    async fn runs_query_exposes_cancellation_settlement_summary_only() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(
            &pool,
            &domain::run::Run {
                status: domain::run::RunStatus::Cancelling,
                cancellation_settlement_log: Some(
                    serde_json::json!([
                        {
                            "agent_execution_id": "ae-1",
                            "agent_id": "writer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": true,
                            "settled_at": "2026-04-15T10:00:00Z"
                        },
                        {
                            "agent_execution_id": "ae-2",
                            "agent_id": "reviewer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": false,
                            "settled_at": "2026-04-15T10:00:02Z"
                        }
                    ])
                    .to_string(),
                ),
                ..make_run(run_id, idea_id)
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(
                r#"
                query Runs {
                  runs {
                    id
                    cancellationSettlementSummary
                    cancellationSettlementLog
                  }
                }
                "#,
            ))
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["runs"][0]["cancellationSettlementSummary"],
            serde_json::json!("2/2 agents settled, 1 sessions closed")
        );
        assert!(json["runs"][0]["cancellationSettlementLog"].is_null());
    }

    #[tokio::test]
    async fn artifacts_query_decodes_validation_failure_record() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let payload_path = std::env::temp_dir().join(format!("validation-failure-{}.json", run_id));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&validation_failure_payload(run_id)).unwrap(),
        )
        .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;

        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        let record =
            validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id);
        db::repos::validation::insert(&pool, &record).await.unwrap();

        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(format!(
                r#"
                query Artifacts {{
                  artifacts(runId: "{run_id}") {{
                    name
                    reportKind
                    validationFailureRecord {{
                      failureSummary
                      failureClass
                      sessionReuseDisposition
                      sessionResetReason
                      outputResults {{
                        outputName
                        missingFields
                      }}
                    }}
                  }}
                }}
                "#
            )))
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        let validation_failure = artifacts
            .iter()
            .find(|artifact| artifact["reportKind"] == serde_json::json!("validation_failure"))
            .expect("validation failure artifact");

        assert_eq!(
            validation_failure["validationFailureRecord"]["failureSummary"],
            serde_json::json!("report: Missing required fields: summary")
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["outputResults"][0]["missingFields"],
            serde_json::json!(["summary"])
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["sessionReuseDisposition"],
            serde_json::json!("reused")
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["sessionResetReason"],
            serde_json::json!("operator_reset")
        );
    }

    #[tokio::test]
    async fn steward_graphql_readback_tests_exposes_analysis_rows() {
        let pool = test_pool().await;
        let now = Utc::now();
        let analysis = StewardAnalysis {
            id: "analysis-1".into(),
            created_at: now,
            window_start: now,
            window_end: now,
            run_count: 5,
            cohort_keys_json: serde_json::json!({
                "workflow_family": "mvp_live",
                "risk_class": "high"
            })
            .to_string(),
            cohort_quality: CohortQuality::Weak,
            status: StewardAnalysisStatus::Inconclusive,
            degradation_count: 1,
            improvement_count: 0,
            workflow_snapshot_artifact_hash: "workflow-hash".into(),
            agent_catalog_snapshot_hash: "catalog-hash".into(),
            steward_config_snapshot_hash: "config-hash".into(),
            metrics_snapshot_artifact_id: Some("/tmp/steward/metrics-window.json".into()),
            baseline_snapshot_artifact_id: Some("/tmp/steward/baseline-window.json".into()),
            agent_catalog_snapshot_artifact_id: Some("/tmp/steward/catalog-snapshot.json".into()),
            workflow_snapshot_artifact_id: Some("/tmp/steward/workflow-snapshot.json".into()),
            config_change_log_artifact_id: Some("/tmp/steward/config-change-log.json".into()),
            health_report_artifact_id: None,
            degradation_alert_artifact_id: Some("/tmp/steward/degradation-alert.json".into()),
            agent_tuning_artifact_id: None,
            workflow_tuning_artifact_id: None,
            experiment_plan_artifact_id: None,
            audit_report_artifact_id: None,
            trigger_reason: "manual".into(),
            error_summary: None,
        };
        steward::insert_analysis(&pool, &analysis).await.unwrap();
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        steward::insert_run_link(
            &pool,
            &StewardAnalysisRunLink {
                id: "link-1".into(),
                analysis_id: "analysis-1".into(),
                run_id: run_id.to_string(),
                role: "implicated".into(),
            },
        )
        .await
        .unwrap();
        steward::insert_recommendation(
            &pool,
            &StewardRecommendation {
                id: "rec-1".into(),
                analysis_id: "analysis-1".into(),
                created_at: now,
                category: "degradation".into(),
                summary: "Lead time regressed".into(),
                target_metric: "lead_time_median_seconds".into(),
                confidence_level: "high".into(),
                status: "proposed".into(),
                source_artifact_name: Some("deterministic_signal".into()),
                decision_comment: None,
                decided_at: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let response = schema
            .execute(Request::new(
                r#"
                query Steward {
                  stewardAnalyses(limit: 10) {
                    id
                    status
                    triggerReason
                    cohortKeysJson
                    cohortQuality
                    runCount
                    degradationCount
                    artifactIds
                    recommendations { id targetMetric status }
                    linkedRuns { id runId role }
                  }
                  stewardAnalysis(id: "analysis-1") {
                    id
                    stewardConfigSnapshotHash
                    recommendations { id summary }
                    linkedRuns { id role }
                  }
                }
                "#,
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["stewardAnalyses"][0]["id"],
            "analysis-1"
        );
    }
}
