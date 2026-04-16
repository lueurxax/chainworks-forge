use std::sync::Arc;

use async_graphql::*;
use async_graphql::futures_util::StreamExt;
use sqlx::SqlitePool;
use tokio_stream::wrappers::BroadcastStream;

use db::repos::{approvals, ideas, runs, projections};
use domain::commands::{
    ApproveStageCmd, CancelRunCmd, Command, RejectStageCmd, RetryStageCmd, StartRunCmd,
};
use domain::events::DomainEvent;
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;

use crate::types::approval::GqlApproval;
use crate::types::artifact::GqlArtifact;
use crate::types::idea::GqlIdea;
use crate::types::run::GqlRun;
use crate::types::stage::GqlStageExecution;

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .finish()
}

pub struct QueryRoot;

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
        let idea_id: IdeaId = id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;
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
        let run_id: RunId =
            id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;
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
        let items = projections::list_stages_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
    }
}

pub struct MutationRoot;

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
    ) -> Result<GqlRun> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let iid: IdeaId =
            idea_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

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

        let result = cmd_handler.handle(cmd).await?;
        let run_id = match result {
            engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let run = runs::find_by_id(pool, run_id)
            .await?
            .ok_or_else(|| Error::new("Run not found after creation"))?;

        Ok(GqlRun::from(run))
    }

    async fn approve_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<GqlApproval> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let result = cmd_handler.handle(cmd).await?;
        let approval_id = match result {
            engine::command_handler::CommandResult::StageApproved { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(GqlApproval::from(approval))
    }

    async fn reject_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<GqlApproval> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RejectStage(RejectStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let result = cmd_handler.handle(cmd).await?;
        let approval_id = match result {
            engine::command_handler::CommandResult::StageRejected { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(GqlApproval::from(approval))
    }

    async fn retry_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
    ) -> Result<bool> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RetryStage(RetryStageCmd {
            run_id: rid,
            stage_id,
        });

        cmd_handler.handle(cmd).await?;
        Ok(true)
    }

    async fn cancel_run(&self, ctx: &Context<'_>, run_id: ID) -> Result<bool> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::CancelRun(CancelRunCmd { run_id: rid });
        cmd_handler.handle(cmd).await?;
        Ok(true)
    }
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn run_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRun>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
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
        })
    }

    async fn stage_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlStageExecution>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: RunId = run_id.parse().unwrap_or_else(|_| RunId::new());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
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
                        let stage =
                            db::repos::stages::find_by_id(&pool, stage_execution_id)
                                .await
                                .ok()??;
                        Some(Ok(Some(GqlStageExecution::from(stage))))
                    }
                    _ => None,
                }
            };
            fut
        })
    }

    async fn approval_requested(
        &self,
        ctx: &Context<'_>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalRequested { approval_id, .. } => {
                        let approval =
                            approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
                    _ => None,
                }
            };
            fut
        })
    }

    /// Live stream of ACP runtime/session lifecycle events.
    /// Emits on session_started, session_completed, and session_failed.
    /// Required for the SwiftUI thin-client's runtime health surface (P027 §8.1).
    async fn runtime_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRuntimeEvent>>> + '_ {
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
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
        })
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
    use db::repos::{artifacts, ideas, projections, runs};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::sync::Arc;

    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
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
        create_pool("sqlite::memory:").await.expect("in-memory pool failed")
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

        let schema = build_schema(pool.clone(), make_command_handler(pool.clone()), event_bus::new_bus(64));
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
                    deliveryConfigurationJson: "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                  ) {
                    id
                  }
                }
                "#
                .replace("IDEA_ID", &idea_id.to_string())
                .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path()),
            ))
            .await;

        assert!(response.errors.is_empty(), "mutation must succeed: {response:?}");
        let json = response.data.into_json().unwrap();
        let run_id = json["startRun"]["id"].as_str().unwrap();
        let run = runs::find_by_id(&pool, run_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            run.delivery_configuration_json,
            Some(
                "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .to_string()
            )
        );
    }

    #[tokio::test]
    async fn run_query_exposes_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let schema = build_schema(pool.clone(), make_command_handler(pool.clone()), event_bus::new_bus(64));
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
                    deliveryConfigurationJson: "{\"repo_identifier\":\"repo-2\",\"repo_root\":\"/repo-2\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                  ) {
                    id
                  }
                }
                "#
                .replace("IDEA_ID", &idea_id.to_string())
                .replace("WORKFLOW_YAML_PATH", &test_workflow_yaml_path())
                .replace("AGENT_CATALOG_YAML_PATH", &test_agent_catalog_yaml_path()),
            ))
            .await;

        assert!(start.errors.is_empty(), "mutation must succeed: {start:?}");
        let run_id = start.data.into_json().unwrap()["startRun"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let response = schema
            .execute(Request::new(
                format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryConfigurationJson
                  }}
                }}
                "#
                )
            ))
            .await;

        assert!(response.errors.is_empty(), "query must succeed: {response:?}");
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["run"]["deliveryConfigurationJson"],
            serde_json::json!(
                "{\"repo_identifier\":\"repo-2\",\"repo_root\":\"/repo-2\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
            )
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

        let schema = build_schema(pool.clone(), make_command_handler(pool.clone()), event_bus::new_bus(64));
        let response = schema
            .execute(Request::new(
                format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    cancellationSettlementLog
                  }}
                }}
                "#
                )
            ))
            .await;

        assert!(response.errors.is_empty(), "query must succeed: {response:?}");
        let json = response.data.into_json().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(json["run"]["cancellationSettlementLog"].as_str().unwrap()).unwrap();
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
        projections::rebuild_all_for_run(&pool, run_id).await.unwrap();

        let schema = build_schema(pool.clone(), make_command_handler(pool.clone()), event_bus::new_bus(64));
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
                "#
            ))
            .await;

        assert!(response.errors.is_empty(), "query must succeed: {response:?}");
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
        runs::insert(&pool, &make_run(run_id, idea_id)).await.unwrap();

        let payload_path = std::env::temp_dir().join(format!("validation-failure-{}.json", run_id));
        std::fs::write(&payload_path, serde_json::to_vec(&validation_failure_payload(run_id)).unwrap())
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
        artifacts::insert(&pool, &artifact)
        .await
        .unwrap();
        let record = validation_failure_record(
            artifact.id,
            run_id,
            stage_execution_id,
            agent_execution_id,
        );
        db::repos::validation::insert(&pool, &record)
            .await
            .unwrap();

        projections::rebuild_all_for_run(&pool, run_id).await.unwrap();

        let schema = build_schema(pool.clone(), make_command_handler(pool.clone()), event_bus::new_bus(64));
        let response = schema
            .execute(Request::new(
                format!(
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
                )
            ))
            .await;

        assert!(response.errors.is_empty(), "query must succeed: {response:?}");
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
}
