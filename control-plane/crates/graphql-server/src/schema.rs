use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_graphql::futures_util::StreamExt;
use async_graphql::*;
use sqlx::{Row, SqlitePool};
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, warn};

use db::repos::{
    approvals, artifact_contracts, artifacts, closeout, code_writer_completion_receipts, ideas,
    projections, rollout_contract_checks, runs, steward as steward_repo, workflow_conflicts,
};
use db::writer::DbWriterHeartbeat;
use domain::commands::{ApprovalResolutionDecision, CallerContext, Command, ResolveApprovalCmd};
use domain::events::DomainEvent;
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::lifecycle::DaemonStatus;
use engine::command_handler::{ApprovalResolutionConflict, CommandHandler};
use engine::event_bus::EventSender;
use engine::lifecycle_reporter::LifecycleReporter;

use crate::types::approval::GqlApproval;
use crate::types::artifact::{GqlArtifact, P085_NO_DEADLINE_JUSTIFICATION};
use crate::types::idea::GqlIdea;
use crate::types::p031::{
    GqlMutationConflictResultCode, GqlPayloadAvailabilityState, GqlPayloadUnavailableReasonCode,
};
use crate::types::run::GqlRun;
use crate::types::scheduler::{GqlStartupRecoverySummary, GqlToolchainCacheHousekeepingSummary};
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
    reporter: LifecycleReporter,
) -> AppSchema {
    build_schema_inner(pool, cmd_handler, events, principal_table, reporter, None)
}

pub fn build_schema_with_storage_writer(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
) -> AppSchema {
    build_schema_inner(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        Some(storage_writer_heartbeat),
    )
}

fn build_schema_inner(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
) -> AppSchema {
    let mut builder = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .data(principal_table)
        .data(reporter);
    if let Some(heartbeat) = storage_writer_heartbeat {
        builder = builder.data(heartbeat);
    }
    builder.finish()
}

pub struct QueryRoot;

fn require_operator_read(ctx: &Context<'_>) -> Result<()> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;
    if principal.class != auth::PrincipalClass::Operator {
        return Err(Error::new("forbidden"));
    }
    // P072: enforce allow_queries surface policy when present.
    if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
        if let Some(allowed) = auth::is_query_allowed_by_surface_policy(table, &principal.id) {
            if !allowed {
                return Err(Error::new("forbidden"));
            }
        }
    }
    Ok(())
}

async fn run_from_projection_or_canonical(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<GqlRun>> {
    let item = runs::find_by_id(pool, run_id).await?;
    if let Some(run) = item {
        if let Some(projection) =
            projections::find_run_projection(pool, &run_id.to_string()).await?
        {
            let mut gql = GqlRun::from_projection_and_run(projection, run);
            enrich_run_with_artifact_contracts(pool, run_id, &mut gql).await?;
            Ok(Some(gql))
        } else {
            let mut gql = GqlRun::from(run);
            enrich_run_with_artifact_contracts(pool, run_id, &mut gql).await?;
            Ok(Some(gql))
        }
    } else {
        Ok(None)
    }
}

async fn enrich_run_with_artifact_contracts(
    pool: &SqlitePool,
    run_id: RunId,
    gql: &mut GqlRun,
) -> Result<()> {
    if let Some(projection) =
        db::repos::artifact_contracts::find_run_state_projection(pool, run_id).await?
    {
        gql.active_artifact_index_json =
            Some(serde_json::to_string(&projection.active_index_json)?);
        gql.run_state_projection_json = Some(serde_json::to_string(&projection.run_state_json)?);
        let overrides = db::repos::artifact_contracts::list_overrides(pool, run_id).await?;
        gql.operator_overrides_json = Some(serde_json::to_string(&overrides)?);
    }
    let legacy_overrides = db::repos::legacy_discovery_overrides::list_by_run(pool, run_id).await?;
    gql.legacy_discovery_overrides_json = Some(serde_json::to_string(&legacy_overrides)?);
    gql.implementation_self_assessment_summary =
        artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
            .await?
            .map(|stored| stored.summary.into());
    gql.rollout_contract_readback_json =
        rollout_contract_checks::find_terminal_rollout_contract_check_for_run(pool, run_id.inner())
            .await?
            .map(|check| Json(check.operator_readback_json_for_lane("graphql")));
    gql.side_effect_readback_json = Some(Json(side_effect_readback_json(pool, run_id).await?));
    let code_writer_completion_readbacks =
        code_writer_completion_receipts::list_by_run(pool, run_id).await?;
    let canonical_code_writer_completion_readbacks =
        code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
    gql.implementation_completion =
        domain::code_writer_completion::project_implementation_completion(
            &canonical_code_writer_completion_readbacks,
        )
        .into();
    gql.code_writer_completion_receipts = code_writer_completion_readbacks
        .into_iter()
        .map(Into::into)
        .collect();
    gql.workflow_conflict = workflow_conflicts::get_current_blocking_conflict(pool, run_id)
        .await?
        .map(Into::into);
    // P017: Enrich workflow conflict with lead mediation readback if present.
    // API-001 (P017 R2 audit): the enriched projection includes
    // mediation-owned `execution_attempts` so operators can inspect the
    // mediation's runtime facts, watchdog outcome, artifacts, and
    // provider/timing details directly through the conflict surface.
    if let Some(ref mut conflict) = gql.workflow_conflict {
        if let Some(ref mediation_id) = conflict.mediation_record_id {
            if let Ok(Some(med)) =
                db::repos::lead_conflict_mediations::find_by_id(pool, mediation_id).await
            {
                conflict.lead_mediation = Some(
                    crate::types::run::GqlLeadMediation::build_with_attempts(pool, &med).await?,
                );
            }
        }
    }
    gql.implementation_handoff_status_json = if let Some(status) =
        workflow_conflicts::get_implementation_handoff_status(pool, run_id).await?
    {
        Some(async_graphql::Json(serde_json::to_value(status)?))
    } else {
        None
    };
    gql.main_sync_readback_json = Some(async_graphql::Json(
        proposal_064_main_sync_readback(pool, run_id).await?,
    ));
    gql.knowledge_capsule_readback_json = Some(async_graphql::Json(
        proposal_064_knowledge_capsule_readback(pool, run_id).await?,
    ));
    // P077: Populate closeout readiness summary via CloseoutReadinessSummaryAccessor.
    if let Some(summary) =
        closeout::load_closeout_readiness_summary(pool, &run_id.to_string()).await?
    {
        let summary_json = async_graphql::Json(serde_json::to_value(&summary)?);
        gql.closeout_readiness_summary_json = Some(summary_json.clone());
        gql.implementation_closeout_readiness_summary = Some(summary_json);
    }
    Ok(())
}

async fn stage_from_projection_or_canonical(
    pool: &SqlitePool,
    stage_execution_id: domain::ids::StageExecutionId,
) -> Result<Option<GqlStageExecution>> {
    let item = db::repos::stages::find_by_id(pool, stage_execution_id).await?;
    if let Some(stage) = item {
        let projection = projections::list_stages_projection(pool, &stage.run_id.to_string())
            .await?
            .into_iter()
            .find(|row| row.id == stage.id.to_string());
        if let Some(projection) = projection {
            Ok(Some(GqlStageExecution::from_projection_and_stage(
                projection, stage,
            )))
        } else {
            Ok(Some(GqlStageExecution::from(stage)))
        }
    } else {
        Ok(None)
    }
}

#[Object]
impl QueryRoot {
    async fn ideas(
        &self,
        ctx: &Context<'_>,
        include_archived: Option<bool>,
    ) -> Result<Vec<GqlIdea>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let include = include_archived.unwrap_or(false);
        let items = ideas::list(pool, include).await?;
        Ok(items.into_iter().map(GqlIdea::from).collect())
    }

    async fn idea(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlIdea>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let idea_id: IdeaId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = ideas::find_by_id(pool, idea_id).await?;
        Ok(item.map(GqlIdea::from))
    }

    async fn runs(&self, ctx: &Context<'_>, idea_id: Option<ID>) -> Result<Vec<GqlRun>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        if let Some(id) = idea_id {
            let items = projections::list_by_idea_projection(pool, id.as_str()).await?;
            runs_with_latest_summaries(pool, items.into_iter().map(GqlRun::from).collect()).await
        } else {
            let items = projections::list_active_projection(pool).await?;
            runs_with_latest_summaries(pool, items.into_iter().map(GqlRun::from).collect()).await
        }
    }

    async fn run(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlRun>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        run_from_projection_or_canonical(pool, run_id).await
    }

    async fn approval_inbox(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> Result<Vec<GqlApproval>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_pending_inbox_projection(pool).await?;
        Ok(items
            .into_iter()
            .filter(|row| {
                run_id.as_ref().map_or(true, |requested_run_id| {
                    row.run_id == requested_run_id.as_str()
                })
            })
            .map(GqlApproval::from)
            .collect())
    }

    async fn artifacts(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlArtifact>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let parsed_run_id: RunId = run_id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let run = runs::find_by_id(pool, parsed_run_id).await?;
        let items = projections::list_artifacts_projection(pool, run_id.as_str()).await?;
        let should_attach_payload = ctx.look_ahead().field("payloadText").exists();
        debug!(
            run_id = %run_id.as_str(),
            artifact_count = items.len(),
            payload_requested = should_attach_payload,
            "P031 artifacts query"
        );
        if should_attach_payload {
            info!(
                run_id = %run_id.as_str(),
                artifact_count = items.len(),
                "P031 bulk artifact payload requested"
            );
        }
        let mut bulk_preview_budget_remaining = P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES;
        Ok(items
            .into_iter()
            .map(|row| {
                let mut artifact = GqlArtifact::from(row.clone());
                if should_attach_payload {
                    attach_p031_artifact_payload(
                        &row,
                        run.as_ref(),
                        &mut artifact,
                        &mut bulk_preview_budget_remaining,
                    );
                }
                artifact
            })
            .collect())
    }

    async fn artifact(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlArtifact>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let artifact_id: ArtifactId = id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let Some(row) = artifacts::find_by_id(pool, artifact_id).await? else {
            debug!(artifact_id = %id.as_str(), "P031 selected artifact query missed");
            return Ok(None);
        };
        let run = runs::find_by_id(pool, row.run_id).await?;
        let format = row.format.to_string();
        let mut artifact = GqlArtifact::from(row.clone());
        let should_attach_payload = ctx.look_ahead().field("payloadText").exists();
        debug!(
            artifact_id = %id.as_str(),
            run_id = %row.run_id,
            payload_requested = should_attach_payload,
            "P031 selected artifact query"
        );
        if should_attach_payload {
            let mut preview_budget = P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES;
            attach_p031_artifact_payload_from_metadata(
                &format,
                row.report_kind.as_deref(),
                row.size_bytes,
                &row.file_path,
                run.as_ref(),
                &mut artifact,
                &mut preview_budget,
            );
        }
        debug!(
            artifact_id = %artifact.id.as_str(),
            payload_state = ?artifact.payload_availability_state,
            has_payload = artifact.payload_text.as_ref().is_some_and(|text| !text.is_empty()),
            "P031 selected artifact response"
        );
        Ok(Some(artifact))
    }

    async fn stages(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlStageExecution>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_stages_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
    }

    /// Work-queue counts for all items associated with a run.
    async fn run_queue_summary(&self, ctx: &Context<'_>, run_id: ID) -> Result<GqlRunQueueSummary> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id_str = run_id.as_str();
        let rows = sqlx::query(
            r#"SELECT status, COUNT(*) AS cnt FROM work_items WHERE run_id = ?1 GROUP BY status"#,
        )
        .bind(run_id_str)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
        let mut pending = 0i64;
        let mut running = 0i64;
        let mut completed = 0i64;
        let mut failed = 0i64;
        let mut cancelled = 0i64;
        for row in &rows {
            let status: String = row.get("status");
            let cnt: i64 = row.get("cnt");
            match status.as_str() {
                "pending" => pending = cnt,
                "running" => running = cnt,
                "completed" => completed = cnt,
                "failed" => failed = cnt,
                "cancelled" => cancelled = cnt,
                _ => {}
            }
        }
        Ok(GqlRunQueueSummary {
            run_id: run_id.clone(),
            pending,
            running,
            completed,
            failed,
            cancelled,
            total: pending + running + completed + failed + cancelled,
        })
    }

    /// Work-queue counts for all items associated with a stage execution.
    async fn stage_queue_summary(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<GqlStageQueueSummary> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let stage_id_str = stage_execution_id.as_str();
        let rows = sqlx::query(
            r#"SELECT status, COUNT(*) AS cnt FROM work_items WHERE stage_id = ?1 GROUP BY status"#,
        )
        .bind(stage_id_str)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
        let mut pending = 0i64;
        let mut running = 0i64;
        let mut completed = 0i64;
        let mut failed = 0i64;
        let mut cancelled = 0i64;
        for row in &rows {
            let status: String = row.get("status");
            let cnt: i64 = row.get("cnt");
            match status.as_str() {
                "pending" => pending = cnt,
                "running" => running = cnt,
                "completed" => completed = cnt,
                "failed" => failed = cnt,
                "cancelled" => cancelled = cnt,
                _ => {}
            }
        }
        Ok(GqlStageQueueSummary {
            stage_execution_id: stage_execution_id.clone(),
            pending,
            running,
            completed,
            failed,
            cancelled,
            total: pending + running + completed + failed + cancelled,
        })
    }

    async fn stage(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlStageExecution>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let stage_execution_id: domain::ids::StageExecutionId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        stage_from_projection_or_canonical(pool, stage_execution_id).await
    }

    async fn agent_executions(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<Vec<GqlAgentExecution>> {
        require_operator_read(ctx)?;
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
        require_operator_read(ctx)?;
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
        require_operator_read(ctx)?;
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
        require_operator_read(ctx)?;
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
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = steward_repo::list_recommendations(pool, analysis_id.as_str()).await?;
        Ok(items
            .into_iter()
            .map(GqlStewardRecommendation::from)
            .collect())
    }

    /// P042 §5.2 readback surface. Returns the authoritative
    /// `DaemonStatus` owned by the in-process lifecycle reporter.
    /// Operator-only — matches the `/health` vs `daemonStatus` trust
    /// split: any authenticated operator can read the full typed status,
    /// unauthenticated loopback probes get the JSON snapshot at `/health`.
    async fn daemon_status(&self, ctx: &Context<'_>) -> Result<GqlDaemonStatus> {
        require_operator_read(ctx)?;
        let reporter = ctx.data::<LifecycleReporter>()?;
        Ok(GqlDaemonStatus::from(reporter.snapshot()))
    }

    /// P075: Storage health readback for write pressure, evidence spooling,
    /// units, freshness, thresholds, and kill-switch state.
    async fn storage_health(
        &self,
        ctx: &Context<'_>,
    ) -> Result<crate::types::storage::GqlStorageHealth> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let heartbeat = ctx.data_opt::<Arc<DbWriterHeartbeat>>();
        let json = db::repos::storage_health::storage_health_with_writer(
            pool,
            heartbeat.map(|heartbeat| heartbeat.as_ref()),
        )
        .await?;
        crate::types::storage::GqlStorageHealth::from_storage_health_json(json)
            .map_err(|e| Error::new(e.to_string()))
    }

    /// P066 T17: Latest startup recovery summary including toolchainCache fields.
    /// Returns None when no startup recovery sweep has been recorded yet.
    async fn startup_recovery_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlStartupRecoverySummary>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let readback = db::repos::startup_repairs::latest_startup_recovery_readback(pool).await?;
        Ok(readback.map(GqlStartupRecoverySummary::from))
    }

    /// P066 T18: Latest toolchain cache housekeeping summary.
    /// Returns None before any housekeeping sweep has been recorded.
    async fn toolchain_cache_housekeeping_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlToolchainCacheHousekeepingSummary>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let readback = db::repos::toolchain_cache_housekeeping::latest(pool).await?;
        Ok(readback.map(GqlToolchainCacheHousekeepingSummary::from))
    }

    /// P078: Bounded read-only projection of unresolved side-effect records.
    /// Returns at most `first` records (1-100, default 50).
    /// Read-only: no reconcile, retry, push, upload, or command mutations.
    async fn unresolved_side_effects(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
    ) -> Result<Vec<GqlSideEffectSummary>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let limit = first.unwrap_or(50).clamp(1, 100) as u32;
        let effects = db::repos::side_effects::list_unresolved(pool, limit).await?;
        Ok(effects
            .into_iter()
            .map(GqlSideEffectSummary::from_domain)
            .collect())
    }
}

