use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_graphql::futures_util::StreamExt;
use async_graphql::*;
use sqlx::SqlitePool;
use tokio_stream::wrappers::BroadcastStream;

use db::repos::{
    approvals, artifact_contracts, ideas, projections, runs, steward as steward_repo,
    workflow_conflicts,
};
use domain::commands::{
    ApproveStageCmd, CallerContext, CancelRunCmd, Command, OverrideLegacyDiscoveryPolicyCmd,
    RejectStageCmd, RetryStageCmd, StartRunCmd,
};
use domain::discovery::LegacyBroadDiscoveryPolicy;
use domain::events::DomainEvent;
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::lifecycle::DaemonStatus;
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;
use engine::lifecycle_reporter::LifecycleReporter;

use crate::types::approval::GqlApproval;
use crate::types::artifact::GqlArtifact;
use crate::types::idea::GqlIdea;
use crate::types::p031::{GqlPayloadAvailabilityState, GqlPayloadUnavailableReasonCode};
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
    reporter: LifecycleReporter,
) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .data(principal_table)
        .data(reporter)
        .finish()
}

pub struct QueryRoot;

fn require_operator_read(ctx: &Context<'_>) -> Result<()> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;
    if principal.class != auth::PrincipalClass::Operator {
        return Err(Error::new("forbidden"));
    }
    Ok(())
}