/// GraphQL wrapper around [`DaemonStatus`] (P042 §5.2). Every field of the
/// domain type is exposed as a first-class GraphQL field: `state`,
/// `degraded`, and `failure` are typed enum/object values so clients can
/// pattern-match terminal reasons without parsing a stringified JSON.
///
/// The `json` field is retained as a convenience for clients that want
/// the canonical snake-case serialization (matching `/health` wire
/// format) without re-serializing the typed fields.
#[derive(SimpleObject, Clone)]
pub struct GqlDaemonStatus {
    pub state: GqlDaemonLifecycleState,
    pub schema_version: i32,
    pub binary_schema_version: i32,
    pub build_sha: String,
    /// ISO-8601 UTC. `None` before the daemon has reached `Ready`.
    pub started_at: Option<String>,
    pub last_state_change_at: String,
    pub restart_count_since_boot: i32,
    pub pid: i32,
    /// Non-empty iff `state == DEGRADED`.
    pub degraded: Vec<GqlDegradedReason>,
    /// Populated iff `state == FAILED` (P042 §4.1 invariant).
    pub failure: Option<GqlFailureReason>,
    /// Xcode MCP broker health when the daemon has mounted the broker pool.
    pub xcode_broker_health: Option<GqlXcodeBrokerHealthSnapshot>,
    /// Canonical JSON per P042 §5.2 (`{state, schema_version, pid,
    /// degraded?, failure?}`). Kept for clients that prefer the
    /// snake-case wire shape identical to `/health`.
    pub json: String,
}

/// GraphQL mirror of [`domain::lifecycle::DaemonLifecycleState`]. Names
/// match the domain enum exactly so the `#[Enum]` mapping round-trips.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlDaemonLifecycleState {
    NotStarted,
    Starting,
    Ready,
    Degraded,
    Restarting,
    Failed,
    Shutdown,
}

impl From<domain::lifecycle::DaemonLifecycleState> for GqlDaemonLifecycleState {
    fn from(s: domain::lifecycle::DaemonLifecycleState) -> Self {
        use domain::lifecycle::DaemonLifecycleState::*;
        match s {
            NotStarted => Self::NotStarted,
            Starting => Self::Starting,
            Ready => Self::Ready,
            Degraded => Self::Degraded,
            Restarting => Self::Restarting,
            Failed => Self::Failed,
            Shutdown => Self::Shutdown,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlDegradedKind {
    BackgroundExecutorStalled,
    AcpRuntimeUnavailable,
    StaleProjection,
    AuthPrincipalTableUnreadable,
    DiskSpaceLow,
}

impl From<domain::lifecycle::DegradedKind> for GqlDegradedKind {
    fn from(k: domain::lifecycle::DegradedKind) -> Self {
        use domain::lifecycle::DegradedKind::*;
        match k {
            BackgroundExecutorStalled => Self::BackgroundExecutorStalled,
            AcpRuntimeUnavailable => Self::AcpRuntimeUnavailable,
            StaleProjection => Self::StaleProjection,
            AuthPrincipalTableUnreadable => Self::AuthPrincipalTableUnreadable,
            DiskSpaceLow => Self::DiskSpaceLow,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlFailureKind {
    MigrationFailed,
    SchemaNewerThanBinary,
    BackupFailed,
    CrashLoopBudgetExhausted,
}

impl From<domain::lifecycle::FailureKind> for GqlFailureKind {
    fn from(k: domain::lifecycle::FailureKind) -> Self {
        use domain::lifecycle::FailureKind::*;
        match k {
            MigrationFailed => Self::MigrationFailed,
            SchemaNewerThanBinary => Self::SchemaNewerThanBinary,
            BackupFailed => Self::BackupFailed,
            CrashLoopBudgetExhausted => Self::CrashLoopBudgetExhausted,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlXcodeBrokerHealthState {
    Disabled,
    Healthy,
    Degraded,
    Failed,
}

impl From<domain::lifecycle::XcodeBrokerHealthState> for GqlXcodeBrokerHealthState {
    fn from(s: domain::lifecycle::XcodeBrokerHealthState) -> Self {
        use domain::lifecycle::XcodeBrokerHealthState::*;
        match s {
            Disabled => Self::Disabled,
            Healthy => Self::Healthy,
            Degraded => Self::Degraded,
            Failed => Self::Failed,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct GqlDegradedReason {
    pub kind: GqlDegradedKind,
    pub detail: String,
    /// ISO-8601 UTC.
    pub since: String,
}

impl From<domain::lifecycle::DegradedReason> for GqlDegradedReason {
    fn from(r: domain::lifecycle::DegradedReason) -> Self {
        Self {
            kind: r.kind.into(),
            detail: r.detail,
            since: r.since.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct GqlFailureReason {
    pub kind: GqlFailureKind,
    pub detail: String,
    /// ISO-8601 UTC.
    pub since: String,
    /// Absolute path of the pre-migration backup when applicable.
    pub backup_path: Option<String>,
}

impl From<domain::lifecycle::FailureReason> for GqlFailureReason {
    fn from(r: domain::lifecycle::FailureReason) -> Self {
        Self {
            kind: r.kind.into(),
            detail: r.detail,
            since: r.since.to_rfc3339(),
            backup_path: r.backup_path,
        }
    }
}

/// Run-level work-queue summary for `runQueueSummary(runId:)`.
#[derive(SimpleObject, Clone)]
pub struct GqlRunQueueSummary {
    pub run_id: ID,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub total: i64,
}

/// Stage-level work-queue summary for `stageQueueSummary(stageExecutionId:)`.
#[derive(SimpleObject, Clone)]
pub struct GqlStageQueueSummary {
    pub stage_execution_id: ID,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub total: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GqlXcodeBrokerHealthSnapshot {
    pub state: GqlXcodeBrokerHealthState,
    pub reason_code: String,
    pub can_acquire_new_xcode_leases: bool,
    pub active_lease_count: i32,
    pub initialize_queue_depth: i32,
    pub last_transition_at: String,
    pub operator_message: String,
    pub pool_id: String,
    pub active_leases: i32,
    pub queued_leases: i32,
    pub max_active_leases: i32,
    pub max_queued_leases: i32,
    pub broker_disabled: bool,
    pub backend_available: bool,
    pub observation_persistence_failures: i32,
    pub stale_lease_count: i32,
    pub backend_session_count: i32,
    pub helper_cleanup_reaped_leases_total: i32,
}

impl From<domain::lifecycle::XcodeBrokerHealthSnapshot> for GqlXcodeBrokerHealthSnapshot {
    fn from(s: domain::lifecycle::XcodeBrokerHealthSnapshot) -> Self {
        Self {
            state: s.state.into(),
            reason_code: s.reason_code,
            can_acquire_new_xcode_leases: s.can_acquire_new_xcode_leases,
            active_lease_count: s.active_lease_count as i32,
            initialize_queue_depth: s.initialize_queue_depth as i32,
            last_transition_at: s.last_transition_at,
            operator_message: s.operator_message,
            pool_id: s.pool_id,
            active_leases: s.active_leases as i32,
            queued_leases: s.queued_leases as i32,
            max_active_leases: s.max_active_leases as i32,
            max_queued_leases: s.max_queued_leases as i32,
            broker_disabled: s.broker_disabled,
            backend_available: s.backend_available,
            observation_persistence_failures: s.observation_persistence_failures as i32,
            stale_lease_count: s.stale_lease_count as i32,
            backend_session_count: s.backend_session_count as i32,
            helper_cleanup_reaped_leases_total: s.helper_cleanup_reaped_leases_total as i32,
        }
    }
}

impl From<DaemonStatus> for GqlDaemonStatus {
    fn from(s: DaemonStatus) -> Self {
        let json = serde_json::to_string(&s).unwrap_or_else(|_| "{}".to_string());
        Self {
            state: s.state.into(),
            schema_version: s.schema_version as i32,
            binary_schema_version: s.binary_schema_version as i32,
            build_sha: s.build_sha,
            started_at: s.started_at.map(|t| t.to_rfc3339()),
            last_state_change_at: s.last_state_change_at.to_rfc3339(),
            restart_count_since_boot: s.restart_count_since_boot as i32,
            pid: s.pid as i32,
            degraded: s
                .degraded
                .into_iter()
                .map(GqlDegradedReason::from)
                .collect(),
            failure: s.failure.map(GqlFailureReason::from),
            xcode_broker_health: s
                .xcode_broker_health
                .map(GqlXcodeBrokerHealthSnapshot::from),
            json,
        }
    }
}

/// P078: Read-only projection of a single unresolved side-effect record.
/// Exposes raw kind/status strings for forward-compatible clients.
/// No mutation fields.
#[derive(SimpleObject, Clone)]
pub struct GqlSideEffectSummary {
    pub id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    /// Decoded effect kind (e.g. "git_commit"). Use effect_kind_raw for unknown values.
    pub effect_kind: String,
    /// Raw effect kind string for forward-compatible clients.
    pub effect_kind_raw: String,
    /// Decoded status string. Use status_raw for unknown values.
    pub status: String,
    /// Raw status string for forward-compatible clients.
    pub status_raw: String,
    pub target_key: String,
    pub external_write_attempted: bool,
    pub last_error_kind: Option<String>,
    pub expected_evidence_json: Option<Json<serde_json::Value>>,
    pub observed_evidence_summary_json: Option<Json<serde_json::Value>>,
    pub evidence_root: Option<String>,
    pub readback_source: String,
    pub report_path: Option<String>,
    pub blocked_reason: String,
    pub operator_next_action: String,
    pub recommended_mcp_tool: String,
    pub retry_forbidden: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl GqlSideEffectSummary {
    pub fn from_domain(e: domain::side_effect::SideEffect) -> Self {
        let kind_str = e.effect_kind.to_string();
        let status_str = e.status.to_string();
        let expected_evidence_json = parse_optional_json(&e.expected_evidence_json);
        let observed_evidence_summary_json = parse_optional_json(&e.observed_evidence_summary_json);
        let report_path = observed_evidence_summary_json
            .as_ref()
            .and_then(|json| json.0.get("manifest_path"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let operator_next_action = side_effect_operator_next_action(&e.status);
        Self {
            id: e.id.to_string(),
            run_id: e.run_id.to_string(),
            stage_execution_id: e.stage_execution_id.to_string(),
            effect_kind: kind_str.clone(),
            effect_kind_raw: kind_str,
            status: status_str.clone(),
            status_raw: status_str,
            target_key: e.target_key,
            external_write_attempted: e.external_write_attempted,
            last_error_kind: e.last_error_kind,
            expected_evidence_json,
            observed_evidence_summary_json,
            evidence_root: e.evidence_root,
            readback_source: "side_effects_ledger".into(),
            report_path,
            blocked_reason: side_effect_blocked_reason(&e.status),
            operator_next_action: operator_next_action.clone(),
            recommended_mcp_tool: operator_next_action,
            retry_forbidden: true,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

fn parse_optional_json(raw: &Option<String>) -> Option<Json<serde_json::Value>> {
    raw.as_ref().map(|value| {
        Json(
            serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.clone())),
        )
    })
}

fn side_effect_blocked_reason(status: &domain::side_effect::SideEffectStatus) -> String {
    match status {
        domain::side_effect::SideEffectStatus::Prepared => "prepared_effect_not_executed",
        domain::side_effect::SideEffectStatus::Executing => "executing_effect_not_settled",
        domain::side_effect::SideEffectStatus::ExternallyObserved => {
            "external_write_observed_pending_settlement"
        }
        domain::side_effect::SideEffectStatus::NeedsReconciliation => "effect_needs_reconciliation",
        domain::side_effect::SideEffectStatus::Conflict => "effect_conflict_requires_disposition",
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effect_unrecoverable_requires_manual_clear"
        }
        _ => "not_blocking",
    }
    .to_string()
}

fn side_effect_operator_next_action(status: &domain::side_effect::SideEffectStatus) -> String {
    match status {
        domain::side_effect::SideEffectStatus::NeedsReconciliation
        | domain::side_effect::SideEffectStatus::ExternallyObserved => "effects.reconcile",
        domain::side_effect::SideEffectStatus::Conflict => {
            "effects.mark_unrecoverable or effects.clear_after_manual_verification"
        }
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effects.clear_after_manual_verification"
        }
        _ => "effects.inspect",
    }
    .to_string()
}

async fn runs_with_latest_summaries(pool: &SqlitePool, runs: Vec<GqlRun>) -> Result<Vec<GqlRun>> {
    let mut with_summaries = Vec::with_capacity(runs.len());
    for run in runs {
        with_summaries.push(run_with_latest_summary(pool, run).await?);
    }
    Ok(with_summaries)
}

async fn run_with_latest_summary(pool: &SqlitePool, mut run: GqlRun) -> Result<GqlRun> {
    let run_id: RunId = run
        .id
        .as_str()
        .parse()
        .map_err(|error: uuid::Error| Error::new(error.to_string()))?;
    run.implementation_self_assessment_summary =
        artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
            .await?
            .map(|stored| stored.summary.into());
    let code_writer_completion_readbacks =
        code_writer_completion_receipts::list_by_run(pool, run_id).await?;
    let canonical_code_writer_completion_readbacks =
        code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
    run.implementation_completion =
        domain::code_writer_completion::project_implementation_completion(
            &canonical_code_writer_completion_readbacks,
        )
        .into();
    run.code_writer_completion_receipts = code_writer_completion_readbacks
        .into_iter()
        .map(Into::into)
        .collect();
    run.workflow_conflict = workflow_conflicts::get_current_blocking_conflict(pool, run_id)
        .await?
        .map(Into::into);
    // P017: Enrich workflow conflict with lead mediation readback if present.
    // API-001 (P017 R2 audit): the enriched projection includes
    // mediation-owned `execution_attempts` so operators can inspect the
    // mediation's runtime facts, watchdog outcome, artifacts, and
    // provider/timing details directly through the conflict surface.
    if let Some(ref mut conflict) = run.workflow_conflict {
        if let Some(ref mediation_id) = conflict.mediation_record_id {
            if let Ok(Some(med)) =
                db::repos::lead_conflict_mediations::find_by_id(pool, mediation_id).await
            {
                conflict.lead_mediation = Some(
                    crate::types::run::GqlLeadMediation::build_with_attempts(pool, &med).await?,
                );
            }
        }
    }
    run.implementation_handoff_status_json = if let Some(status) =
        workflow_conflicts::get_implementation_handoff_status(pool, run_id).await?
    {
        Some(async_graphql::Json(serde_json::to_value(status)?))
    } else {
        None
    };
    // P077: Populate closeout readiness summary via CloseoutReadinessSummaryAccessor.
    let run_id_str = run_id.to_string();
    if let Some(summary) = closeout::load_closeout_readiness_summary(pool, &run_id_str).await? {
        let summary_json = async_graphql::Json(serde_json::to_value(&summary)?);
        run.closeout_readiness_summary_json = Some(summary_json.clone());
        run.implementation_closeout_readiness_summary = Some(summary_json);
    }
    run.side_effect_readback_json = Some(Json(side_effect_readback_json(pool, run_id).await?));
    Ok(run)
}

async fn side_effect_readback_json(pool: &SqlitePool, run_id: RunId) -> Result<serde_json::Value> {
    let unresolved =
        db::repos::side_effects::list_unresolved_for_run(pool, &run_id.to_string()).await?;
    let effects: Vec<serde_json::Value> = unresolved
        .iter()
        .map(|effect| {
            let observed = effect
                .observed_evidence_summary_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
            let report_path = observed
                .as_ref()
                .and_then(|value| value.get("manifest_path"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            serde_json::json!({
                "id": effect.id.to_string(),
                "run_id": effect.run_id.to_string(),
                "stage_execution_id": effect.stage_execution_id.to_string(),
                "agent_execution_id": effect.agent_execution_id.as_ref().map(|id| id.to_string()),
                "effect_kind": effect.effect_kind.to_string(),
                "status": effect.status.to_string(),
                "target_key": effect.target_key,
                "external_write_attempted": effect.external_write_attempted,
                "evidence_root": effect.evidence_root.clone(),
                "readback_source": "side_effects_ledger",
                "report_path": report_path,
                "blocked_reason": side_effect_blocked_reason(&effect.status),
                "operator_next_action": side_effect_operator_next_action(&effect.status),
                "recommended_mcp_tool": side_effect_operator_next_action(&effect.status),
                "retry_forbidden": true,
                "last_error_kind": effect.last_error_kind.clone(),
                "updated_at": effect.updated_at.to_rfc3339()
            })
        })
        .collect();
    Ok(serde_json::json!({
        "schema_version": "p078_side_effect_readback_v1",
        "run_id": run_id.to_string(),
        "unresolved_count": effects.len(),
        "blocked": !effects.is_empty(),
        "readback_source": "side_effects_ledger",
        "effects": effects
    }))
}

async fn proposal_064_command_readback(
    pool: &SqlitePool,
    run_id: RunId,
    command_types: &[&str],
) -> Result<serde_json::Value> {
    let placeholders = command_types
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, command_type, result_status, created_at, completed_at, caller_surface, caller_principal_id, caller_tool \
         FROM command_journal \
         WHERE run_id = ? AND command_type IN ({placeholders}) \
         ORDER BY created_at DESC LIMIT 8"
    );
    let mut query = sqlx::query(&sql).bind(run_id.to_string());
    for command_type in command_types {
        query = query.bind(*command_type);
    }
    let rows = query.fetch_all(pool).await?;
    let commands = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.try_get::<String, _>("id").ok(),
                "command_type": row.try_get::<String, _>("command_type").ok(),
                "result_status": row.try_get::<String, _>("result_status").ok(),
                "created_at": row.try_get::<String, _>("created_at").ok(),
                "completed_at": row.try_get::<Option<String>, _>("completed_at").ok().flatten(),
                "caller_surface": row.try_get::<Option<String>, _>("caller_surface").ok().flatten(),
                "caller_principal_id": row.try_get::<Option<String>, _>("caller_principal_id").ok().flatten(),
                "caller_tool": row.try_get::<Option<String>, _>("caller_tool").ok().flatten(),
            })
        })
        .collect::<Vec<_>>();
    let pending = commands
        .iter()
        .filter(|command| command["result_status"] == "pending")
        .cloned()
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "latest_commands": commands,
        "pending_commands": pending,
    }))
}

async fn proposal_064_main_sync_readback(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let latest_attempt = sqlx::query(
        "SELECT id, idempotency_key, trigger_reason, status, barrier_id, conflict_count, resolver_work_item_id, error_message, requested_by_stage_id, requested_by_work_item_id, created_at, started_at, completed_at \
         FROM main_sync_attempts WHERE run_id = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let barrier = sqlx::query(
        "SELECT id, owner_id, owner_kind, status, reason, acquired_at, heartbeat_at, expires_at, released_at \
         FROM worktree_mutation_barriers WHERE run_id = ? AND status IN ('pending', 'active') ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let active_consumers = sqlx::query(
        "SELECT id, worktree_resource_key, owner_id, worktree_access_mode, owner_kind, reason, acquired_at, expires_at, heartbeat_at \
         FROM background_leases WHERE run_id = ? AND worktree_resource_key IS NOT NULL AND released_at IS NULL ORDER BY acquired_at DESC LIMIT 16",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let command_readback = proposal_064_command_readback(
        pool,
        run_id,
        &[
            "MainSyncRequest",
            "MainSyncRetry",
            "MainSyncSetRunOverride",
            "MainSyncRepairState",
            "MainSyncRecordRecoveryDecision",
        ],
    )
    .await?;

    Ok(serde_json::json!({
        "schema_version": "p064_main_sync_readback_v1",
        "mode": "off",
        "operator_tools_enabled": false,
        "latest_attempt": latest_attempt.map(|row| serde_json::json!({
            "id": row.try_get::<String, _>("id").ok(),
            "idempotency_key": row.try_get::<String, _>("idempotency_key").ok(),
            "trigger_reason": row.try_get::<String, _>("trigger_reason").ok(),
            "status": row.try_get::<String, _>("status").ok(),
            "barrier_id": row.try_get::<Option<String>, _>("barrier_id").ok().flatten(),
            "conflict_count": row.try_get::<Option<i64>, _>("conflict_count").ok().flatten(),
            "resolver_work_item_id": row.try_get::<Option<String>, _>("resolver_work_item_id").ok().flatten(),
            "error_message": row.try_get::<Option<String>, _>("error_message").ok().flatten(),
            "requested_by_stage_id": row.try_get::<Option<String>, _>("requested_by_stage_id").ok().flatten(),
            "requested_by_work_item_id": row.try_get::<Option<String>, _>("requested_by_work_item_id").ok().flatten(),
            "created_at": row.try_get::<String, _>("created_at").ok(),
            "started_at": row.try_get::<Option<String>, _>("started_at").ok().flatten(),
            "completed_at": row.try_get::<Option<String>, _>("completed_at").ok().flatten(),
        })),
        "active_barrier": barrier.map(|row| serde_json::json!({
            "id": row.try_get::<String, _>("id").ok(),
            "owner_id": row.try_get::<String, _>("owner_id").ok(),
            "owner_kind": row.try_get::<String, _>("owner_kind").ok(),
            "status": row.try_get::<String, _>("status").ok(),
            "reason": row.try_get::<String, _>("reason").ok(),
            "acquired_at": row.try_get::<Option<String>, _>("acquired_at").ok().flatten(),
            "heartbeat_at": row.try_get::<Option<String>, _>("heartbeat_at").ok().flatten(),
            "expires_at": row.try_get::<String, _>("expires_at").ok(),
            "released_at": row.try_get::<Option<String>, _>("released_at").ok().flatten(),
        })),
        "active_consumers": active_consumers.into_iter().map(|row| serde_json::json!({
            "lease_id": row.try_get::<String, _>("id").ok(),
            "resource_key": row.try_get::<String, _>("worktree_resource_key").ok(),
            "owner_id": row.try_get::<String, _>("owner_id").ok(),
            "access_mode": row.try_get::<Option<String>, _>("worktree_access_mode").ok().flatten(),
            "owner_kind": row.try_get::<Option<String>, _>("owner_kind").ok().flatten(),
            "reason": row.try_get::<Option<String>, _>("reason").ok().flatten(),
            "acquired_at": row.try_get::<String, _>("acquired_at").ok(),
            "expires_at": row.try_get::<String, _>("expires_at").ok(),
            "heartbeat_at": row.try_get::<Option<String>, _>("heartbeat_at").ok().flatten(),
        })).collect::<Vec<_>>(),
        "commands": command_readback,
    }))
}

async fn proposal_064_knowledge_capsule_readback(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let attachments = sqlx::query(
        "SELECT a.id, a.capsule_id, a.match_rule, a.attachment_reason, a.injected, a.injected_byte_count, a.injected_token_count, a.truncated, a.stale_main, a.ignored, a.ignored_reason, a.created_at, c.source_run_id, c.source_proposal_id, c.source_status, c.status AS capsule_status \
         FROM run_knowledge_capsule_attachments a \
         JOIN run_knowledge_capsules c ON c.id = a.capsule_id \
         WHERE a.target_run_id = ? ORDER BY a.created_at DESC LIMIT 16",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let command_readback =
        proposal_064_command_readback(pool, run_id, &["KnowledgeCapsuleIgnore"]).await?;

    Ok(serde_json::json!({
        "schema_version": "p064_knowledge_capsule_readback_v1",
        "mode": "off",
        "operator_tools_enabled": false,
        "attached_capsules": attachments.into_iter().map(|row| serde_json::json!({
            "attachment_id": row.try_get::<String, _>("id").ok(),
            "capsule_id": row.try_get::<String, _>("capsule_id").ok(),
            "source_run_id": row.try_get::<String, _>("source_run_id").ok(),
            "source_proposal_id": row.try_get::<Option<String>, _>("source_proposal_id").ok().flatten(),
            "source_status": row.try_get::<String, _>("source_status").ok(),
            "capsule_status": row.try_get::<String, _>("capsule_status").ok(),
            "match_rule": row.try_get::<String, _>("match_rule").ok(),
            "attachment_reason": row.try_get::<String, _>("attachment_reason").ok(),
            "injected": row.try_get::<i64, _>("injected").unwrap_or_default() != 0,
            "injected_byte_count": row.try_get::<Option<i64>, _>("injected_byte_count").ok().flatten(),
            "injected_token_count": row.try_get::<Option<i64>, _>("injected_token_count").ok().flatten(),
            "truncated": row.try_get::<i64, _>("truncated").unwrap_or_default() != 0,
            "stale_main": row.try_get::<i64, _>("stale_main").unwrap_or_default() != 0,
            "ignored": row.try_get::<i64, _>("ignored").unwrap_or_default() != 0,
            "ignored_reason": row.try_get::<Option<String>, _>("ignored_reason").ok().flatten(),
            "created_at": row.try_get::<String, _>("created_at").ok(),
        })).collect::<Vec<_>>(),
        "commands": command_readback,
    }))
}

const P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES: usize = 120_000;
const P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES: usize = 1_000_000;

struct P031ArtifactPayloadPreview {
    text: String,
    truncated: bool,
    bytes_read: usize,
}

fn attach_p031_artifact_payload(
    row: &db::repos::projections::ArtifactIndexRow,
    run: Option<&domain::run::Run>,
    artifact: &mut GqlArtifact,
    bulk_preview_budget_remaining: &mut usize,
) {
    attach_p031_artifact_payload_from_metadata(
        &row.format,
        row.report_kind.as_deref(),
        row.size_bytes,
        &row.file_path,
        run,
        artifact,
        bulk_preview_budget_remaining,
    );
}

fn attach_p031_artifact_payload_from_metadata(
    format: &str,
    report_kind: Option<&str>,
    size_bytes: Option<i64>,
    file_path: &str,
    run: Option<&domain::run::Run>,
    artifact: &mut GqlArtifact,
    bulk_preview_budget_remaining: &mut usize,
) {
    if report_kind.is_some() || format == "report" {
        return;
    }

    let estimated_preview_bytes = size_bytes
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| *size > 0)
        .map(|size| size.min(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES))
        .unwrap_or(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
    if estimated_preview_bytes > *bulk_preview_budget_remaining {
        warn!(
            artifact_id = %artifact.id.as_str(),
            estimated_preview_bytes,
            bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
            "P031 artifact payload deferred before read: preview budget exhausted"
        );
        mark_payload_deferred(
            artifact,
            "Artifact payload preview deferred because the bulk artifact list reached its payload preview budget",
        );
        return;
    }

    let Some(run) = run else {
        warn!(
            artifact_id = %artifact.id.as_str(),
            "P031 artifact payload unavailable: missing run metadata"
        );
        mark_payload_unavailable(
            artifact,
            "Run metadata was unavailable for artifact readback",
        );
        return;
    };

    let Some(path) = resolve_server_owned_artifact_path(file_path, run) else {
        warn!(
            artifact_id = %artifact.id.as_str(),
            "P031 artifact payload unavailable: path outside run-owned roots"
        );
        mark_payload_unavailable(
            artifact,
            "Artifact path is outside the selected run's server-owned roots",
        );
        return;
    };

    match read_p031_artifact_payload_preview(&path) {
        Ok(preview) => {
            let consumed_preview_bytes = estimated_preview_bytes.max(
                preview
                    .bytes_read
                    .min(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES),
            );
            if consumed_preview_bytes > *bulk_preview_budget_remaining {
                warn!(
                    artifact_id = %artifact.id.as_str(),
                    consumed_preview_bytes,
                    bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
                    "P031 artifact payload deferred after read: preview budget exhausted"
                );
                mark_payload_deferred(
                    artifact,
                    "Artifact payload preview deferred because the bulk artifact list reached its payload preview budget",
                );
                return;
            }
            *bulk_preview_budget_remaining =
                bulk_preview_budget_remaining.saturating_sub(consumed_preview_bytes);
            artifact.payload_text = Some(preview.text);
            artifact.payload_availability_state = GqlPayloadAvailabilityState::Available;
            artifact.payload_unavailable_reason_code = None;
            artifact.server_debug_detail = preview.truncated.then(|| {
                format!(
                    "Artifact payload preview capped at {} bytes; full payload remains server-owned",
                    P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES
                )
            });
            debug!(
                artifact_id = %artifact.id.as_str(),
                consumed_preview_bytes,
                bytes_read = preview.bytes_read,
                truncated = preview.truncated,
                bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
                "P031 artifact payload preview attached"
            );
        }
        Err(err) => {
            warn!(
                artifact_id = %artifact.id.as_str(),
                error = %err,
                "P031 artifact payload readback failed"
            );
            mark_payload_unavailable(
                artifact,
                &format!("Artifact payload readback failed: {err}"),
            );
        }
    }
}

fn read_p031_artifact_payload_preview(path: &Path) -> io::Result<P031ArtifactPayloadPreview> {
    let file = std::fs::File::open(path)?;
    let mut limited = file.take((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES + 1) as u64);
    let mut bytes = Vec::with_capacity(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES + 1);
    limited.read_to_end(&mut bytes)?;

    let truncated = bytes.len() > P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES;
    if truncated {
        bytes.truncate(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
    }
    let bytes_read = bytes.len();

    match String::from_utf8(bytes) {
        Ok(text) => Ok(P031ArtifactPayloadPreview {
            text,
            truncated,
            bytes_read,
        }),
        Err(err) => {
            let valid_up_to = err.utf8_error().valid_up_to();
            let mut bytes = err.into_bytes();
            bytes.truncate(valid_up_to);
            let text = String::from_utf8(bytes).map_err(|utf8_err| {
                io::Error::new(io::ErrorKind::InvalidData, utf8_err.to_string())
            })?;
            Ok(P031ArtifactPayloadPreview {
                text,
                truncated: true,
                bytes_read,
            })
        }
    }
}

fn mark_payload_unavailable(artifact: &mut GqlArtifact, detail: &str) {
    artifact.payload_text = None;
    artifact.payload_availability_state = GqlPayloadAvailabilityState::Unavailable;
    artifact.payload_unavailable_reason_code = Some(GqlPayloadUnavailableReasonCode::NotAvailable);
    artifact.server_debug_detail = Some(detail.to_string());
}

fn mark_payload_deferred(artifact: &mut GqlArtifact, detail: &str) {
    artifact.payload_text = None;
    artifact.payload_availability_state = GqlPayloadAvailabilityState::PayloadDeferred;
    artifact.payload_unavailable_reason_code =
        Some(GqlPayloadUnavailableReasonCode::PayloadDeferredByP031);
    artifact.server_debug_detail = Some(format!("{detail}. {P085_NO_DEADLINE_JUSTIFICATION}"));
}

fn resolve_server_owned_artifact_path(file_path: &str, run: &domain::run::Run) -> Option<PathBuf> {
    let raw_path = PathBuf::from(file_path);
    let candidate = if raw_path.is_absolute() {
        raw_path
    } else if !run.artifact_root.is_empty() {
        PathBuf::from(&run.artifact_root).join(raw_path)
    } else {
        PathBuf::from(&run.workspace_root).join(raw_path)
    };
    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;
    let allowed_roots = [
        Some(run.artifact_root.as_str()),
        Some(run.workspace_root.as_str()),
        run.chainworks_meta_root.as_deref(),
    ];
    allowed_roots
        .into_iter()
        .flatten()
        .filter(|root| !root.is_empty())
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| path_is_inside(&canonical_candidate, &root))
        .then_some(canonical_candidate)
}

fn path_is_inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

pub struct MutationRoot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationName {
    /// P072: Converged approval mutation by approval_id.
    ApproveApproval,
    /// P072: Converged rejection mutation by approval_id.
    RejectApproval,
}

impl MutationName {
    fn graphql_name(self) -> &'static str {
        match self {
            MutationName::ApproveApproval => "approveApproval",
            MutationName::RejectApproval => "rejectApproval",
        }
    }
}

pub fn capability_id_for(mutation: MutationName) -> domain::CapabilityToolId {
    match mutation {
        MutationName::ApproveApproval | MutationName::RejectApproval => {
            domain::CapabilityToolId::ApprovalsResolve
        }
    }
}

fn mutation_allowed(
    ctx: &Context<'_>,
    principal: &auth::Principal,
    mutation: MutationName,
) -> bool {
    if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
        if let Some(allowed) = auth::is_mutation_allowed_by_surface_policy(
            table,
            &principal.id,
            mutation.graphql_name(),
        ) {
            return allowed && principal.class == auth::PrincipalClass::Operator;
        }
        if auth::find_principal_by_id(table, &principal.id).is_some() {
            return false;
        }
    }

    auth::filter_tools(principal, &[capability_id_for(mutation)]).len() == 1
}

/// Build the GraphQL caller context for a mutation and attach the
/// `X-Request-ID` from the async-graphql request data (P042 §9.3) when
/// the outer axum middleware injected one. The command journal INSERT
/// picks it up transparently via `CallerContext.request_id`.
fn graphql_caller_with_request_id(
    ctx: &Context<'_>,
    principal: &auth::Principal,
    mutation_name: &str,
) -> CallerContext {
    let mut caller = CallerContext::graphql(&principal.id, &principal.class, mutation_name);
    if let Ok(rid) = ctx.data::<crate::request_id::RequestId>() {
        caller = caller.with_request_id(&rid.0);
    }
    caller
}

// ── P029 payload wrappers ──────────────────────────────────────────────
// Dedicated types for each mutation so journal_id doesn't pollute shared
// Run/Approval types used by read queries.

/// P072: Payload for approveApproval mutation.
#[derive(SimpleObject)]
pub struct ApproveApprovalPayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
    pub conflict_result_code: Option<GqlMutationConflictResultCode>,
}

/// P072: Payload for rejectApproval mutation.
#[derive(SimpleObject)]
pub struct RejectApprovalPayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
    pub conflict_result_code: Option<GqlMutationConflictResultCode>,
}

fn approval_resolution_conflict_code(
    error: &anyhow::Error,
) -> Option<(ID, GqlMutationConflictResultCode)> {
    let conflict = error.downcast_ref::<ApprovalResolutionConflict>()?;
    match conflict {
        ApprovalResolutionConflict::AlreadyResolved { .. } => Some((
            ID::from(conflict.journal_id().to_owned()),
            GqlMutationConflictResultCode::AlreadyResolved,
        )),
    }
}

#[Object]
impl MutationRoot {
    /// P072: Approve a stage approval by approval_id. The resolver
    /// server-resolves run_id and stage_id from the approval record
    /// before constructing ResolveApprovalCmd.
    async fn approve_approval(
        &self,
        ctx: &Context<'_>,
        approval_id: ID,
        comment: Option<String>,
    ) -> Result<ApproveApprovalPayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(ctx, &principal, MutationName::ApproveApproval) {
            return Err(Error::new("forbidden"));
        }

        let caller = graphql_caller_with_request_id(ctx, &principal, "approveApproval");

        let aid: domain::ids::ApprovalId = approval_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        // Server-resolve run_id and stage_id from the approval record.
        let approval = approvals::find_by_id(pool, aid)
            .await?
            .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;

        let cmd = Command::ResolveApproval(ResolveApprovalCmd {
            approval_id: aid,
            decision: ApprovalResolutionDecision::Approved,
            rationale: comment,
            run_id: approval.run_id,
            stage_id: approval.stage_id.clone(),
        });

        let result = cmd_handler.handle(cmd, caller).await;
        match result {
            Ok(commanded) => {
                let jid = ID::from(commanded.journal_id);
                // Re-fetch for authoritative readback.
                let updated = approvals::find_by_id(pool, aid)
                    .await?
                    .ok_or_else(|| Error::new("Approval not found after update"))?;
                Ok(ApproveApprovalPayload {
                    approval: GqlApproval::from(updated),
                    journal_id: jid,
                    conflict_result_code: None,
                })
            }
            Err(e) => {
                if let Some((journal_id, conflict_result_code)) =
                    approval_resolution_conflict_code(&e)
                {
                    let current = approvals::find_by_id(pool, aid)
                        .await?
                        .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;
                    Ok(ApproveApprovalPayload {
                        approval: GqlApproval::from(current),
                        journal_id,
                        conflict_result_code: Some(conflict_result_code),
                    })
                } else {
                    Err(Error::new(e.to_string()))
                }
            }
        }
    }

    /// P072: Reject a stage approval by approval_id with a required reason.
    async fn reject_approval(
        &self,
        ctx: &Context<'_>,
        approval_id: ID,
        reason: String,
    ) -> Result<RejectApprovalPayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(ctx, &principal, MutationName::RejectApproval) {
            return Err(Error::new("forbidden"));
        }

        let caller = graphql_caller_with_request_id(ctx, &principal, "rejectApproval");

        let aid: domain::ids::ApprovalId = approval_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        // Server-resolve run_id and stage_id from the approval record.
        let approval = approvals::find_by_id(pool, aid)
            .await?
            .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;

        let cmd = Command::ResolveApproval(ResolveApprovalCmd {
            approval_id: aid,
            decision: ApprovalResolutionDecision::Rejected,
            rationale: Some(reason),
            run_id: approval.run_id,
            stage_id: approval.stage_id.clone(),
        });

        let result = cmd_handler.handle(cmd, caller).await;
        match result {
            Ok(commanded) => {
                let jid = ID::from(commanded.journal_id);
                let updated = approvals::find_by_id(pool, aid)
                    .await?
                    .ok_or_else(|| Error::new("Approval not found after update"))?;
                Ok(RejectApprovalPayload {
                    approval: GqlApproval::from(updated),
                    journal_id: jid,
                    conflict_result_code: None,
                })
            }
            Err(e) => {
                if let Some((journal_id, conflict_result_code)) =
                    approval_resolution_conflict_code(&e)
                {
                    let current = approvals::find_by_id(pool, aid)
                        .await?
                        .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;
                    Ok(RejectApprovalPayload {
                        approval: GqlApproval::from(current),
                        journal_id,
                        conflict_result_code: Some(conflict_result_code),
                    })
                } else {
                    Err(Error::new(e.to_string()))
                }
            }
        }
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
        require_operator_read(ctx)?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                let refresh_run_id = match event {
                    DomainEvent::RunStatusChanged { run_id, .. }
                    | DomainEvent::RunStarted { run_id, .. }
                    | DomainEvent::StageStatusChanged { run_id, .. }
                    | DomainEvent::ApprovalRequested { run_id, .. }
                    | DomainEvent::ArtifactCreated { run_id, .. }
                    | DomainEvent::RuntimeStatusChanged { run_id, .. }
                    | DomainEvent::MediationConfirmationResolved { run_id, .. }
                    | DomainEvent::RoutingCompleted { run_id, .. } => Some(run_id),
                    DomainEvent::ApprovalResolved { approval_id, .. } => {
                        approvals::find_by_id(&pool, approval_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|approval| approval.run_id)
                    }
                    DomainEvent::SchedulerBackpressureChanged { run_id, .. } => {
                        run_id.and_then(|id| id.parse().ok())
                    }
                    DomainEvent::DaemonStatusChanged { .. } => None,
                }?;
                if let Some(fid) = filter_run_id {
                    if refresh_run_id != fid {
                        return None;
                    }
                }
                match run_from_projection_or_canonical(&pool, refresh_run_id).await {
                    Ok(run) => Some(Ok(run)),
                    Err(err) => Some(Err(err)),
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
        require_operator_read(ctx)?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

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
                        match stage_from_projection_or_canonical(&pool, stage_execution_id).await {
                            Ok(stage) => Some(Ok(stage)),
                            Err(err) => Some(Err(err)),
                        }
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
        require_operator_read(ctx)?;

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

    async fn approval_resolved(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>>> {
        require_operator_read(ctx)?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalResolved { approval_id, .. } => {
                        let approval = approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
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
        require_operator_read(ctx)?;

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

    /// P042 §5.2 push surface. Emits a `GqlDaemonStatus` frame on every
    /// lifecycle transition (driven by the same EventBus the reporter
    /// broadcasts into). Clients typically call `daemonStatus` once at
    /// connect time to seed state, then subscribe here to stay in sync.
    ///
    /// Operator-only per P042 §5.2 readback-surfaces table. A principal
    /// of any other class receives `unauthorized`; the check runs
    /// before `events.subscribe()` so a non-operator never even sees the
    /// first frame.
    async fn daemon_status_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<GqlDaemonStatus>>> {
        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized: no principal in subscription context"))?;
        if principal.class != auth::PrincipalClass::Operator {
            return Err(Error::new("forbidden"));
        }
        // P072: enforce allow_subscriptions surface policy when present.
        if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
            if let Some(allowed) =
                auth::is_subscription_allowed_by_surface_policy(table, &principal.id)
            {
                if !allowed {
                    return Err(Error::new("forbidden"));
                }
            }
        }
        let events = ctx.data::<EventSender>()?.clone();
        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| async move {
            let event = msg.ok()?;
            match event {
                DomainEvent::DaemonStatusChanged { status } => {
                    Some(Ok(GqlDaemonStatus::from(status)))
                }
                _ => None,
            }
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
    use db::repos::{
        artifact_contracts, artifacts, ideas, projections, rollout_contract_checks, runs, stages,
        steward, workflow_conflicts,
    };
    use db::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::mediation::{LeadConflictMediationRecord, LeadMediationStatus};
    use domain::steward::{
        CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
        StewardRecommendation,
    };
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use domain::workflow_conflict::{
        candidate_transition_hash, workflow_conflict_fingerprint, CandidateTransitionEvaluation,
        CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
        WorkflowConflictStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// Shared in-process `LifecycleReporter` for tests. Every `build_schema`
    /// call now requires a reporter per P042 §5.2; tests get a default one
    /// seeded in `NotStarted` unless they need a specific transition.
    fn test_reporter() -> LifecycleReporter {
        LifecycleReporter::new(0, "test", event_bus::new_bus(16))
    }

    const P041_FIXTURES: &[&str] = &[
        "proposal-loop-basic",
        "implementation-refine-review",
        "approval-pause-resume",
        "retry-recovery-flow",
        "cancelled-or-blocked-run",
        "terminal-report-evidence",
        "projection-readback-surface",
    ];

    fn p041_selected_fixtures() -> Vec<&'static str> {
        match std::env::var("P041_ONLY_FIXTURE") {
            Ok(raw) if !raw.trim().is_empty() => {
                let requested = raw.trim().to_string();
                let fixture = P041_FIXTURES
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == requested.as_str())
                    .unwrap_or_else(|| {
                        panic!("P041_ONLY_FIXTURE {requested:?} is not in P041_FIXTURES")
                    });
                vec![fixture]
            }
            _ => P041_FIXTURES.to_vec(),
        }
    }

    #[test]
    fn mutation_name_converter_covers_approval_mutations() {
        assert_eq!(
            capability_id_for(MutationName::ApproveApproval),
            domain::CapabilityToolId::ApprovalsResolve
        );
        assert_eq!(
            capability_id_for(MutationName::RejectApproval),
            domain::CapabilityToolId::ApprovalsResolve
        );
    }

    #[tokio::test]
    async fn graphql_mutation_root_exposes_only_approval_mutations() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let sdl = schema.sdl();

        assert!(sdl.contains("approveApproval("));
        assert!(sdl.contains("rejectApproval("));
        for mutation in [
            "startRun(",
            "approveStage(",
            "rejectStage(",
            "retryStage(",
            "overrideLegacyDiscoveryPolicy(",
            "cancelRun(",
        ] {
            assert!(
                !sdl.contains(mutation),
                "{mutation} must not be present on GraphQL MutationRoot"
            );
        }
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
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    async fn persist_rollout_contract_readback(pool: &SqlitePool, run_id: RunId) {
        use rollout_contract_checks::{
            ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
            RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
        };

        let now = Utc::now();
        rollout_contract_checks::upsert_rollout_contract_check(
            pool,
            &UpsertRolloutContractCheck {
                id: uuid::Uuid::new_v4(),
                run_id: run_id.inner(),
                proposal_id: "proposal-084".into(),
                proposal_revision_id: "p084-r5".into(),
                proposal_content_hash: "sha256:proposal".into(),
                contract_object_hash: "sha256:contract".into(),
                content_snapshot_id: "snapshot-1".into(),
                checker_version: "p084-lint-1".into(),
                status: RolloutContractStatus::Pass,
                decision: RolloutContractDecision::Release,
                lifecycle_state: RolloutContractLifecycleState::Terminal,
                enforcement_mode: RolloutContractEnforcementMode::Enforce,
                failure_reasons: vec![],
                diagnostics: vec![],
                waiver: None,
                rollback_disposition: serde_json::json!({
                    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                    "data_loss_risk": "none",
                    "steps": ["Move enforcement mode through an audited mutation."]
                }),
                projection_integrity: ProjectionIntegrity::Valid,
                cutover_policy_revision: Some("p084-cutover-v1".into()),
                redaction_state: "partial".into(),
                retry_count: 0,
                preflight_timeout_seconds: 45,
            },
            now,
        )
        .await
        .unwrap();
    }

    fn make_workflow_conflict(run_id: RunId) -> WorkflowConflictRecord {
        let candidates = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_refine".into(),
            from_state_id: "review".into(),
            to_state_id: "refine".into(),
            condition_expression_id: Some("proposal_review_summary.pass == false".into()),
            result: CandidateTransitionResult::MissingInput,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec!["proposal_review_summary".into()],
            missing_fields: vec![],
            source_artifact_ids: vec![],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some("proposal_review_summary is required".into()),
        }];
        let reason = WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition;
        let candidate_hash = candidate_transition_hash(&candidates);
        WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: workflow_conflict_fingerprint(
                &run_id.to_string(),
                "review",
                &reason,
                &candidate_hash,
                &[],
            ),
            run_id: run_id.to_string(),
            stage_execution_id: None,
            lineage_id: Some("lineage-p017".into()),
            current_state_id: "review".into(),
            reason,
            operator_label: "Required transition input is missing".into(),
            status: WorkflowConflictStatus::Unresolved,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: vec![],
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".into(),
        }
    }

    fn make_lead_mediation_record(
        run_id: RunId,
        conflict: &WorkflowConflictRecord,
        mediation_id: &str,
    ) -> LeadConflictMediationRecord {
        LeadConflictMediationRecord {
            id: mediation_id.to_string(),
            run_id: run_id.to_string(),
            conflict_id: conflict.conflict_id.clone(),
            conflict_fingerprint: conflict.conflict_fingerprint.clone(),
            lead_agent_id: "lead-agent-1".into(),
            status: LeadMediationStatus::OperatorConfirmationRequired,
            settlement_result: Some("operator_confirmed".into()),
            recovery_action: None,
            chosen_action: Some("advance".into()),
            chosen_next_state_id: Some("release".into()),
            chosen_next_state_label: Some("Release".into()),
            operator_rationale: Some("PRIVATE rationale must not leave storage".into()),
            sanitized_progress: Some("Lead mediation selected a release transition.".into()),
            validation_errors_json: Some(
                serde_json::json!([{"field": "summary", "message": "safe validation note"}])
                    .to_string(),
            ),
            cost_summary_json: Some(
                serde_json::json!({
                    "total_cost_cents": 42,
                    "input_tokens": 100,
                    "output_tokens": 25
                })
                .to_string(),
            ),
            metric_event_id: Some("metric-1".into()),
            superseded_by_event_ref: Some("event-2".into()),
            agent_execution_id: Some("agent-exec-1".into()),
            confirmation_subject_id: Some("confirmation-1".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settled_at: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    async fn p043_test_pool() -> sqlx::SqlitePool {
        let path =
            std::env::temp_dir().join(format!("chainworks-p043-{}.sqlite", uuid::Uuid::new_v4()));
        create_pool(&format!("sqlite://{}", path.to_string_lossy()))
            .await
            .expect("P043 file-backed pool failed")
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        Arc::new(CommandHandler::new(pool, events, work_queue))
    }

    fn assert_enum_values(json: &serde_json::Value, alias: &str, expected: &[&str]) {
        let values = json[alias]["enumValues"]
            .as_array()
            .unwrap_or_else(|| panic!("{alias} enumValues should be present"));
        let actual: Vec<&str> = values
            .iter()
            .map(|value| {
                value["name"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{alias} enum value name should be a string"))
            })
            .collect();
        assert_eq!(actual, expected.to_vec(), "{alias} enum values drifted");
    }

    async fn persist_blocked_implementation_summary(pool: &sqlx::SqlitePool, run_id: RunId) {
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_8_implementation_continued".into(),
            agent_id: "code_writer".into(),
            name: "implementation_self_assessment".into(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/implementation/self-assessment.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["verification blocked by environment"],
            "tests_run": ["cargo test: blocked"],
            "docs_impacted": []
        });
        let summary = parse_implementation_self_assessment_v2(
            &raw,
            ContractParseContext {
                run_id: run_id.to_string(),
                run_age: None,
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(artifact.file_path.clone()),
                source_generation_id: None,
                artifact_created_at: Some(artifact.created_at),
                v2_generation_seen_for_run: true,
                legacy_v1_generation_available: false,
            },
        );
        artifact_contracts::persist_implementation_self_assessment_summary(
            pool,
            run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await
        .unwrap();
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
                stage_execution_id: Some(stage_id),
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
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: Some(17),
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
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
    async fn proposal_064_run_query_exposes_sync_and_capsule_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO main_sync_attempts (id, run_id, idempotency_key, trigger_reason, status, conflict_count, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind("attempt-1")
        .bind(run_id.to_string())
        .bind("before-review-1")
        .bind("before_review")
        .bind("waiting_for_barrier")
        .bind(0_i64)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO worktree_mutation_barriers (id, run_id, worktree_resource_key, owner_id, owner_kind, status, reason, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind("barrier-1")
        .bind(run_id.to_string())
        .bind(format!("run-worktree:{run_id}"))
        .bind("main-sync")
        .bind("main_sync")
        .bind("pending")
        .bind("active reader")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        let run_id_string = run_id.to_string();
        db::repos::command_journal::record(
            &pool,
            "journal-1",
            "MainSyncRequest",
            "{}",
            Some(&run_id_string),
            Utc::now(),
            Some("mcp"),
            Some("operator"),
            Some("operator"),
            Some("runs.main_sync.request"),
            None,
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    {{
                      run(id: "{run_id}") {{
                        mainSyncReadbackJson
                        knowledgeCapsuleReadbackJson
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
        let main_sync = &json["run"]["mainSyncReadbackJson"];
        assert_eq!(
            main_sync["schema_version"], "p064_main_sync_readback_v1",
            "unexpected P064 readback payload: {json}"
        );
        assert_eq!(main_sync["latest_attempt"]["status"], "waiting_for_barrier");
        assert_eq!(main_sync["active_barrier"]["owner_kind"], "main_sync");
        assert_eq!(
            main_sync["commands"]["pending_commands"][0]["command_type"],
            "MainSyncRequest"
        );
        assert_eq!(
            json["run"]["knowledgeCapsuleReadbackJson"]["schema_version"],
            "p064_knowledge_capsule_readback_v1"
        );
    }

    #[tokio::test]
    async fn run_query_exposes_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
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
        let mut run = make_run(run_id, idea_id);
        run.delivery_configuration_json = Some(delivery_json.clone());
        runs::insert(&pool, &run).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryConfigurationJson
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
            json["run"]["deliveryConfigurationJson"],
            serde_json::json!(delivery_json)
        );
    }

    #[tokio::test]
    async fn run_query_exposes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_blocked_implementation_summary(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    implementationSelfAssessmentSummary {{
                      status
                      implementationComplete
                      verificationGreen
                      blockingRemainingCodeTaskCount
                      testsRun
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
        let summary = &json["run"]["implementationSelfAssessmentSummary"];
        assert_eq!(summary["status"], serde_json::json!("blocked"));
        assert_eq!(summary["implementationComplete"], serde_json::json!(true));
        assert_eq!(summary["verificationGreen"], serde_json::json!(false));
        assert_eq!(
            summary["blockingRemainingCodeTaskCount"],
            serde_json::json!(0)
        );
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_current_workflow_conflict() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let conflict = make_workflow_conflict(run_id);
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    workflowConflict {{
                      reason
                      status
                      currentStateId
                      candidateTransitions {{
                        transitionId
                        result
                        missingArtifacts
                      }}
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
        let conflict_json = &json["run"]["workflowConflict"];
        assert_eq!(
            conflict_json["reason"],
            serde_json::json!("REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION")
        );
        assert_eq!(conflict_json["status"], serde_json::json!("UNRESOLVED"));
        assert_eq!(conflict_json["currentStateId"], serde_json::json!("review"));
        assert_eq!(
            conflict_json["candidateTransitions"][0]["result"],
            serde_json::json!("MISSING_INPUT")
        );
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_refine_instruction_action_hint() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let mut conflict = make_workflow_conflict(run_id);
        conflict.reason = WorkflowConflictReason::NoDeclarativeTransitionMatched;
        conflict.operator_label = "No declarative workflow transition matched".into();
        conflict.candidate_transitions = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_refine".into(),
            from_state_id: "review".into(),
            to_state_id: "review".into(),
            condition_expression_id: Some("proposal_needs_refine".into()),
            result: CandidateTransitionResult::NotMatched,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec![],
            missing_fields: vec![],
            source_artifact_ids: vec!["proposal_review_summary".into()],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(
                "Loop budget exhausted for proposal_review_count: 3/3 iterations".into(),
            ),
        }];
        conflict.candidate_transition_hash =
            candidate_transition_hash(&conflict.candidate_transitions);
        conflict.conflict_fingerprint = workflow_conflict_fingerprint(
            &run_id.to_string(),
            "review",
            &conflict.reason,
            &conflict.candidate_transition_hash,
            &[],
        );
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      suggestedOperatorAction
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
            json["run"]["workflowConflict"]["suggestedOperatorAction"],
            serde_json::json!("choose_transition_or_provide_refine_instruction")
        );
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_sanitized_lead_mediation_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-readback";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        assert!(!schema.sdl().contains("operatorRationale"));

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query P017LeadMediationReadback {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      mediationRecordId
                      leadMediation {{
                        id
                        conflictId
                        leadAgentId
                        status
                        resolutionMode
                        chosenAction
                        chosenNextStateId
                        chosenNextStateLabel
                        sanitizedProgress
                        statusUpdates {{
                          status
                          sanitizedProgress
                          updatedAt
                          attemptNumber
                        }}
                        validationErrors
                        confirmationSubjectId
                        supersededByEventRef
                        costSummary
                      }}
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
        let mediation = &json["run"]["workflowConflict"]["leadMediation"];
        assert_eq!(mediation["id"], serde_json::json!(mediation_id));
        assert_eq!(
            mediation["conflictId"],
            serde_json::json!(conflict.conflict_id)
        );
        assert_eq!(mediation["leadAgentId"], serde_json::json!("lead-agent-1"));
        assert_eq!(
            mediation["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["resolutionMode"],
            serde_json::json!("operator_confirmation")
        );
        assert_eq!(mediation["chosenAction"], serde_json::json!("advance"));
        assert_eq!(mediation["chosenNextStateId"], serde_json::json!("release"));
        assert_eq!(
            mediation["chosenNextStateLabel"],
            serde_json::json!("Release")
        );
        assert_eq!(
            mediation["sanitizedProgress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["sanitizedProgress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["attemptNumber"],
            serde_json::json!(1)
        );
        assert!(mediation["statusUpdates"][0]["updatedAt"].is_string());
        assert_eq!(
            mediation["confirmationSubjectId"],
            serde_json::json!("confirmation-1")
        );
        assert_eq!(
            mediation["supersededByEventRef"],
            serde_json::json!("event-2")
        );
        assert_eq!(
            mediation["validationErrors"][0]["field"],
            serde_json::json!("summary")
        );
        assert_eq!(
            mediation["costSummary"]["total_cost_cents"],
            serde_json::json!(42)
        );

        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    /// P017 R2 / API-001: every mediation-owned `agent_executions` row must
    /// surface under `workflowConflict.leadMediation.executionAttempts` in
    /// GraphQL, with owner identity, nullable stage execution ID, runtime
    /// facts, watchdog, and per-attempt timing/status.
    #[tokio::test]
    async fn proposal_017_run_query_exposes_lead_mediation_execution_attempts() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-attempts";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        // Insert two mediation-owned agent_executions (no stage_execution_id).
        let exec_one = domain::agent::AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            stage_execution_id: None,
            agent_id: "lead-agent-1".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
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
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: Some("lead_conflict_mediation".into()),
            owner_id: Some(mediation_id.into()),
            lead_mediation_record_id: Some(mediation_id.into()),
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
        };
        let exec_one_id = exec_one.id;
        db::repos::agent_executions::insert(&pool, &exec_one)
            .await
            .unwrap();

        let exec_two = domain::agent::AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            started_at: exec_one.started_at + chrono::Duration::seconds(1),
            status: domain::agent::AgentStatus::Completed,
            ..exec_one.clone()
        };
        let exec_two_id = exec_two.id;
        db::repos::agent_executions::insert(&pool, &exec_two)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query P017LeadMediationAttempts {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      leadMediation {{
                        statusUpdates {{ attemptNumber }}
                        executionAttempts {{
                          agentExecutionId
                          ownerKind
                          ownerId
                          mediationRecordId
                          stageExecutionId
                          agentId
                          provider
                          model
                          status
                          startedAt
                          completedAt
                          attemptNumber
                          runtimeFacts
                          watchdog
                          cost
                          transcriptRef
                          artifacts {{ id }}
                        }}
                      }}
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
        let mediation = &json["run"]["workflowConflict"]["leadMediation"];
        let attempts = mediation["executionAttempts"]
            .as_array()
            .expect("executionAttempts array");
        assert_eq!(attempts.len(), 2, "two attempts expected");

        for attempt in attempts {
            assert_eq!(
                attempt["ownerKind"],
                serde_json::json!("lead_conflict_mediation")
            );
            assert_eq!(attempt["ownerId"], serde_json::json!(mediation_id));
            assert_eq!(
                attempt["mediationRecordId"],
                serde_json::json!(mediation_id)
            );
            assert!(
                attempt["stageExecutionId"].is_null(),
                "mediation-owned attempt has no stage execution id"
            );
            assert_eq!(attempt["agentId"], serde_json::json!("lead-agent-1"));
            assert_eq!(attempt["provider"], serde_json::json!("claude"));
            assert!(attempt["startedAt"].is_string());
        }

        // Attempts are sorted by started_at ASC; attemptNumber is durable.
        assert_eq!(
            attempts[0]["agentExecutionId"],
            serde_json::json!(exec_one_id.to_string())
        );
        assert_eq!(attempts[0]["attemptNumber"], serde_json::json!(1));
        assert_eq!(attempts[0]["status"], serde_json::json!("failed"));
        assert_eq!(
            attempts[1]["agentExecutionId"],
            serde_json::json!(exec_two_id.to_string())
        );
        assert_eq!(attempts[1]["attemptNumber"], serde_json::json!(2));
        assert_eq!(attempts[1]["status"], serde_json::json!("completed"));

        // The synthesized status_updates entry's attemptNumber reflects the
        // durable mediation attempt count, not hard-coded 1.
        assert_eq!(
            mediation["statusUpdates"][0]["attemptNumber"],
            serde_json::json!(2)
        );

        // No operator_rationale anywhere in the readback.
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    #[tokio::test]
    async fn proposal_017_runs_query_exposes_current_workflow_conflict_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &make_workflow_conflict(run_id))
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query Runs {
                  runs {
                    id
                    workflowConflict {
                      reason
                      status
                      currentStateId
                    }
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let run_json = json["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["id"] == serde_json::json!(run_id.to_string()))
            .expect("run appears in active run list");
        assert_eq!(
            run_json["workflowConflict"]["reason"],
            serde_json::json!("REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION")
        );
        assert_eq!(
            run_json["workflowConflict"]["status"],
            serde_json::json!("UNRESOLVED")
        );
        assert_eq!(
            run_json["workflowConflict"]["currentStateId"],
            serde_json::json!("review")
        );
    }

    #[tokio::test]
    async fn delivery_preflight_graphql_readback_tests() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.delivery_preflight_json = Some(
            serde_json::json!({
                "passed": true,
                "checks": [
                    {
                        "id": "repo_root_exists",
                        "label": "Repository root exists",
                        "passed": true,
                        "detail": null
                    }
                ]
            })
            .to_string(),
        );
        runs::insert(&pool, &run).await.unwrap();
        persist_rollout_contract_readback(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryPreflightJson
                    rolloutContractReadbackJson
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
        assert!(json["run"]["deliveryPreflightJson"]
            .as_str()
            .unwrap()
            .contains("repo_root_exists"));
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["schemaVersion"],
            serde_json::json!("operator_readback_v1")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["backendDecision"],
            serde_json::json!("release")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["sourceLane"],
            serde_json::json!("graphql")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["rollbackDisposition"]["dataLossRisk"],
            serde_json::json!("none")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["adoptionMetric"]["name"],
            serde_json::json!("new_applicable_proposals_with_passing_rollout_contract_percent")
        );
    }

    #[tokio::test]
    async fn execution_mcp_truth_contract_tests() {
        let pool = p043_test_pool().await;
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
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
                ))
                .data(test_principal()),
            )
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
    async fn proposal_043_run_query_uses_projection_summary_fields() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043RunDetail {{
                      run(id: "{run_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        totalStages
                        failedStages
                        pendingApprovals
                      }}
                      stage(id: "{stage_execution_id}") {{
                        id
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 run detail query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["run"]["projectionPresent"], serde_json::json!(true));
        assert!(json["run"]["projectionUpdatedAt"].is_string());
        assert_eq!(json["run"]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["run"]["totalStages"], serde_json::json!(1));
        assert_eq!(json["run"]["failedStages"], serde_json::json!(1));
        assert_eq!(json["run"]["pendingApprovals"], serde_json::json!(1));
    }

    #[tokio::test]
    async fn proposal_043_stage_queries_expose_projection_decision_flags() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        let payload_path = std::env::temp_dir().join(format!("p043-artifact-{run_id}.json"));
        std::fs::write(&payload_path, br#"{"ok":true}"#).unwrap();
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
            agent_execution_id: None,
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043StageReadback {{
                      stages(runId: "{run_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasArtifacts
                        hasPendingApproval
                        hasValidationFailure
                      }}
                      stage(id: "{stage_execution_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasArtifacts
                        hasPendingApproval
                        hasValidationFailure
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 stage readback query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["stages"][0]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["stages"][0]["projectionUpdatedAt"].is_string());
        assert_eq!(json["stages"][0]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["stages"][0]["hasArtifacts"], serde_json::json!(true));
        assert_eq!(
            json["stages"][0]["hasPendingApproval"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stages"][0]["hasValidationFailure"],
            serde_json::json!(true)
        );
        assert_eq!(json["stage"]["projectionPresent"], serde_json::json!(true));
        assert!(json["stage"]["projectionUpdatedAt"].is_string());
        assert_eq!(json["stage"]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["stage"]["hasArtifacts"], serde_json::json!(true));
        assert_eq!(json["stage"]["hasPendingApproval"], serde_json::json!(true));
        assert_eq!(
            json["stage"]["hasValidationFailure"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn proposal_043_graphql_reads_are_operator_only_v1() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P043OperatorOnly {
                      runs { id }
                    }
                    "#,
                )
                .data(observer_principal()),
            )
            .await;

        assert!(
            response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "P043 V1 reads must reject non-operator principals: {response:?}"
        );
    }

    #[tokio::test]
    async fn proposal_043_run_subscription_uses_projection_summary_fields() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
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
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription P043RunSubscription {{
                  runStatusChanged(runId: "{run_id}") {{
                    id
                    projectionPresent
                    projectionUpdatedAt
                    projectionLag
                    totalStages
                    pendingApprovals
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::RunStatusChanged {
                run_id,
                status: domain::run::RunStatus::Ready,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 run subscription frame timed out")
            .expect("P043 run subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 run subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["runStatusChanged"]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["runStatusChanged"]["projectionUpdatedAt"].is_string());
        assert_eq!(
            json["runStatusChanged"]["projectionLag"],
            serde_json::json!(false)
        );
        assert_eq!(
            json["runStatusChanged"]["totalStages"],
            serde_json::json!(1)
        );
        assert_eq!(
            json["runStatusChanged"]["pendingApprovals"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn run_subscription_refreshes_on_runtime_progress_events() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription RuntimeProgressRefreshesRun {{
                  runStatusChanged(runId: "{run_id}") {{
                    id
                    status
                    freshnessState
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::RuntimeStatusChanged {
                run_id,
                stage_id: "state_9".into(),
                agent_id: "code_writer".into(),
                provider: "codex".into(),
                event_kind: "session_started".into(),
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("runtime progress run subscription frame timed out")
            .expect("runtime progress run subscription ended");
        assert!(
            frame.errors.is_empty(),
            "runtime progress run subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["runStatusChanged"]["id"],
            serde_json::json!(run_id.to_string())
        );
    }

    #[tokio::test]
    async fn proposal_043_stage_subscription_uses_projection_decision_flags() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        let payload_path = std::env::temp_dir().join(format!("p043-sub-artifact-{run_id}.json"));
        std::fs::write(&payload_path, br#"{"ok":true}"#).unwrap();
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
            agent_execution_id: None,
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
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription P043StageSubscription {{
                  stageStatusChanged(runId: "{run_id}") {{
                    id
                    projectionPresent
                    projectionUpdatedAt
                    projectionLag
                    hasArtifacts
                    hasPendingApproval
                    hasValidationFailure
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id,
                status: domain::stage::StageStatus::Failed,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 stage subscription frame timed out")
            .expect("P043 stage subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 stage subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["stageStatusChanged"]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["stageStatusChanged"]["projectionUpdatedAt"].is_string());
        assert_eq!(
            json["stageStatusChanged"]["projectionLag"],
            serde_json::json!(false)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasArtifacts"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasPendingApproval"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasValidationFailure"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn proposal_043_approval_resolved_subscription_is_available() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(
                r#"
                subscription P043ApprovalResolved {
                  approvalResolved {
                    id
                    decision
                    decidedAt
                  }
                }
                "#,
            )
            .data(test_principal()),
        );
        let pool_for_event = pool.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            approvals::resolve(
                &pool_for_event,
                approval_id,
                domain::approval::ApprovalDecision::Granted,
                Utc::now(),
                Some("approved".into()),
            )
            .await
            .unwrap();
            let _ = bus.send(DomainEvent::ApprovalResolved {
                approval_id,
                decision: domain::approval::ApprovalDecision::Granted,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 approvalResolved subscription frame timed out")
            .expect("P043 approvalResolved subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 approvalResolved subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["approvalResolved"]["decision"],
            serde_json::json!("granted")
        );
        assert!(
            json["approvalResolved"]["decidedAt"].is_string(),
            "resolved approval subscription must expose decidedAt: {json:?}"
        );
    }

    #[tokio::test]
    async fn proposal_043_missing_projection_rows_are_explicit_lag_state() {
        let pool = p043_test_pool().await;
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043MissingProjectionLag {{
                      run(id: "{run_id}") {{
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        pendingApprovals
                      }}
                      stage(id: "{stage_execution_id}") {{
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasPendingApproval
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 missing projection query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["run"]["projectionPresent"], serde_json::json!(false));
        assert_eq!(json["run"]["projectionUpdatedAt"], serde_json::Value::Null);
        assert_eq!(json["run"]["projectionLag"], serde_json::json!(true));
        assert_eq!(json["stage"]["projectionPresent"], serde_json::json!(false));
        assert_eq!(
            json["stage"]["projectionUpdatedAt"],
            serde_json::Value::Null
        );
        assert_eq!(json["stage"]["projectionLag"], serde_json::json!(true));
        assert_ne!(
            json["run"]["projectionLag"],
            serde_json::json!(false),
            "missing projection must not be indistinguishable from normal zero-count truth"
        );
        assert_ne!(
            json["stage"]["projectionLag"],
            serde_json::json!(false),
            "missing stage projection must not be indistinguishable from normal false flags"
        );
    }

    #[tokio::test]
    async fn proposal_031_schema_exposes_required_enum_values() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P031EnumContract {
                      freshness: __type(name: "FreshnessState") {
                        enumValues { name }
                      }
                      disabledReason: __type(name: "DisabledReasonCode") {
                        enumValues { name }
                      }
                      writePath: __type(name: "WritePathState") {
                        enumValues { name }
                      }
                      payloadAvailability: __type(name: "PayloadAvailabilityState") {
                        enumValues { name }
                      }
                      payloadUnavailableReason: __type(name: "PayloadUnavailableReasonCode") {
                        enumValues { name }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 enum contract introspection must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_enum_values(
            &json,
            "freshness",
            &[
                "live",
                "refreshing",
                "projection_lag",
                "stale",
                "unavailable",
                "unauthorized",
            ],
        );
        assert_enum_values(
            &json,
            "disabledReason",
            &[
                "WRITE_PATH_NOT_AVAILABLE",
                "MANAGED_OUTSIDE_UI",
                "AMBIGUOUS_APPROVAL_IDENTITY",
                "STALE_READ",
                "PROJECTION_LAG",
                "UNAUTHORIZED",
                "UNSUPPORTED_ACTION",
            ],
        );
        assert_enum_values(
            &json,
            "writePath",
            &[
                "available",
                "read_only_diagnostic",
                "write_path_not_available",
                "external_transport_required",
                "hidden",
            ],
        );
        assert_enum_values(
            &json,
            "payloadAvailability",
            &[
                "available",
                "metadata_only",
                "payload_deferred",
                "generating",
                "unavailable",
            ],
        );
        assert_enum_values(
            &json,
            "payloadUnavailableReason",
            &[
                "PAYLOAD_DEFERRED_BY_P031",
                "GENERATING",
                "NOT_INDEXED",
                "NOT_AUTHORIZED",
                "NOT_AVAILABLE",
                "UNKNOWN",
            ],
        );
    }

    #[tokio::test]
    async fn proposal_075_storage_health_is_typed_graphql_contract() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let sdl = schema.sdl();
        assert!(sdl.contains("type StorageHealth"));
        assert!(sdl.contains("type DbWriterHealth"));
        assert!(sdl.contains("type EvidenceSpoolSummary"));
        assert!(sdl.contains("enum StorageDbState"));
        assert!(!sdl.contains("storageHealth: JSON"));

        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P075StorageHealthTyped {
                      storageHealth {
                        updatedAt
                        staleAfterMs
                        isStale
                        dbState
                        writer {
                          alive
                          lanes { lane capacity queuedDepth queuedDepthRatio }
                        }
                        wal { available warnSizeBytes criticalSizeBytes }
                        evidenceSpool {
                          enabled
                          filesWrittenTotal
                          bytesWrittenTotal
                          metadataRowsTotal
                        }
                        thresholds { metric warn critical unit action }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P075 typed storageHealth query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["storageHealth"]["staleAfterMs"], 5000);
        assert!(json["storageHealth"]["writer"]["lanes"]
            .as_array()
            .is_some_and(|lanes| lanes.len() >= 6));
        assert!(json["storageHealth"]["thresholds"]
            .as_array()
            .is_some_and(|thresholds| !thresholds.is_empty()));
    }

    #[tokio::test]
    async fn proposal_075_storage_health_reads_live_dbwriter_heartbeat() {
        let pool = test_pool().await;
        let writer = db::writer::DbWriter::new(pool.clone());
        let result = writer
            .submit(
                WriteOperation {
                    class: WriteClass::A,
                    lane: WriteLane::CriticalBarrier,
                    operation_name: "graphql_storage_health_live_writer_test",
                    expected_rows: 1,
                    batchable: false,
                    barrier: true,
                    deadline: std::time::Duration::from_secs(5),
                    deadline_reason: None,
                    idempotency_key: "graphql-storage-health-live-writer".into(),
                    replay_policy: ReplayPolicy::NaturalKey,
                    observed_at: None,
                },
                |pool| async move {
                    let mut tx = db::pool::begin_immediate_with_retry(
                        &pool,
                        "graphql_storage_health_live_writer_test",
                    )
                    .await?;
                    sqlx::query(
                        "CREATE TABLE IF NOT EXISTS p075_graphql_storage_health_probe (id TEXT PRIMARY KEY)",
                    )
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT OR REPLACE INTO p075_graphql_storage_health_probe (id) VALUES ('probe')",
                    )
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    Ok(1)
                },
            )
            .await;
        assert_eq!(result, WriteResult::Committed);

        for _ in 0..30 {
            if writer.is_alive() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(writer.is_alive(), "DbWriter heartbeat should become live");

        let schema = build_schema_with_storage_writer(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            writer.heartbeat.clone(),
        );

        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P075StorageHealthWriter {
                      storageHealth {
                        isStale
                        writer {
                          alive
                          totalQueued
                          lastHeartbeatAt
                          lastDrainAt
                          writeLockWaitP50Ms
                          writeLockWaitP95Ms
                          transactionDurationP95Ms
                          lanes { lane queuedDepth }
                        }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P075 live storageHealth query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["storageHealth"]["writer"]["alive"], true);
        assert_eq!(json["storageHealth"]["isStale"], false);
        assert!(json["storageHealth"]["writer"]["lastHeartbeatAt"]
            .as_str()
            .is_some());
        assert!(json["storageHealth"]["writer"]["lastDrainAt"]
            .as_str()
            .is_some());
        assert!(json["storageHealth"]["writer"]["writeLockWaitP50Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["writeLockWaitP95Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["transactionDurationP95Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["lanes"]
            .as_array()
            .is_some_and(|lanes| lanes.len() == 6));
    }

    #[tokio::test]
    async fn proposal_031_freshness_state_is_derived_from_server_projection() {
        let pool = p043_test_pool().await;
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
            test_reporter(),
        );

        let lagging = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031LaggingFreshness {{
                      run(id: "{run_id}") {{ freshnessState projectionLag }}
                      stage(id: "{stage_execution_id}") {{ freshnessState projectionLag }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            lagging.errors.is_empty(),
            "P031 lagging freshness query must succeed: {lagging:?}"
        );
        let lagging_json = lagging.data.into_json().unwrap();
        assert_eq!(
            lagging_json["run"]["freshnessState"],
            serde_json::json!("projection_lag")
        );
        assert_eq!(
            lagging_json["stage"]["freshnessState"],
            serde_json::json!("projection_lag")
        );

        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();
        let live = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031LiveFreshness {{
                      run(id: "{run_id}") {{ freshnessState projectionLag }}
                      stage(id: "{stage_execution_id}") {{ freshnessState projectionLag }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            live.errors.is_empty(),
            "P031 live freshness query must succeed: {live:?}"
        );
        let live_json = live.data.into_json().unwrap();
        assert_eq!(
            live_json["run"]["freshnessState"],
            serde_json::json!("live")
        );
        assert_eq!(
            live_json["stage"]["freshnessState"],
            serde_json::json!("live")
        );
    }

    #[tokio::test]
    async fn proposal_031_approval_inbox_is_diagnostic_read_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P031ApprovalDiagnostics {
                      approvalInbox {
                        id
                        freshnessState
                        disabledReasonCode
                        writePathState
                        diagnosticId
                        serverDebugDetail
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 approval diagnostic query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let approval = &json["approvalInbox"][0];
        assert_eq!(approval["freshnessState"], serde_json::json!("live"));
        assert_eq!(approval["disabledReasonCode"], serde_json::Value::Null);
        assert_eq!(approval["writePathState"], serde_json::json!("available"));
        assert_eq!(
            approval["diagnosticId"],
            serde_json::json!(approval_id.to_string())
        );
        assert!(
            approval["serverDebugDetail"].is_null(),
            "serverDebugDetail must be null for Phase 0 approval rows"
        );
    }

    #[tokio::test]
    async fn proposal_031_approval_inbox_can_be_scoped_to_run() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let other_run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        let other_approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        runs::insert(&pool, &make_run(other_run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: other_approval_id,
                run_id: other_run_id,
                stage_id: "stage_2".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::rebuild_approval_inbox(&pool, other_run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031RunScopedApprovalDiagnostics {{
                      approvalInbox(runId: "{run_id}") {{
                        id
                        runId
                        stageId
                        writePathState
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 run-scoped approval query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["approvalInbox"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["approvalInbox"][0]["id"],
            serde_json::json!(approval_id.to_string())
        );
        assert_eq!(
            json["approvalInbox"][0]["runId"],
            serde_json::json!(run_id.to_string())
        );
    }

    #[tokio::test]
    async fn proposal_031_report_artifacts_are_metadata_only_payloads() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "Release report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ReportPayloadMetadata {{
                      artifacts(runId: "{run_id}") {{
                        id
                        freshnessState
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        diagnosticId
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 report metadata query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("metadata_only")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
        );
        assert_eq!(
            artifact["diagnosticId"],
            serde_json::json!(artifact_id.to_string())
        );
        assert!(
            artifact["serverDebugDetail"].is_string(),
            "operator diagnostic detail should explain why report payload rendering is deferred"
        );
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_is_server_owned_readback() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-artifact-payload-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("proposal.md");
        fs::write(&artifact_path, "# Proposal\n\nGraphQL payload").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "proposal.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(24),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadReadback {{
                      artifacts(runId: "{run_id}") {{
                        id
                        format
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 artifact payload readback query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(artifact["format"], serde_json::json!("markdown"));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert!(artifact["payloadUnavailableReasonCode"].is_null());
        assert_eq!(
            artifact["payloadText"],
            serde_json::json!("# Proposal\n\nGraphQL payload")
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_query_reads_selected_payload_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-selected-artifact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("selected.md");
        fs::write(&artifact_path, "# Selected\n\nOnly this artifact").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "selected.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(29),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031SelectedArtifactPayload {{
                      artifact(id: "{artifact_id}") {{
                        id
                        payloadAvailabilityState
                        payloadText
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 selected artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifact"];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert_eq!(
            artifact["payloadText"],
            serde_json::json!("# Selected\n\nOnly this artifact")
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_is_capped_for_bulk_readback() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-artifact-preview-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("large.md");
        let payload = format!(
            "{}tail-marker",
            "large artifact preview line\n"
                .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 28) + 256)
        );
        fs::write(&artifact_path, &payload).unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "large.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(payload.len() as i64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadPreview {{
                      artifacts(runId: "{run_id}") {{
                        payloadAvailabilityState
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 capped artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        let preview = artifact["payloadText"].as_str().unwrap();
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert!(preview.len() <= P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
        assert!(preview.starts_with("large artifact preview line"));
        assert!(!preview.contains("tail-marker"));
        assert!(
            artifact["serverDebugDetail"]
                .as_str()
                .unwrap()
                .contains("preview capped"),
            "truncated payloads should expose operator-visible preview metadata"
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_has_bulk_response_budget() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root = std::env::temp_dir().join(format!(
            "p031-artifact-bulk-preview-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&artifact_root).unwrap();
        let payload = format!(
            "{}tail-marker",
            "bulk artifact preview line\n"
                .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 27) + 8)
        );

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        for index in 0..10 {
            let artifact_id = ArtifactId::new();
            let artifact_path = artifact_root.join(format!("large-{index}.md"));
            fs::write(&artifact_path, &payload).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: artifact_id,
                    run_id,
                    stage_id: "proposal".into(),
                    agent_id: "proposal_writer".into(),
                    name: format!("large-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: artifact_path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    // Stale discovery metadata can under-report size. The
                    // response budget must be enforced from actual preview
                    // bytes, not only the indexed size.
                    size_bytes: Some(0),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadBulkBudget {{
                      artifacts(runId: "{run_id}") {{
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 bulk artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        let available_count = artifacts
            .iter()
            .filter(|artifact| {
                artifact["payloadAvailabilityState"] == serde_json::json!("available")
            })
            .count();
        let deferred_count = artifacts
            .iter()
            .filter(|artifact| {
                artifact["payloadAvailabilityState"] == serde_json::json!("payload_deferred")
            })
            .count();
        let total_payload_bytes: usize = artifacts
            .iter()
            .filter_map(|artifact| artifact["payloadText"].as_str())
            .map(str::len)
            .sum();

        assert_eq!(available_count, 8);
        assert_eq!(deferred_count, 2);
        assert!(total_payload_bytes <= P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES);
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadUnavailableReasonCode"]
                == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["serverDebugDetail"]
                    .as_str()
                    .unwrap()
                    .contains("bulk artifact list reached its payload preview budget")
        }));

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_metadata_query_does_not_consume_payload_budget() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root = std::env::temp_dir().join(format!(
            "p031-artifact-metadata-budget-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&artifact_root).unwrap();
        let payload = "metadata-only artifact preview line\n"
            .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 36) + 8);

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        for index in 0..10 {
            let artifact_id = ArtifactId::new();
            let artifact_path = artifact_root.join(format!("metadata-{index}.md"));
            fs::write(&artifact_path, &payload).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: artifact_id,
                    run_id,
                    stage_id: "proposal".into(),
                    agent_id: "proposal_writer".into(),
                    name: format!("metadata-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: artifact_path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    size_bytes: Some(payload.len() as i64),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactMetadataList {{
                      artifacts(runId: "{run_id}") {{
                        id
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 artifact metadata query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 10);
        assert!(artifacts.iter().all(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("available")
                && artifact["payloadUnavailableReasonCode"].is_null()
                && artifact["serverDebugDetail"].is_null()
        }));

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_diagnostic_metadata_is_operator_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "Release report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let approval_response = schema
            .execute(
                Request::new(
                    r#"
                    query P031ApprovalDiagnosticsObserverDenied {
                      approvalInbox {
                        diagnosticId
                        serverDebugDetail
                        disabledReasonCode
                        writePathState
                      }
                    }
                    "#,
                )
                .data(observer_principal()),
            )
            .await;
        assert!(
            approval_response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "observer principals must not read P031 approval diagnostic metadata: {approval_response:?}"
        );

        let artifact_response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ReportDiagnosticsObserverDenied {{
                      artifacts(runId: "{run_id}") {{
                        diagnosticId
                        serverDebugDetail
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                      }}
                    }}
                    "#
                ))
                .data(observer_principal()),
            )
            .await;
        assert!(
            artifact_response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "observer principals must not read P031 report diagnostic metadata: {artifact_response:?}"
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    cancellationSettlementLog
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query Runs {
                  runs {
                    id
                    cancellationSettlementSummary
                    cancellationSettlementLog
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
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
            agent_execution_id: None,
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
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
                ))
                .data(test_principal()),
            )
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
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
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
                )
                .data(test_principal()),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["stewardAnalyses"][0]["id"],
            "analysis-1"
        );
    }

    #[tokio::test]
    async fn proposal_041_graphql_readback_parity_surfaces() {
        for fixture_id in p041_selected_fixtures() {
            // The engine crate's `proposal_041_parity.rs` integration
            // test produces
            // `target/parity/reports/<generation>/<fixture_id>/behavioral-diff-report.json`
            // + the SQLite DB at the path that report's `database_ref`
            // points to. Under `cargo test --workspace` the engine
            // integration binary and this graphql-server lib binary
            // run in parallel slots — there is no ordering guarantee.
            // If the engine binary hasn't produced the report yet (or
            // was cleaned between runs), skip instead of failing. The
            // engine-side gate still enforces that the report IS
            // produced; this test exercises the readback contract
            // only when the artifacts exist. The dedicated
            // `./scripts/test-gate.sh proposal-041` lane runs both in
            // the right order and is the authoritative readiness
            // signal for P041.
            let report_path = p041_report_path(fixture_id);
            let replay_path = p041_replay_path(fixture_id);
            if !report_path.is_file() || !replay_path.is_file() {
                eprintln!(
                    "P041 readback: skipping fixture '{fixture_id}' — engine-side replay has \
                     not produced {} yet. Run `cargo test -p engine --test proposal_041_parity` \
                     first, or use `./scripts/test-gate.sh proposal-041`.",
                    report_path.display()
                );
                return;
            }
            let mut report = p041_report(fixture_id);
            let replay = p041_replay(fixture_id);
            let run_id = replay["run_id"].as_str().expect("run_id");
            let idea_id = replay["run_projection"]["idea_id"]
                .as_str()
                .expect("idea_id");
            // stageQueueSummary requires a stage execution ID; use the first
            // stage from the replay's stage_projection.
            let first_stage_exec_id = replay["stage_projection"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|s| s["id"].as_str())
                .unwrap_or("");
            let db_path =
                workspace_root().join(report["database_ref"].as_str().expect("database_ref"));
            if !db_path.is_file() {
                eprintln!(
                    "P041 readback: skipping fixture '{fixture_id}' — engine-side replay DB \
                     {} is missing (likely cleaned between runs).",
                    db_path.display()
                );
                return;
            }
            let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
                .await
                .expect("open P041 fixture DB");
            let schema = build_schema(
                pool.clone(),
                make_command_handler(pool.clone()),
                event_bus::new_bus(64),
                auth::PrincipalTable::test_fixture(),
                test_reporter(),
            );
            let response = schema
                .execute(
                    Request::new(format!(
                        r#"
                    query P041FixtureReadback {{
                      run(id: "{run_id}") {{
                        id
                        status
                        workflowId
                      }}
                      runs(ideaId: "{idea_id}") {{
                        id
                        totalStages
                        completedStages
                        failedStages
                        pendingApprovals
                      }}
                      stages(runId: "{run_id}") {{
                        stageId
                        label
                        status
                      }}
                      artifacts(runId: "{run_id}") {{
                        name
                        contractId
                        reportKind
                      }}
                      runQueueSummary(runId: "{run_id}") {{
                        runId
                        pending
                        running
                        completed
                        failed
                        cancelled
                        total
                      }}
                      stageQueueSummary(stageExecutionId: "{first_stage_exec_id}") {{
                        stageExecutionId
                        pending
                        running
                        completed
                        failed
                        cancelled
                        total
                      }}
                    }}
                    "#
                    ))
                    .data(test_principal()),
                )
                .await;
            assert!(
                response.errors.is_empty(),
                "P041 GraphQL fixture readback query must succeed for {fixture_id}: {response:?}"
            );
            let data = response.data.into_json().unwrap();
            let actual = normalize_p041_graphql_actual(data, run_id, first_stage_exec_id);
            update_p041_surface(
                &mut report,
                "graphql_readback",
                actual,
                "graphql-server::schema::build_schema",
            );
            write_p041_report(fixture_id, &report);
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("graphql crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn control_plane_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("graphql crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn p041_report_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/reports")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("behavioral-diff-report.json")
    }

    fn p041_replay_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/work")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("server-replay.json")
    }

    fn p041_generation_id() -> String {
        let generation_id = std::env::var("P041_PUBLICATION_GENERATION_ID")
            .unwrap_or_else(|_| "unscoped-fixture-replay".to_string());
        assert_safe_p041_generation_id(&generation_id);
        generation_id
    }

    fn assert_safe_p041_generation_id(raw: &str) {
        if raw == "unscoped-fixture-replay" {
            return;
        }
        let valid_prefix = raw.starts_with("p041-");
        let valid_chars = raw
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | 'T' | 'Z'));
        assert!(
            valid_prefix
                && valid_chars
                && !raw.contains("..")
                && !raw.contains('/')
                && !raw.contains('\\'),
            "P041_PUBLICATION_GENERATION_ID must be a safe path segment"
        );
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read JSON")).expect("parse JSON")
    }

    fn p041_report(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_report_path(fixture_id))
    }

    fn p041_replay(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_replay_path(fixture_id))
    }

    fn write_p041_report(fixture_id: &str, report: &serde_json::Value) {
        fs::write(
            p041_report_path(fixture_id),
            serde_json::to_string_pretty(report).expect("serialize P041 report"),
        )
        .expect("write P041 report");
    }

    fn normalize_p041_graphql_actual(
        data: serde_json::Value,
        run_id: &str,
        first_stage_exec_id: &str,
    ) -> serde_json::Value {
        let mut stages = data["stages"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|stage| {
                serde_json::json!({
                    "stage_id": stage["stageId"],
                    "label": stage["label"],
                    "status": stage["status"],
                })
            })
            .collect::<Vec<_>>();
        stages.sort_by(|left, right| {
            left["stage_id"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["stage_id"].as_str().unwrap_or_default())
        });
        // Exclude P057 system projection exports (active-index.json,
        // run-state.json). They are supplemental infrastructure artifacts that
        // post-date the golden fixtures and are not agent-produced outputs.
        const P057_SYSTEM_ARTIFACTS: &[&str] = &["active-index.json", "run-state.json"];
        let mut artifacts = data["artifacts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|artifact| {
                !P057_SYSTEM_ARTIFACTS.contains(&artifact["name"].as_str().unwrap_or_default())
            })
            .map(|artifact| {
                serde_json::json!({
                    "name": artifact["name"],
                    "contract_id": artifact["contractId"],
                    "report_kind": artifact["reportKind"],
                })
            })
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            left["name"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["name"].as_str().unwrap_or_default())
        });
        // Normalize queue summaries by comparing only active (pending/running)
        // counts. Exact completed totals vary by fixture, but active counts must
        // be zero for any terminal run.
        let run_qs = &data["runQueueSummary"];
        let normalized_run_queue_summary = serde_json::json!({
            "run_id": "$run_id",
            "pending": run_qs["pending"],
            "running": run_qs["running"],
        });
        let stage_qs = &data["stageQueueSummary"];
        let normalized_stage_queue_summary = serde_json::json!({
            "stage_execution_id": if first_stage_exec_id.is_empty() {
                serde_json::json!("$first_stage_id")
            } else {
                serde_json::json!("$first_stage_id")
            },
            "pending": stage_qs["pending"],
            "running": stage_qs["running"],
        });
        serde_json::json!({
            "collector_owner": "graphql-server::schema::build_schema",
            "query": "P041FixtureReadback",
            "run": {
                "id": "$run_id",
                "status": data["run"]["status"],
                "workflow_id": data["run"]["workflowId"],
            },
            "runs_by_idea": data["runs"].as_array().cloned().unwrap_or_default().into_iter().map(|run| {
                serde_json::json!({
                    "id": if run["id"] == serde_json::json!(run_id) { serde_json::json!("$run_id") } else { run["id"].clone() },
                    "total_stages": run["totalStages"],
                    "completed_stages": run["completedStages"],
                    "failed_stages": run["failedStages"],
                    "pending_approvals": run["pendingApprovals"],
                })
            }).collect::<Vec<_>>(),
            "stages": stages,
            "artifacts": artifacts,
            "run_queue_summary": normalized_run_queue_summary,
            "stage_queue_summary": normalized_stage_queue_summary,
        })
    }

    fn update_p041_surface(
        report: &mut serde_json::Value,
        surface: &str,
        actual: serde_json::Value,
        collector_owner: &str,
    ) {
        let comparisons = report["surface_comparisons"]
            .as_array_mut()
            .expect("surface_comparisons");
        let comparison = comparisons
            .iter_mut()
            .find(|item| item["surface"] == serde_json::json!(surface))
            .expect("surface comparison");
        let expected = comparison["expected"].clone();
        let matched = expected == actual;
        comparison["actual"] = actual.clone();
        comparison["collector_owner"] = serde_json::json!(collector_owner);
        comparison["status"] = serde_json::json!(if matched { "matched" } else { "diverged" });

        let divergences = report["divergences"].as_array_mut().expect("divergences");
        divergences.retain(|item| item["owner_surface"] != serde_json::json!(surface));
        if !matched {
            divergences.push(serde_json::json!({
                "path": format!("$.{surface}"),
                "expected": expected,
                "actual": actual,
                "severity": "blocking",
                "owner_surface": surface,
                "investigation_hint": "P041 fixture-bound GraphQL readback diverged from expected client truth."
            }));
        }
        let blocking_count = divergences
            .iter()
            .filter(|item| item["severity"] == "blocking")
            .count();
        report["summary"]["blocking_count"] = serde_json::json!(blocking_count);
        report["verdict"] = serde_json::json!(if blocking_count == 0 { "ready" } else { "red" });
    }

    // ── P050 GraphQL readback proof ──

    #[tokio::test]
    async fn test_graphql_run_exposes_chainworks_meta_root() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "P050 GraphQL proof".into(),
                body: "body".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();

        let mut run = make_run(run_id, idea_id);
        run.chainworks_meta_root = Some(format!(".chainworks/runs/{run_id}"));
        runs::insert(&pool, &run).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                async_graphql::Request::new(format!(
                    r#"{{ run(id: "{}") {{ chainworksMetaRoot }} }}"#,
                    run_id
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            response.errors.is_empty(),
            "GraphQL errors: {:?}",
            response.errors
        );
        let data = response.data.into_json().unwrap();
        let meta_root = data["run"]["chainworksMetaRoot"].as_str();
        assert!(
            meta_root.is_some(),
            "GraphQL run query must expose chainworksMetaRoot"
        );
        assert!(
            meta_root.unwrap().contains(".chainworks/runs/"),
            "chainworksMetaRoot must contain per-run path, got: {:?}",
            meta_root
        );
    }

    // ───────────────────────────────────────────────────────────────────
    // Proposal 029 §4.4.b / §9.1 — `journalId` surfacing on mutations
    // ───────────────────────────────────────────────────────────────────
    //
    // Every GraphQL mutation that invokes `CommandHandler` returns a
    // dedicated payload wrapper that exposes `journalId: ID!`. These tests
    // cover the success path for every command mutation plus two denial
    // paths:
    //
    //   - `test_graphql_start_run_started_variant_includes_journal_id`
    //   - `test_graphql_start_run_blocked_variant_includes_journal_id`
    //   - `test_graphql_approve_stage_returns_payload_with_approval_and_journal_id`
    //   - `test_graphql_retry_stage_returns_payload_with_retried_and_journal_id`
    //   - `test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id`
    //   - `test_response_omits_journal_id_when_capability_check_fails`
    //
    // See also AC-11 at proposal §8.

    use db::repos::approvals;
    use domain::approval::{Approval, ApprovalDecision};
    use domain::ids::{ApprovalId, StageExecutionId};
    use domain::stage::{StageExecution, StageStatus};

    fn make_approval(run_id: RunId, stage_id: &str) -> Approval {
        Approval {
            id: ApprovalId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            decision: ApprovalDecision::Pending,
            requested_at: Utc::now(),
            decided_at: None,
            comment: None,
            expires_at: None,
        }
    }

    fn make_manual_gate_stage(run_id: RunId, stage_id: &str) -> StageExecution {
        StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            label: stage_id.to_string(),
            status: StageStatus::WaitingApproval,
            iteration: 0,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: Some("manual_gate".into()),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        }
    }

    fn p072_principal_table() -> (auth::PrincipalTable, auth::Principal, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "schema_version": 2,
              "principals": [
                {
                  "token": "default-token",
                  "id": "default-operator",
                  "class": "operator",
                  "surface_policies": {
                    "graphql": {
                      "allow_queries": true,
                      "allow_subscriptions": true,
                      "allowed_mutations": ["approveApproval", "rejectApproval"]
                    },
                    "mcp": {
                      "allowed_tools": ["runs.list", "runs.get"]
                    }
                  }
                },
                {
                  "token": "ui-token",
                  "id": "ui_operator",
                  "class": "operator",
                  "surface_policies": {
                    "graphql": {
                      "allow_queries": true,
                      "allow_subscriptions": true,
                      "allowed_mutations": ["approveApproval", "rejectApproval"]
                    },
                    "mcp": {
                      "allowed_tools": []
                    }
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let default_operator = auth::resolve_bearer("default-token", &table).unwrap();
        let ui_operator = auth::resolve_bearer("ui-token", &table).unwrap();
        (table, default_operator, ui_operator)
    }

    fn observer_principal() -> auth::Principal {
        auth::Principal::new("test-observer", auth::PrincipalClass::Observer)
    }

    fn p072_legacy_default_operator_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-default-token",
                  "id": "default-operator",
                  "class": "operator"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-default-token", &table).unwrap();
        (table, principal)
    }

    fn p072_legacy_custom_operator_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-custom-token",
                  "id": "custom-operator",
                  "class": "operator"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-custom-token", &table).unwrap();
        (table, principal)
    }

    fn p072_legacy_agent_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-agent-token",
                  "id": "legacy-agent",
                  "class": "agent"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-agent-token", &table).unwrap();
        (table, principal)
    }

    #[tokio::test]
    async fn test_graphql_approve_approval_uses_p072_ui_operator_policy() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let allowed_with_default_operator = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(default_operator),
            )
            .await;
        assert!(
            allowed_with_default_operator.errors.is_empty(),
            "default-operator is the app bearer and must allow approveApproval: {allowed_with_default_operator:?}"
        );

        let ui_approval = make_approval(run_id, "state_6");
        let ui_approval_id = ui_approval.id;
        approvals::insert(&pool, &ui_approval).await.unwrap();

        let allowed = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{
                          id
                          decision
                          availableActions
                          disabledReasonCode
                          writePathState
                        }}
                        journalId
                      }}
                    }}"#,
                    ui_approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "ui_operator approveApproval must succeed: {allowed:?}"
        );
        let data = allowed.data.into_json().unwrap();
        let approval = &data["approveApproval"]["approval"];
        assert_eq!(
            approval["id"],
            serde_json::json!(ui_approval_id.to_string())
        );
        assert_eq!(approval["decision"], serde_json::json!("granted"));
        assert_eq!(approval["availableActions"], serde_json::json!([]));
        assert_eq!(
            approval["writePathState"],
            serde_json::json!("write_path_not_available")
        );
        assert_eq!(
            approval["disabledReasonCode"],
            serde_json::json!("UNSUPPORTED_ACTION")
        );
        assert!(
            data["approveApproval"]["journalId"].is_string(),
            "approveApproval must return journalId"
        );
    }

    #[tokio::test]
    async fn test_graphql_ui_principals_denied_non_approval_mutations() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let cases = [
            "mutation { startRun }".to_string(),
            "mutation { retryStage }".to_string(),
            "mutation { cancelRun }".to_string(),
        ];

        for principal in [default_operator, ui_operator] {
            for query in cases.clone() {
                let response = schema
                    .execute(Request::new(query).data(principal.clone()))
                    .await;
                assert!(
                    !response.errors.is_empty(),
                    "P072 UI principals must be denied non-approval mutation: {response:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_graphql_legacy_default_operator_denied_non_approval_mutations() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let (principal_table, principal) = p072_legacy_default_operator_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let response = schema
            .execute(Request::new("mutation { startRun }").data(principal))
            .await;

        assert!(
            !response.errors.is_empty(),
            "legacy default-operator must not see removed startRun GraphQL mutation: {response:?}"
        );
    }

    #[tokio::test]
    async fn test_graphql_missing_graphql_surface_policy_principals_denied_non_approval_mutations()
    {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let query = "mutation { startRun }";

        for (principal_table, principal, label) in [
            {
                let (table, principal) = p072_legacy_custom_operator_table();
                (table, principal, "custom operator")
            },
            {
                let (table, principal) = p072_legacy_agent_table();
                (table, principal, "agent")
            },
        ] {
            let schema = build_schema(
                pool.clone(),
                make_command_handler(pool.clone()),
                event_bus::new_bus(64),
                principal_table,
                test_reporter(),
            );

            let response = schema.execute(Request::new(query).data(principal)).await;
            assert!(
                !response.errors.is_empty(),
                "{label} must not see removed startRun GraphQL mutation: {response:?}"
            );

            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM command_journal WHERE command_type = 'StartRun'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                count, 0,
                "{label} denial must not create a command_journal row"
            );
        }
    }

    #[tokio::test]
    async fn test_graphql_approve_approval_rejects_missing_or_resolved_approval() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let missing_id = ApprovalId::new();
        let missing = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    missing_id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            !missing.errors.is_empty(),
            "missing approval must return a GraphQL error"
        );

        let first = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{ id decision }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            first.errors.is_empty(),
            "first approveApproval must succeed: {first:?}"
        );

        let second = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{ id decision }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            second.errors.is_empty(),
            "already-resolved approval must return a typed conflict code, not a GraphQL error: {second:?}"
        );
        assert_eq!(
            second.data.into_json().unwrap()["approveApproval"]["conflictResultCode"],
            serde_json::json!("already_resolved"),
            "already-resolved approval must return already_resolved conflict code"
        );
    }

    #[tokio::test]
    async fn proposal_085_approval_conflict_result_code_uses_real_failed_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let approve = |fields: &str| {
            Request::new(format!(
                r#"mutation {{
                  approveApproval(approvalId: "{}") {{
                    {fields}
                  }}
                }}"#,
                approval.id
            ))
            .data(ui_operator.clone())
        };

        let first = schema
            .execute(approve(
                "approval { id decision } journalId conflictResultCode",
            ))
            .await;
        assert!(
            first.errors.is_empty(),
            "first approveApproval must succeed: {first:?}"
        );
        let first_json = first.data.into_json().unwrap();
        assert_eq!(
            first_json["approveApproval"]["conflictResultCode"],
            serde_json::Value::Null
        );

        let second = schema
            .execute(approve(
                "approval { id decision } journalId conflictResultCode",
            ))
            .await;
        assert!(
            second.errors.is_empty(),
            "already-resolved approval must return typed conflict payload: {second:?}"
        );
        let second_json = second.data.into_json().unwrap();
        let payload = &second_json["approveApproval"];
        assert_eq!(
            payload["conflictResultCode"],
            serde_json::json!("already_resolved")
        );
        assert_eq!(
            payload["approval"]["decision"],
            serde_json::json!("granted")
        );
        let journal_id = payload["journalId"]
            .as_str()
            .expect("conflict payload must include journalId");
        assert_ne!(
            journal_id, "00000000-0000-0000-0000-000000000000",
            "conflict journalId must be the real failed command journal row"
        );
        uuid::Uuid::parse_str(journal_id).expect("conflict journalId must be a UUID");

        let row: (String, String) =
            sqlx::query_as("SELECT result_status, command_type FROM command_journal WHERE id = ?1")
                .bind(journal_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "failed");
        assert_eq!(row.1, "ResolveApproval");
    }

    #[tokio::test]
    async fn proposal_085_conflict_enum_matches_backend_emitted_codes() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P085ConflictEnumContract {
                      conflict: __type(name: "MutationConflictResultCode") {
                        enumValues { name }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P085 mutation conflict enum introspection must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_enum_values(&json, "conflict", &["already_resolved"]);
    }

    #[tokio::test]
    async fn proposal_085_reject_conflict_result_code_uses_real_failed_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "state_6"))
            .await
            .unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let approve = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}") {{
                        approval {{ id decision }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            approve.errors.is_empty(),
            "initial approveApproval must succeed before reject conflict proof: {approve:?}"
        );

        let reject = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "stale reject") {{
                        approval {{ id decision availableActions disabledReasonCode writePathState }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            reject.errors.is_empty(),
            "rejectApproval on resolved approval must return typed conflict payload: {reject:?}"
        );
        let json = reject.data.into_json().unwrap();
        let payload = &json["rejectApproval"];
        assert_eq!(
            payload["conflictResultCode"],
            serde_json::json!("already_resolved")
        );
        assert_eq!(
            payload["approval"]["decision"],
            serde_json::json!("granted")
        );
        assert_eq!(
            payload["approval"]["availableActions"],
            serde_json::json!([])
        );
        assert_eq!(
            payload["approval"]["disabledReasonCode"],
            serde_json::json!("UNSUPPORTED_ACTION")
        );
        assert_eq!(
            payload["approval"]["writePathState"],
            serde_json::json!("write_path_not_available")
        );
        let journal_id = payload["journalId"]
            .as_str()
            .expect("reject conflict payload must include journalId");
        assert_ne!(journal_id, "00000000-0000-0000-0000-000000000000");

        let row: (String, String) =
            sqlx::query_as("SELECT result_status, command_type FROM command_journal WHERE id = ?1")
                .bind(journal_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "failed");
        assert_eq!(row.1, "ResolveApproval");
    }

    #[tokio::test]
    async fn proposal_085_backend_artifact_projection_state_matrix() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p085-affordance-matrix-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let outside_root =
            std::env::temp_dir().join(format!("p085-affordance-outside-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&outside_root).unwrap();
        let outside_path = outside_root.join("outside.md");
        fs::write(&outside_path, "outside payload").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "release-report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: artifact_root
                    .join("report.json")
                    .to_string_lossy()
                    .into_owned(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        for index in 0..10 {
            let path = artifact_root.join(format!("payload-{index}.md"));
            fs::write(&path, format!("payload {index}")).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: ArtifactId::new(),
                    run_id,
                    stage_id: "artifact".into(),
                    agent_id: "writer".into(),
                    name: format!("payload-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    size_bytes: Some(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES as i64),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }

        let unavailable_id = ArtifactId::new();
        artifacts::insert(
            &pool,
            &Artifact {
                id: unavailable_id,
                run_id,
                stage_id: "artifact".into(),
                agent_id: "writer".into(),
                name: "outside.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: outside_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(16),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let list = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P085ArtifactProjectionStateMatrix {{
                      artifacts(runId: "{run_id}") {{
                        name
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        freshnessState
                        diagnosticId
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            list.errors.is_empty(),
            "P085 artifact projection state matrix query must succeed: {list:?}"
        );
        let data = list.data.into_json().unwrap();
        let artifacts = data["artifacts"].as_array().unwrap();
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("available")
                && artifact["payloadText"].is_string()
                && artifact["freshnessState"] == serde_json::json!("live")
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("metadata_only")
                && artifact["payloadUnavailableReasonCode"]
                    == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["diagnosticId"].is_string()
                && artifact["serverDebugDetail"].is_string()
                && artifact["serverDebugDetail"]
                    .as_str()
                    .is_some_and(|detail| detail.contains(P085_NO_DEADLINE_JUSTIFICATION))
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("payload_deferred")
                && artifact["payloadUnavailableReasonCode"]
                    == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["serverDebugDetail"]
                    .as_str()
                    .is_some_and(|detail| {
                        detail.contains("payload preview budget")
                            && detail.contains(P085_NO_DEADLINE_JUSTIFICATION)
                    })
        }));

        let detail = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P085UnavailableArtifactDetail {{
                      artifact(id: "{unavailable_id}") {{
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            detail.errors.is_empty(),
            "P085 unavailable artifact detail query must succeed: {detail:?}"
        );
        let detail_json = detail.data.into_json().unwrap();
        let artifact = &detail_json["artifact"];
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("unavailable")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("NOT_AVAILABLE")
        );
        assert!(artifact["payloadText"].is_null());
        assert!(artifact["serverDebugDetail"]
            .as_str()
            .is_some_and(|detail| detail.contains("outside the selected run")));

        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[tokio::test]
    async fn proposal_085_graphql_backend_projection_and_authorization_contract() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = ApprovalId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "stage_1"))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "release".into(),
                name: "release-report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/release-report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(128),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let query = format!(
            r#"
            query P085BackendAffordanceProof {{
              approvalInbox(runId: "{run_id}") {{
                id
                decision
                availableActions
                disabledReasonCode
                writePathState
                freshnessState
                diagnosticId
                serverDebugDetail
              }}
              artifacts(runId: "{run_id}") {{
                id
                payloadAvailabilityState
                payloadUnavailableReasonCode
                freshnessState
                diagnosticId
                serverDebugDetail
              }}
            }}
            "#
        );
        let allowed = schema
            .execute(Request::new(query.clone()).data(ui_operator))
            .await;
        assert!(
            allowed.errors.is_empty(),
            "P085 backend projection query must succeed for UI operator: {allowed:?}"
        );
        let json = allowed.data.into_json().unwrap();
        let approval = &json["approvalInbox"][0];
        assert_eq!(approval["id"], serde_json::json!(approval_id.to_string()));
        assert_eq!(approval["decision"], serde_json::json!("pending"));
        assert_eq!(
            approval["availableActions"],
            serde_json::json!(["approve", "reject"])
        );
        assert_eq!(approval["writePathState"], serde_json::json!("available"));
        assert_eq!(approval["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            approval["diagnosticId"],
            serde_json::json!(approval_id.to_string())
        );
        assert!(approval["serverDebugDetail"].is_null());

        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("metadata_only")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
        );
        assert_eq!(artifact["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            artifact["diagnosticId"],
            serde_json::json!(artifact_id.to_string())
        );
        assert!(
            artifact["serverDebugDetail"].is_string(),
            "server-owned diagnostic detail should explain deferred report payload"
        );

        let denied = schema
            .execute(Request::new(query).data(observer_principal()))
            .await;
        assert!(
            denied
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "P085 diagnostic fields must be denied to unauthorized observers: {denied:?}"
        );
    }

    #[tokio::test]
    async fn test_graphql_reject_approval_uses_p072_ui_operator_policy() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let allowed_with_default_operator = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "needs more work") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(default_operator),
            )
            .await;
        assert!(
            allowed_with_default_operator.errors.is_empty(),
            "default-operator is the app bearer and must allow rejectApproval: {allowed_with_default_operator:?}"
        );

        let ui_approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &ui_approval).await.unwrap();

        let allowed = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "needs more work") {{
                        approval {{ id decision }}
                        journalId
                      }}
                    }}"#,
                    ui_approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "ui_operator rejectApproval must succeed: {allowed:?}"
        );
        let data = allowed.data.into_json().unwrap();
        assert_eq!(
            data["rejectApproval"]["approval"]["decision"],
            serde_json::json!("rejected")
        );
        assert!(
            data["rejectApproval"]["journalId"].is_string(),
            "rejectApproval must return journalId"
        );
    }

    #[tokio::test]
    async fn test_p072_ui_principals_allow_queries() {
        // The app uses one operator bearer, so P072 UI principals must support
        // read operations and approval-only mutations on the same GraphQL surface.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table.clone(),
            test_reporter(),
        );

        let ui_allowed = schema
            .execute(
                Request::new(format!(r#"{{ run(id: "{}") {{ id }} }}"#, run_id))
                    .data(ui_operator.clone()),
            )
            .await;
        assert!(
            ui_allowed.errors.is_empty(),
            "ui_operator query must succeed: {ui_allowed:?}"
        );

        let (_, default_operator, _) = p072_principal_table();
        let allowed = schema
            .execute(
                Request::new(format!(r#"{{ run(id: "{}") {{ id }} }}"#, run_id))
                    .data(default_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "default-operator query must succeed: {allowed:?}"
        );
    }

    // ── P042 §5.2: daemonStatus query + daemonStatusChanged subscription ──

    fn operator_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    #[tokio::test]
    async fn test_daemon_status_query_includes_build_sha_and_schema_versions() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "cafe-babe", bus.clone());
        reporter.set_state(domain::lifecycle::DaemonLifecycleState::Starting);
        reporter.set_state(domain::lifecycle::DaemonLifecycleState::Ready);
        reporter.set_xcode_broker_health(domain::lifecycle::XcodeBrokerHealthSnapshot {
            state: domain::lifecycle::XcodeBrokerHealthState::Degraded,
            reason_code: "xcode_mcp_capacity_backpressure".to_string(),
            can_acquire_new_xcode_leases: false,
            active_lease_count: 2,
            initialize_queue_depth: 8,
            last_transition_at: "2026-04-25T09:00:00Z".to_string(),
            operator_message: "Xcode MCP bridge pool is applying capacity backpressure."
                .to_string(),
            pool_id: "pool-test".to_string(),
            active_leases: 2,
            queued_leases: 8,
            max_active_leases: 2,
            max_queued_leases: 8,
            broker_disabled: false,
            backend_available: true,
            observation_persistence_failures: 0,
            stale_lease_count: 1,
            backend_session_count: 1,
            helper_cleanup_reaped_leases_total: 2,
        });

        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(
                Request::new(
                    r#"{
                      daemonStatus {
                        state schemaVersion binarySchemaVersion buildSha
                        pid lastStateChangeAt json
                        xcodeBrokerHealth {
                          state reasonCode canAcquireNewXcodeLeases
                          activeLeaseCount initializeQueueDepth lastTransitionAt
                          operatorMessage poolId activeLeases queuedLeases
                          maxActiveLeases maxQueuedLeases brokerDisabled
                          backendAvailable observationPersistenceFailures
                          staleLeaseCount backendSessionCount helperCleanupReapedLeasesTotal
                        }
                      }
                    }"#,
                )
                .data(operator_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "daemonStatus errored: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        // GraphQL enums serialize as SCREAMING_SNAKE_CASE per spec.
        assert_eq!(data["daemonStatus"]["state"], "READY");
        assert_eq!(data["daemonStatus"]["binarySchemaVersion"], 14);
        assert_eq!(data["daemonStatus"]["buildSha"], "cafe-babe");
        let health = &data["daemonStatus"]["xcodeBrokerHealth"];
        assert_eq!(health["state"], "DEGRADED");
        assert_eq!(health["reasonCode"], "xcode_mcp_capacity_backpressure");
        assert_eq!(health["canAcquireNewXcodeLeases"], false);
        assert_eq!(health["activeLeaseCount"], 2);
        assert_eq!(health["initializeQueueDepth"], 8);
        assert_eq!(health["lastTransitionAt"], "2026-04-25T09:00:00Z");
        assert_eq!(
            health["operatorMessage"],
            "Xcode MCP bridge pool is applying capacity backpressure."
        );
        assert_eq!(health["poolId"], "pool-test");
        assert_eq!(health["activeLeases"], 2);
        assert_eq!(health["queuedLeases"], 8);
        assert_eq!(health["maxActiveLeases"], 2);
        assert_eq!(health["maxQueuedLeases"], 8);
        assert_eq!(health["brokerDisabled"], false);
        assert_eq!(health["backendAvailable"], true);
        assert_eq!(health["observationPersistenceFailures"], 0);
        assert_eq!(health["staleLeaseCount"], 1);
        assert_eq!(health["backendSessionCount"], 1);
        assert_eq!(health["helperCleanupReapedLeasesTotal"], 2);
        // The json field carries the full P042 §5.2 wire shape (snake_case).
        let json_str = data["daemonStatus"]["json"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["state"], "ready");
        assert_eq!(parsed["binary_schema_version"], 14);
        assert_eq!(
            parsed["xcode_broker_health"]["reason_code"],
            "xcode_mcp_capacity_backpressure"
        );
        assert_eq!(
            parsed["xcode_broker_health"]["can_acquire_new_xcode_leases"],
            false
        );
        assert_eq!(parsed["xcode_broker_health"]["stale_lease_count"], 1);
    }

    #[tokio::test]
    async fn daemon_status_query_is_operator_only() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(Request::new("{ daemonStatus { state } }").data(observer_principal()))
            .await;
        assert!(
            response
                .errors
                .iter()
                .any(|e| e.message.contains("forbidden")),
            "observer must be denied, got {response:?}"
        );
    }

    #[tokio::test]
    async fn daemon_status_query_populates_failure_field_when_failed() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        reporter.set_failed(
            domain::lifecycle::FailureKind::MigrationFailed,
            "test failure",
            Some("/tmp/bk.sqlite".into()),
        );
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(
                Request::new("{ daemonStatus { state failure { kind detail backupPath } json } }")
                    .data(operator_principal()),
            )
            .await;
        assert!(response.errors.is_empty(), "{response:?}");
        let data = response.data.into_json().unwrap();
        assert_eq!(data["daemonStatus"]["state"], "FAILED");
        // Typed `failure` field is now first-class GraphQL (not nested in
        // a stringified json). `kind` is a GraphQL enum, so it serializes
        // as SCREAMING_SNAKE_CASE.
        assert_eq!(data["daemonStatus"]["failure"]["kind"], "MIGRATION_FAILED");
        assert_eq!(data["daemonStatus"]["failure"]["detail"], "test failure");
        assert_eq!(
            data["daemonStatus"]["failure"]["backupPath"],
            "/tmp/bk.sqlite"
        );
        // `json` retains the snake_case wire shape identical to /health.
        let json_str = data["daemonStatus"]["json"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["failure"]["kind"], "migration_failed");
        assert_eq!(parsed["failure"]["backup_path"], "/tmp/bk.sqlite");
    }

    #[tokio::test]
    async fn daemon_status_changed_subscription_receives_transitions() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        let mut stream = schema.execute_stream(
            Request::new("subscription { daemonStatusChanged { state } }")
                .data(operator_principal()),
        );
        // The BroadcastStream only observes frames sent AFTER
        // `events.subscribe()` runs inside the subscription handler, which
        // only runs when the stream is polled. Kick the transition from a
        // spawned task with a small delay so the first poll activates the
        // subscription before the frame is broadcast.
        let reporter_clone = reporter.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            reporter_clone.set_state(domain::lifecycle::DaemonLifecycleState::Starting);
        });
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("subscription frame timed out")
            .expect("subscription stream ended");
        let data = frame.data.into_json().unwrap();
        assert_eq!(data["daemonStatusChanged"]["state"], "STARTING");
    }

    #[tokio::test]
    async fn test_daemon_status_changed_subscription_auth_required() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        // No principal data inserted into the request — this mirrors what
        // happens when WS `connection_init` is missing or rejects the
        // token. The subscription handler must refuse with "unauthorized"
        // on first poll, not silently pass through frames.
        let mut stream = schema.execute_stream(Request::new(
            "subscription { daemonStatusChanged { state } }",
        ));
        let frame = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .expect("unauthorized subscription should produce a frame, not hang")
            .expect("stream should not end before emitting the error");
        assert!(
            !frame.errors.is_empty(),
            "subscription without principal must surface an error, got {frame:?}"
        );
        assert!(
            frame
                .errors
                .iter()
                .any(|e| e.message.to_lowercase().contains("unauthorized")),
            "error message must mention 'unauthorized': {:?}",
            frame.errors
        );
    }

    #[tokio::test]
    async fn test_daemon_status_changed_subscription_rejects_non_operator_principal() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        // Observer class — has a principal but is not Operator. P042 §5.2
        // marks the subscription as operator-only bearer auth; the handler
        // must refuse with `forbidden`, not stream frames.
        let mut stream = schema.execute_stream(
            Request::new("subscription { daemonStatusChanged { state } }")
                .data(observer_principal()),
        );
        let frame = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .expect("observer subscription should produce a frame, not hang")
            .expect("stream should not end before emitting the error");
        assert!(
            !frame.errors.is_empty(),
            "observer subscription must surface an error, got {frame:?}"
        );
        assert!(
            frame
                .errors
                .iter()
                .any(|e| e.message.to_lowercase().contains("forbidden")),
            "error must mention 'forbidden' for the observer class: {:?}",
            frame.errors
        );
    }
}