fn parse_legacy_broad_discovery_policy(value: &str) -> Result<LegacyBroadDiscoveryPolicy> {
    match value {
        "workflow_opt_in" => Ok(LegacyBroadDiscoveryPolicy::WorkflowOptIn),
        "disabled" => Ok(LegacyBroadDiscoveryPolicy::Disabled),
        _ => Err(Error::new(format!(
            "unknown legacy_discovery_override_policy: {value}"
        ))),
    }
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
    gql.workflow_conflict = workflow_conflicts::get_current_blocking_conflict(pool, run_id)
        .await?
        .map(Into::into);
    // P017: Enrich workflow conflict with lead mediation readback if present.
    if let Some(ref mut conflict) = gql.workflow_conflict {
        if let Some(ref mediation_id) = conflict.mediation_record_id {
            if let Ok(Some(med)) =
                db::repos::lead_conflict_mediations::find_by_id(pool, mediation_id).await
            {
                conflict.lead_mediation = Some(crate::types::run::GqlLeadMediation::from(&med));
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
        let mut bulk_preview_budget_remaining = P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES;
        Ok(items
            .into_iter()
            .map(|row| {
                let mut artifact = GqlArtifact::from(row.clone());
                attach_p031_artifact_payload(
                    &row,
                    run.as_ref(),
                    &mut artifact,
                    &mut bulk_preview_budget_remaining,
                );
                artifact
            })
            .collect())
    }

    async fn stages(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlStageExecution>> {
        require_operator_read(ctx)?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_stages_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
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
    run.workflow_conflict = workflow_conflicts::get_current_blocking_conflict(pool, run_id)
        .await?
        .map(Into::into);
    // P017: Enrich workflow conflict with lead mediation readback if present.
    if let Some(ref mut conflict) = run.workflow_conflict {
        if let Some(ref mediation_id) = conflict.mediation_record_id {
            if let Ok(Some(med)) =
                db::repos::lead_conflict_mediations::find_by_id(pool, mediation_id).await
            {
                conflict.lead_mediation = Some(crate::types::run::GqlLeadMediation::from(&med));
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
    Ok(run)
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
    if artifact.report_kind.is_some() || row.format == "report" {
        return;
    }

    let estimated_preview_bytes = row
        .size_bytes
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| *size > 0)
        .map(|size| size.min(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES))
        .unwrap_or(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
    if estimated_preview_bytes > *bulk_preview_budget_remaining {
        mark_payload_deferred(
            artifact,
            "Artifact payload preview deferred because the bulk artifact list reached its payload preview budget",
        );
        return;
    }

    let Some(run) = run else {
        mark_payload_unavailable(
            artifact,
            "Run metadata was unavailable for artifact readback",
        );
        return;
    };

    let Some(path) = resolve_server_owned_artifact_path(&row.file_path, run) else {
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
        }
        Err(err) => {
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
    artifact.server_debug_detail = Some(detail.to_string());
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
    StartRun,
    ApproveStage,
    RejectStage,
    RetryStage,
    OverrideLegacyDiscoveryPolicy,
    CancelRun,
}

pub fn capability_id_for(mutation: MutationName) -> domain::CapabilityToolId {
    match mutation {
        MutationName::StartRun => domain::CapabilityToolId::RunsStart,
        MutationName::ApproveStage | MutationName::RejectStage => {
            domain::CapabilityToolId::ApprovalsResolve
        }
        MutationName::RetryStage | MutationName::OverrideLegacyDiscoveryPolicy => {
            domain::CapabilityToolId::StagesRetry
        }
        MutationName::CancelRun => domain::CapabilityToolId::RunsCancel,
    }
}

fn mutation_allowed(principal: &auth::Principal, mutation: MutationName) -> bool {
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
    pub legacy_discovery_override_id: Option<ID>,
}

#[derive(SimpleObject)]
pub struct OverrideLegacyDiscoveryPolicyPayload {
    pub override_id: ID,
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

        let caller = graphql_caller_with_request_id(ctx, &principal, "startRun");

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
            review_routing_json: None,
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

        let caller = graphql_caller_with_request_id(ctx, &principal, "approveStage");

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

        let caller = graphql_caller_with_request_id(ctx, &principal, "rejectStage");

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
        consume_quota_budget_now: Option<bool>,
        legacy_discovery_override_policy: Option<String>,
        legacy_discovery_override_reason: Option<String>,
    ) -> Result<RetryStagePayload> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::RetryStage) {
            return Err(Error::new("forbidden"));
        }

        let caller = graphql_caller_with_request_id(ctx, &principal, "retryStage");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RetryStage(RetryStageCmd {
            run_id: rid,
            stage_id,
            consume_quota_budget_now: consume_quota_budget_now.unwrap_or(false),
            agent_execution_id: None,
            legacy_discovery_override_policy: legacy_discovery_override_policy
                .as_deref()
                .map(parse_legacy_broad_discovery_policy)
                .transpose()?,
            legacy_discovery_override_reason,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        let legacy_discovery_override_id = match &commanded.result {
            engine::command_handler::CommandResult::StageRetryScheduled {
                legacy_discovery_override_id,
                ..
            } => legacy_discovery_override_id
                .as_ref()
                .map(|id| ID::from(id.clone())),
            _ => None,
        };
        Ok(RetryStagePayload {
            retried: true,
            journal_id: ID::from(commanded.journal_id),
            legacy_discovery_override_id,
        })
    }

    async fn override_legacy_discovery_policy(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        target_stage_execution_id: ID,
        target_attempt_number: i64,
        legacy_discovery_override_policy: String,
        legacy_discovery_override_reason: String,
    ) -> Result<OverrideLegacyDiscoveryPolicyPayload> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        if !mutation_allowed(&principal, MutationName::OverrideLegacyDiscoveryPolicy) {
            return Err(Error::new("forbidden"));
        }

        let caller =
            graphql_caller_with_request_id(ctx, &principal, "overrideLegacyDiscoveryPolicy");

        let rid: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let target_stage_execution_id: StageExecutionId = target_stage_execution_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd {
            run_id: rid,
            stage_id,
            target_stage_execution_id,
            target_attempt_number,
            legacy_discovery_override_policy: parse_legacy_broad_discovery_policy(
                &legacy_discovery_override_policy,
            )?,
            legacy_discovery_override_reason,
        });

        let commanded = cmd_handler.handle(cmd, caller).await?;
        let override_id = match commanded.result {
            engine::command_handler::CommandResult::LegacyDiscoveryOverrideCreated {
                override_id,
            } => override_id,
            _ => return Err(Error::new("Unexpected command result")),
        };
        Ok(OverrideLegacyDiscoveryPolicyPayload {
            override_id: ID::from(override_id),
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

        let caller = graphql_caller_with_request_id(ctx, &principal, "cancelRun");

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
        artifact_contracts, artifacts, ideas, projections, runs, stages, steward,
        workflow_conflicts,
    };
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
            capability_id_for(MutationName::OverrideLegacyDiscoveryPolicy),
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
            chainworks_meta_root: None,
            review_routing_json: None,
        }
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
            test_reporter(),
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
    async fn graphql_start_run_blocked_payload_contract_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
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
            test_reporter(),
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
            test_reporter(),
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
        assert_eq!(
            approval["disabledReasonCode"],
            serde_json::json!("WRITE_PATH_NOT_AVAILABLE")
        );
        assert_eq!(
            approval["writePathState"],
            serde_json::json!("read_only_diagnostic")
        );
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
            test_reporter(),
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
            test_reporter(),
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
            test_reporter(),
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

    #[tokio::test]
    async fn proposal_041_graphql_readback_parity_surfaces() {
        for fixture_id in P041_FIXTURES {
            // The engine crate's `proposal_041_parity.rs` integration
            // test produces `target/parity/reports/<fixture_id>/behavioral-diff-report.json`
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
                .execute(Request::new(format!(
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
                    }}
                    "#
                )))
                .await;
            assert!(
                response.errors.is_empty(),
                "P041 GraphQL fixture readback query must succeed for {fixture_id}: {response:?}"
            );
            let data = response.data.into_json().unwrap();
            let actual = normalize_p041_graphql_actual(data, run_id);
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
            .join(fixture_id)
            .join("behavioral-diff-report.json")
    }

    fn p041_replay_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity")
            .join(fixture_id)
            .join("server-replay.json")
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

    fn normalize_p041_graphql_actual(data: serde_json::Value, run_id: &str) -> serde_json::Value {
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
        let mut artifacts = data["artifacts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
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
            .execute(async_graphql::Request::new(format!(
                r#"{{ run(id: "{}") {{ chainworksMetaRoot }} }}"#,
                run_id
            )))
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

    fn observer_principal() -> auth::Principal {
        auth::Principal::new("test-observer", auth::PrincipalClass::Observer)
    }

    #[tokio::test]
    async fn test_graphql_start_run_started_variant_includes_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-ok","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );

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
                    mutation StartRun {
                      startRun(
                        ideaId: "IDEA_ID",
                        workflowId: "wf-1",
                        workflowTitle: "t",
                        workspaceRoot: "/tmp/ws",
                        artifactRoot: "/tmp/art",
                        workflowYamlPath: "WORKFLOW_YAML_PATH",
                        agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                        deliveryConfigurationJson: DELIVERY_CONFIG
                      ) {
                        ... on StartRunStartedPayload { run { id } journalId }
                        ... on StartRunBlockedPayload { journalId }
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

        assert!(
            response.errors.is_empty(),
            "mutation must succeed: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        let jid = data["startRun"]["journalId"]
            .as_str()
            .expect("StartRunStartedPayload.journalId");
        assert!(
            !jid.is_empty(),
            "journalId on StartRunStartedPayload must be a non-empty uuid"
        );
    }

    #[tokio::test]
    async fn test_graphql_start_run_blocked_variant_includes_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(Request::new(r#"
                mutation StartRun {
                  startRun(
                    ideaId: "IDEA_ID",
                    workflowId: "wf-blk",
                    workflowTitle: "Blocked",
                    workspaceRoot: "/tmp/ws",
                    artifactRoot: "/tmp/art",
                    workflowYamlPath: "WORKFLOW_YAML_PATH",
                    agentCatalogYamlPath: "AGENT_CATALOG_YAML_PATH",
                    deliveryConfigurationJson: "{\"repo_identifier\":\"r\",\"repo_root\":\"/definitely/missing\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp\",\"target_branch\":\"cw/release\",\"release_target_id\":\"app-store\"}"
                  ) {
                    ... on StartRunStartedPayload { run { id } journalId }
                    ... on StartRunBlockedPayload { journalId deliveryPreflight { passed } }
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
            "blocked mutation must surface as payload, not top-level error: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        assert_eq!(
            data["startRun"]["deliveryPreflight"]["passed"],
            serde_json::json!(false),
            "blocked variant must report preflight failure"
        );
        let jid = data["startRun"]["journalId"]
            .as_str()
            .expect("StartRunBlockedPayload.journalId");
        assert!(
            !jid.is_empty(),
            "journalId must be present on the blocked variant too"
        );
    }

    #[tokio::test]
    async fn test_graphql_approve_stage_returns_payload_with_approval_and_journal_id() {
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
                    r#"mutation {{
                      approveStage(runId: "{}", stageId: "state_6") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    run_id
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "approveStage must succeed: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        let approval_id = data["approveStage"]["approval"]["id"]
            .as_str()
            .expect("approveStage.approval.id");
        assert_eq!(approval_id, approval.id.to_string());

        let jid = data["approveStage"]["journalId"]
            .as_str()
            .expect("approveStage.journalId");
        assert!(!jid.is_empty(), "journalId on ApproveStagePayload");
    }

    #[tokio::test]
    async fn test_graphql_retry_stage_returns_payload_with_retried_and_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
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
                    r#"mutation {{
                      retryStage(runId: "{}", stageId: "state_7") {{
                        retried
                        journalId
                      }}
                    }}"#,
                    run_id
                ))
                .data(test_principal()),
            )
            .await;

        // retry_stage may succeed or fail depending on work-queue state —
        // the contract we care about is that the payload wrapper includes
        // journalId whenever it returns (not errors).
        if response.errors.is_empty() {
            let data = response.data.into_json().unwrap();
            assert_eq!(data["retryStage"]["retried"], serde_json::json!(true));
            let jid = data["retryStage"]["journalId"]
                .as_str()
                .expect("retryStage.journalId");
            assert!(!jid.is_empty(), "journalId on RetryStagePayload");
        } else {
            // Even on error, the command_journal row was written. Assert
            // that from the DB directly to prove journal_id was generated.
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM command_journal WHERE command_type = 'RetryStage'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(count, 1, "retryStage must still leave an audit row");
        }
    }

    #[tokio::test]
    async fn test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
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
                    r#"mutation {{
                      cancelRun(runId: "{}") {{
                        cancelled
                        journalId
                      }}
                    }}"#,
                    run_id
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "cancelRun must succeed: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        assert_eq!(data["cancelRun"]["cancelled"], serde_json::json!(true));
        let jid = data["cancelRun"]["journalId"]
            .as_str()
            .expect("cancelRun.journalId");
        assert!(!jid.is_empty(), "journalId on CancelRunPayload");
    }

    #[tokio::test]
    async fn test_response_omits_journal_id_when_capability_check_fails() {
        // Observer class is forbidden from `startRun`. The mutation returns
        // a GraphQL error of kind `forbidden`, and no payload — therefore
        // no `journalId`. Proof: response.data is null/missing for the
        // startRun field, and response.errors[0].message matches "forbidden".
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

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
                    r#"mutation {
                      startRun(
                        ideaId: "IDEA_ID",
                        workflowId: "wf",
                        workflowTitle: "t",
                        workspaceRoot: "/tmp/ws",
                        artifactRoot: "/tmp/art",
                        workflowYamlPath: "WFP",
                        agentCatalogYamlPath: "AGP"
                      ) {
                        ... on StartRunStartedPayload { journalId }
                        ... on StartRunBlockedPayload { journalId }
                      }
                    }"#
                    .replace("IDEA_ID", &idea_id.to_string())
                    .replace("WFP", &test_workflow_yaml_path())
                    .replace("AGP", &test_agent_catalog_yaml_path()),
                )
                .data(observer_principal()),
            )
            .await;

        assert!(
            !response.errors.is_empty(),
            "observer must be denied with an error, got {response:?}"
        );
        assert!(
            response
                .errors
                .iter()
                .any(|e| e.message.contains("forbidden")),
            "denial reason must mention 'forbidden', got {:?}",
            response.errors
        );

        // Data field must not carry a journalId for the denied mutation.
        let data = response.data.into_json().unwrap_or(serde_json::json!(null));
        let has_jid = data
            .get("startRun")
            .and_then(|v| v.get("journalId"))
            .map(|v| !v.is_null())
            .unwrap_or(false);
        assert!(
            !has_jid,
            "denied mutation must NOT leak a journalId (got {data})"
        );

        // And there must be NO command_journal row — denied at capability
        // check, never reaches CommandHandler::handle.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "denied mutation must not write any audit row");
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
