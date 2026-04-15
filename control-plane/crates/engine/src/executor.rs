use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use acp::AcpRuntimeManager;
use crate::release::{
    connect::ConnectPublishService,
    coordinator::ReleaseResult,
    git::{GitPushReceipt, GitReleaseService, ReleaseManifest},
    receipt::DeliveryReceiptBuilder,
};
use db::work_item::{WorkItem, WorkItemKind};
use db::repos::{agent_executions, artifacts, ideas, projections, stages, validation};
use domain::artifact::ArtifactFormat;
use domain::agent::AgentStatus;
use domain::ids::RunId;
use domain::run::DeliveryConfiguration;

use crate::contracts::{
    artifact_format_for_companion_output, artifact_format_for_machine_output,
    build_validation_failure_record, load_declared_output_bytes, validate_task_outputs,
    DeclaredOutput,
};
use crate::event_bus::EventSender;
use crate::orchestrator::Orchestrator;
use crate::recovery::RecoveryService;
use crate::work_queue::WorkQueue;

pub struct BackgroundExecutor {
    pool: SqlitePool,
    work_queue: WorkQueue,
    orchestrator: Arc<Orchestrator>,
    acp: Arc<AcpRuntimeManager>,
    events: EventSender,
}

impl BackgroundExecutor {
    pub fn new(
        pool: SqlitePool,
        work_queue: WorkQueue,
        orchestrator: Arc<Orchestrator>,
        acp: Arc<AcpRuntimeManager>,
        events: EventSender,
    ) -> Self {
        Self {
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
        }
    }

    /// Start the background loop. Returns a JoinHandle.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run_loop().await;
        })
    }

    /// Claim and process the next pending work item. Returns `Ok(true)` if an
    /// item was processed, `Ok(false)` if the queue was empty.
    /// Intended for test use — the production path uses `start()`.
    pub async fn process_next_item(&self) -> Result<bool> {
        match self.work_queue.claim_next().await? {
            Some(item) => {
                let item_id = item.id.clone();
                let kind = item.kind.clone();
                info!(item_id = %item_id, kind = %kind, "process_next_item: processing");
                match self.process_item(item).await {
                    Ok(()) => {
                        self.work_queue.complete(&item_id).await?;
                        Ok(true)
                    }
                    Err(e) => {
                        self.work_queue.fail(&item_id, &e.to_string()).await?;
                        Err(e)
                    }
                }
            }
            None => Ok(false),
        }
    }

    async fn run_loop(self: &Arc<Self>) {
        info!("BackgroundExecutor: starting work loop");
        loop {
            match self.work_queue.claim_next().await {
                Ok(Some(item)) => {
                    let item_id = item.id.clone();
                    let kind = item.kind.clone();
                    info!(item_id = %item_id, kind = %kind, "Processing work item");

                    // Spawn InvokeAgent items as concurrent tasks so parallel
                    // fan-out tasks run simultaneously (matches Swift TaskGroup).
                    // Other work item kinds run inline (fast, coordination-only).
                    if matches!(kind, WorkItemKind::InvokeAgent) {
                        let executor = Arc::clone(self);
                        tokio::spawn(async move {
                            match executor.process_item(item).await {
                                Ok(()) => {
                                    if let Err(e) = executor.work_queue.complete(&item_id).await {
                                        error!(item_id = %item_id, error = %e, "Failed to mark work item complete");
                                    }
                                }
                                Err(e) => {
                                    error!(item_id = %item_id, kind = %kind, error = %e, "Work item failed");
                                    if let Err(e2) = executor.work_queue.fail(&item_id, &e.to_string()).await {
                                        error!(item_id = %item_id, error = %e2, "Failed to mark work item failed");
                                    }
                                }
                            }
                        });
                    } else {
                        match self.process_item(item).await {
                            Ok(()) => {
                                if let Err(e) = self.work_queue.complete(&item_id).await {
                                    error!(item_id = %item_id, error = %e, "Failed to mark work item complete");
                                }
                            }
                            Err(e) => {
                                error!(item_id = %item_id, kind = %kind, error = %e, "Work item failed");
                                if let Err(e2) = self.work_queue.fail(&item_id, &e.to_string()).await {
                                    error!(item_id = %item_id, error = %e2, "Failed to mark work item failed");
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!(error = %e, "Error claiming next work item");
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    async fn process_item(&self, item: WorkItem) -> Result<()> {
        match item.kind {
            WorkItemKind::AdvanceRun => {
                let run_id = self.extract_run_id(&item)?;
                self.orchestrator.advance_run(run_id).await?;
                self.backfill_delivery_receipt_if_eligible(run_id).await?;
            }

            WorkItemKind::InvokeAgent => {
                let payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;
                let run_id = self.extract_run_id(&item)?;

                let stage_id = payload["stage_id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing 'stage_id'"))?
                    .to_string();

                let stage_execution_id_str = payload["stage_execution_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let stage_execution_id: domain::ids::StageExecutionId =
                    if stage_execution_id_str.is_empty() {
                        domain::ids::StageExecutionId::new()
                    } else {
                        stage_execution_id_str
                            .parse()
                            .map_err(|e| anyhow::anyhow!("{}", e))?
                    };

                // agent_id defaults to the stage_id — a reasonable per-stage identifier.
                let agent_id = payload["agent_id"]
                    .as_str()
                    .unwrap_or(&stage_id)
                    .to_string();

                // provider is required — no "stub" fallback.
                let provider = payload["provider"]
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "InvokeAgent payload missing 'provider' field; \
                             set CHAINWORKS_DEFAULT_PROVIDER or include 'provider' in the payload"
                        )
                    })?
                    .to_string();

                let now = chrono::Utc::now();
                let agent_exec_id = domain::ids::AgentExecutionId::new();
                let agent_exec = domain::agent::AgentExecution {
                    id: agent_exec_id,
                    stage_execution_id,
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model: None,
                    status: domain::agent::AgentStatus::Running,
                    started_at: now,
                    completed_at: None,
                };
                agent_executions::insert(&self.pool, &agent_exec).await?;

                // Build the ACP request from the run record (workspace_root lives there).
                let run = db::repos::runs::find_by_id(&self.pool, run_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;
                // Use the prompt from the work item payload if provided
                // (workflow-driven runs include the agent's system prompt from YAML).
                let prompt = payload["prompt"]
                    .as_str()
                    .unwrap_or(&format!("Execute stage {} for run {}", stage_id, run_id))
                    .to_string();

                let model = payload["model"].as_str().map(String::from);
                let effort = payload["effort"].as_str().map(String::from);
                let worktree_write_enabled = payload["worktree_write_enabled"]
                    .as_bool()
                    .unwrap_or(false);
                let worktree_strategy = payload["worktree_strategy"]
                    .as_str()
                    .map(String::from);
                let declared_outputs: Vec<DeclaredOutput> = serde_json::from_value(
                    payload["declared_outputs"].clone(),
                )
                .unwrap_or_default();
                let expected_output_paths: Vec<String> = declared_outputs
                    .iter()
                    .flat_map(|declared| {
                        std::iter::once(declared.target_path.clone())
                            .chain(declared.companion_path.clone().into_iter())
                    })
                    .collect();

                if self.is_release_agent(&agent_id) {
                    // Native release path: bypass ACP entirely and execute the
                    // deterministic git/publish services.
                    return self
                        .process_release_agent(
                            run_id,
                            run.clone(),
                            stage_id.clone(),
                            stage_execution_id,
                            agent_exec_id,
                            agent_id.clone(),
                            provider.clone(),
                            model,
                            effort,
                            worktree_write_enabled,
                            worktree_strategy,
                            payload,
                        )
                        .await;
                }

                let req = acp::ExecutionRequest {
                    run_id,
                    stage_id: stage_id.clone(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    effort,
                    workspace_root: run.workspace_root.clone(),
                    prompt,
                    worktree_root: run.worktree_root.clone(),
                    worktree_write_enabled,
                    worktree_strategy,
                    expected_output_paths,
                };
                // Runtime event: session starting
                let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                    run_id,
                    stage_id: stage_id.clone(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    event_kind: "session_started".to_string(),
                });

                let result = self.acp.execute(req).await?;

                // Runtime event: session finished (completed or failed)
                let event_kind = match result.status {
                    domain::agent::AgentStatus::Completed => "session_completed",
                    domain::agent::AgentStatus::Failed => "session_failed",
                    _ => "session_completed",
                };
                let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                    run_id,
                    stage_id: stage_id.clone(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    event_kind: event_kind.to_string(),
                });

                let completed_at = chrono::Utc::now();
                let mut persisted_paths = std::collections::HashSet::new();
                let mut persisted_artifacts = self
                    .persist_declared_output_artifacts(
                        &declared_outputs,
                        run_id,
                        &stage_id,
                        &agent_id,
                        &provider,
                        model.clone(),
                        completed_at,
                        &mut persisted_paths,
                    )
                    .await?;

                for path in &result.artifact_paths {
                    if persisted_paths.contains(path) {
                        continue;
                    }
                    if let Some(artifact) = self
                        .persist_generic_artifact(
                            path,
                            run_id,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            completed_at,
                        )
                        .await?
                    {
                        persisted_paths.insert(path.clone());
                        persisted_artifacts.push(artifact);
                    }
                }

                let validation_summary = if declared_outputs.is_empty() {
                    None
                } else {
                    Some(validate_task_outputs(&load_declared_output_bytes(&declared_outputs)?))
                };
                let validation_failed = validation_summary
                    .as_ref()
                    .and_then(|summary| summary.failure_class.as_ref())
                    .is_some();
                let final_agent_status = if result.status == AgentStatus::Completed && !validation_failed {
                    AgentStatus::Completed
                } else {
                    AgentStatus::Failed
                };

                if let Some(summary) = validation_summary {
                    if summary.failure_class.is_some() {
                        let validation_artifact = self
                            .persist_validation_failure_artifact(
                                &run,
                                &stage_id,
                                &agent_id,
                                &provider,
                                model.clone(),
                                agent_exec_id,
                                stage_execution_id,
                                build_validation_failure_record(
                                    domain::ids::ArtifactId::new(),
                                    run_id,
                                    stage_id.clone(),
                                    stage_execution_id,
                                    agent_id.clone(),
                                    agent_exec_id,
                                    summary,
                                    persisted_artifacts
                                        .iter()
                                        .any(|artifact| artifact.name.contains("receipt")),
                                    std::path::Path::new(&format!(
                                        "{}/.chainworks/acp-stderr.log",
                                        run.workspace_root
                                    ))
                                    .exists(),
                                )?,
                            )
                            .await?;
                        persisted_artifacts.push(validation_artifact);
                    }
                }

                agent_executions::update_completed(
                    &self.pool,
                    agent_exec_id,
                    final_agent_status.clone(),
                    completed_at,
                )
                .await?;

                // Normalize artifacts: copy from artifact_root to canonical
                // workspace paths defined in the YAML artifacts map.
                // This ensures transition conditions always find files at
                // the expected location regardless of where the agent wrote them.
                if let (Some(wf_path), Some(ac_path)) =
                    (&run.workflow_yaml_path, &run.agent_catalog_yaml_path)
                {
                    if let Ok(plan) = workflow::compiler::compile(wf_path, ac_path) {
                        normalize_artifacts(
                            &run.artifact_root,
                            &run.workspace_root,
                            run_id,
                            &plan.artifact_paths,
                        );
                    }
                }

                // If this is a multi-task stage (fan-out), don't settle the stage
                // here — let the orchestrator settle after ALL tasks complete.
                // Only settle for single-task stages (backward compat).
                let total_tasks = payload["total_tasks"].as_u64().unwrap_or(1);

                // Settle the stage based on ACP result status.
                let settlement_kind = match final_agent_status {
                    domain::agent::AgentStatus::Completed => {
                        domain::stage::StageSettlementKind::Completed
                    }
                    domain::agent::AgentStatus::Failed => {
                        domain::stage::StageSettlementKind::Failed
                    }
                    _ => domain::stage::StageSettlementKind::Failed,
                };
                let settled_stage_status = match settlement_kind {
                    domain::stage::StageSettlementKind::Completed => {
                        domain::stage::StageStatus::Completed
                    }
                    domain::stage::StageSettlementKind::Failed => {
                        domain::stage::StageStatus::Failed
                    }
                    domain::stage::StageSettlementKind::Skipped => {
                        domain::stage::StageStatus::Skipped
                    }
                };
                if total_tasks <= 1 {
                    // Single-task stage: settle immediately (original behavior).
                    stages::settle(
                        &self.pool,
                        stage_execution_id,
                        settlement_kind,
                        completed_at,
                    )
                    .await?;
                    let _ = self.events.send(domain::events::DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        status: settled_stage_status,
                    });
                }
                // Multi-task stage: the orchestrator will settle after all
                // tasks complete (checked via work item completion count).

                // Rebuild projections so northbound reads reflect latest state.
                projections::rebuild_all_for_run(&self.pool, run_id).await?;

                // Re-evaluate the run.
                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(run_id),
                        None,
                        serde_json::json!({ "run_id": run_id.to_string() }),
                    )
                    .await?;

                info!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    status = ?final_agent_status,
                    "InvokeAgent completed"
                );
            }

            WorkItemKind::StartupRepair => {
                let recovery = RecoveryService::new(
                    self.pool.clone(),
                    self.work_queue.clone(),
                    self.events.clone(),
                );
                recovery.run_startup_repair().await?;
            }

            WorkItemKind::TriggerNextStage => {
                let run_id = self.extract_run_id(&item)?;
                self.orchestrator.advance_run(run_id).await?;
                self.backfill_delivery_receipt_if_eligible(run_id).await?;
            }

            WorkItemKind::SettleStage => {
                let run_id = self.extract_run_id(&item)?;
                self.orchestrator.advance_run(run_id).await?;
                self.backfill_delivery_receipt_if_eligible(run_id).await?;
            }

            WorkItemKind::RebuildProjection => {
                let run_id = self.extract_run_id(&item)?;
                projections::rebuild_all_for_run(&self.pool, run_id).await?;
                info!(run_id = %run_id, "RebuildProjection complete");
            }
        }

        Ok(())
    }

    fn extract_run_id(&self, item: &WorkItem) -> Result<RunId> {
        item.run_id
            .ok_or_else(|| anyhow::anyhow!("Work item {} has no run_id", item.id))
    }

    fn is_release_agent(&self, agent_id: &str) -> bool {
        matches!(
            agent_id,
            "commit_and_push_to_github" | "build_archive_and_push_connect"
        )
    }

    async fn process_release_agent(
        &self,
        run_id: RunId,
        run: domain::run::Run,
        stage_id: String,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_exec_id: domain::ids::AgentExecutionId,
        agent_id: String,
        provider: String,
        model: Option<String>,
        _effort: Option<String>,
        _worktree_write_enabled: bool,
        _worktree_strategy: Option<String>,
        _payload: serde_json::Value,
    ) -> Result<()> {
        let delivery_config = self.load_delivery_configuration(&run).await?;
        let worktree_root = run.worktree_root.clone().ok_or_else(|| {
            anyhow::anyhow!("Release agent requires a provisioned worktree but none is available.")
        })?;
        let idea_title = ideas::find_by_id(&self.pool, run.idea_id)
            .await?
            .map(|idea| idea.title)
            .unwrap_or_else(|| "Unknown".to_string());
        let now = chrono::Utc::now();

        let runtime_started = domain::events::DomainEvent::RuntimeStatusChanged {
            run_id,
            stage_id: stage_id.clone(),
            agent_id: agent_id.clone(),
            provider: provider.clone(),
            event_kind: "session_started".to_string(),
        };
        let _ = self.events.send(runtime_started);

        if agent_id == "commit_and_push_to_github" {
            let commit_message = format!("[{}] {} :: {}", run_id, idea_title, stage_id);
            let git_service = GitReleaseService;
            match git_service
                .commit_and_push(&worktree_root, &delivery_config.target_branch, &commit_message)
                .await
            {
                Ok((manifest, receipt)) => {
                    let _manifest_path = self
                        .persist_json_artifact(
                            &run,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            "release_manifest",
                            &manifest,
                        )
                        .await?;
                    let _receipt_path = self
                        .persist_json_artifact(
                            &run,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            "git_push_receipt",
                            &receipt,
                        )
                        .await?;
                    let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        event_kind: "session_completed".to_string(),
                    });
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Completed,
                        now,
                    )
                    .await?;
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    self.work_queue
                        .enqueue(
                            WorkItemKind::AdvanceRun,
                            Some(run_id),
                            None,
                            serde_json::json!({ "run_id": run_id.to_string() }),
                        )
                        .await?;
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        status = "completed",
                        "Release agent completed"
                    );
                    return Ok(());
                }
                Err(error) => {
                    let release_result = ReleaseResult {
                        git_manifest: None,
                        git_receipt: None,
                        bundle_manifest: None,
                        upload_receipt: None,
                        succeeded: false,
                        failure_stage: Some("commit_and_push".to_string()),
                        failure_reason: Some(error.to_string()),
                    };
                    if let Some(receipt_path) = self
                        .persist_delivery_receipt_if_absent(
                            &run,
                            &delivery_config,
                            &release_result,
                            &idea_title,
                            None,
                            &stage_id,
                            &provider,
                            model.clone(),
                        )
                        .await?
                    {
                        info!(run_id = %run_id, receipt_path = %receipt_path, "delivery receipt persisted");
                    }

                    let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        event_kind: "session_failed".to_string(),
                    });
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Failed,
                        now,
                    )
                    .await?;
                    let settlement_kind = domain::stage::StageSettlementKind::Failed;
                    stages::settle(&self.pool, stage_execution_id, settlement_kind, now).await?;
                    let _ = self.events.send(domain::events::DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        status: domain::stage::StageStatus::Failed,
                    });
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        error = %error,
                        "Release git step failed"
                    );
                    return Err(error);
                }
            }
        } else {
            let git_receipt: GitPushReceipt =
                self.load_release_artifact(run_id, "git_push_receipt").await?;
            let release_manifest: ReleaseManifest =
                self.load_release_artifact(run_id, "release_manifest").await?;
            let publish_service = ConnectPublishService;
            match publish_service
                .build_and_distribute(
                    &worktree_root,
                    &git_receipt,
                    &release_manifest,
                    &delivery_config,
                )
                .await
            {
                Ok((bundle_manifest, upload_receipt)) => {
                    let bundle_path = self
                        .persist_json_artifact(
                            &run,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            "release_bundle_manifest",
                            &bundle_manifest,
                        )
                        .await?;
                    let upload_path = self
                        .persist_json_artifact(
                            &run,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            "connect_upload_receipt",
                            &upload_receipt,
                        )
                        .await?;
                    let release_result = ReleaseResult {
                        git_manifest: Some(release_manifest),
                        git_receipt: Some(git_receipt),
                        bundle_manifest: Some(bundle_manifest),
                        upload_receipt: Some(upload_receipt),
                        succeeded: true,
                        failure_stage: None,
                        failure_reason: None,
                    };
                    let _ = self.persist_delivery_receipt_if_absent(
                        &run,
                        &delivery_config,
                        &release_result,
                        &idea_title,
                        None,
                        &stage_id,
                        &provider,
                        model.clone(),
                    )
                    .await?;
                    let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        event_kind: "session_completed".to_string(),
                    });
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Completed,
                        now,
                    )
                    .await?;
                    stages::settle(
                        &self.pool,
                        stage_execution_id,
                        domain::stage::StageSettlementKind::Completed,
                        now,
                    )
                    .await?;
                    let _ = self.events.send(domain::events::DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        status: domain::stage::StageStatus::Completed,
                    });
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    self.work_queue
                        .enqueue(
                            WorkItemKind::AdvanceRun,
                            Some(run_id),
                            None,
                            serde_json::json!({ "run_id": run_id.to_string() }),
                        )
                        .await?;
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        bundle_path = %bundle_path,
                        upload_path = %upload_path,
                        "Release publish step completed"
                    );
                    return Ok(());
                }
                Err(error) => {
                    let release_result = ReleaseResult {
                        git_manifest: Some(release_manifest),
                        git_receipt: Some(git_receipt),
                        bundle_manifest: None,
                        upload_receipt: None,
                        succeeded: false,
                        failure_stage: Some("build_archive_and_push".to_string()),
                        failure_reason: Some(error.to_string()),
                    };
                    if let Some(receipt_path) = self
                        .persist_delivery_receipt_if_absent(
                            &run,
                            &delivery_config,
                            &release_result,
                            &idea_title,
                            None,
                            &stage_id,
                            &provider,
                            model.clone(),
                        )
                        .await?
                    {
                        info!(run_id = %run_id, receipt_path = %receipt_path, "delivery receipt persisted");
                    }
                    let _ = self.events.send(domain::events::DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        event_kind: "session_failed".to_string(),
                    });
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Failed,
                        now,
                    )
                    .await?;
                    stages::settle(
                        &self.pool,
                        stage_execution_id,
                        domain::stage::StageSettlementKind::Failed,
                        now,
                    )
                    .await?;
                    let _ = self.events.send(domain::events::DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        status: domain::stage::StageStatus::Failed,
                    });
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        error = %error,
                        "Release publish step failed"
                    );
                    return Err(error);
                }
            }
        }
    }

    async fn load_delivery_configuration(
        &self,
        run: &domain::run::Run,
    ) -> Result<DeliveryConfiguration> {
        let json = run
            .delivery_configuration_json
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Release agent requires delivery_configuration_json"))?;
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!("Invalid delivery_configuration_json: {}", e))
    }

    async fn persist_declared_output_artifacts(
        &self,
        declared_outputs: &[DeclaredOutput],
        run_id: RunId,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        persisted_paths: &mut std::collections::HashSet<String>,
    ) -> Result<Vec<domain::artifact::Artifact>> {
        let mut artifacts_out = Vec::new();
        for declared in declared_outputs {
            let default_contract_id = format!("{}.output", provider);
            if let Some(artifact) = self
                .persist_artifact_if_present(
                    &declared.target_path,
                    run_id,
                    stage_id,
                    agent_id,
                    declared.output_name.as_str(),
                    declared
                        .schema
                        .as_ref()
                        .map(|schema| schema.contract_id.as_str())
                        .unwrap_or(default_contract_id.as_str()),
                    artifact_format_for_machine_output(declared.schema.as_ref()),
                    provider,
                    model.clone(),
                    None,
                    created_at,
                )
                .await?
            {
                persisted_paths.insert(declared.target_path.clone());
                artifacts_out.push(artifact);
            }

            if let (Some(companion_name), Some(companion_path), Some(schema)) = (
                declared.companion_output_name.as_deref(),
                declared.companion_path.as_deref(),
                declared.schema.as_ref(),
            ) {
                if let Some(artifact) = self
                    .persist_artifact_if_present(
                        companion_path,
                        run_id,
                        stage_id,
                        agent_id,
                        companion_name,
                        &schema.contract_id,
                        artifact_format_for_companion_output(schema),
                        provider,
                        model.clone(),
                        None,
                        created_at,
                    )
                    .await?
                {
                    persisted_paths.insert(companion_path.to_string());
                    artifacts_out.push(artifact);
                }
            }
        }
        Ok(artifacts_out)
    }

    async fn persist_generic_artifact(
        &self,
        path: &str,
        run_id: RunId,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<domain::artifact::Artifact>> {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("artifact");
        let format = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| match ext {
                "md" | "markdown" => Some(ArtifactFormat::Markdown),
                "diff" | "patch" => Some(ArtifactFormat::Diff),
                "json" => Some(ArtifactFormat::Json),
                "txt" => Some(ArtifactFormat::Report),
                _ => None,
            })
            .unwrap_or(ArtifactFormat::Json);
        self.persist_artifact_if_present(
            path,
            run_id,
            stage_id,
            agent_id,
            name,
            &format!("{}.output", provider),
            format,
            provider,
            model,
            None,
            created_at,
        )
        .await
    }

    async fn persist_artifact_if_present(
        &self,
        path: &str,
        run_id: RunId,
        stage_id: &str,
        agent_id: &str,
        name: &str,
        contract_id: &str,
        format: ArtifactFormat,
        provider: &str,
        model: Option<String>,
        report_kind: Option<&str>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<domain::artifact::Artifact>> {
        let artifact_path = std::path::Path::new(path);
        if !artifact_path.is_file() {
            return Ok(None);
        }
        let size_bytes = artifact_path
            .metadata()
            .ok()
            .map(|meta| meta.len() as i64);
        let artifact = domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            contract_id: contract_id.to_string(),
            format,
            file_path: path.to_string(),
            checksum_sha256: None,
            size_bytes,
            provider: provider.to_string(),
            model,
            created_at,
            is_pinned: false,
            report_kind: report_kind.map(str::to_string),
            report_version: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;
        let _ = self.events.send(domain::events::DomainEvent::ArtifactCreated {
            run_id,
            artifact_id: artifact.id,
        });
        Ok(Some(artifact))
    }

    async fn persist_validation_failure_artifact(
        &self,
        run: &domain::run::Run,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        _agent_execution_id: domain::ids::AgentExecutionId,
        _stage_execution_id: domain::ids::StageExecutionId,
        record: domain::validation::ValidationFailureRecord,
    ) -> Result<domain::artifact::Artifact> {
        let artifact_name = format!("validation_failure_{}_{}", agent_id, record.id);
        let path = std::path::Path::new(&run.artifact_root)
            .join(format!("{artifact_name}.json"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("create validation artifact dir {}: {}", parent.display(), e))?;
        }
        let encoded = serde_json::to_string_pretty(&record)?;
        std::fs::write(&path, encoded)
            .map_err(|e| anyhow::anyhow!("write validation failure artifact {}: {}", path.display(), e))?;

        let artifact = domain::artifact::Artifact {
            id: record.artifact_id,
            run_id: run.id,
            stage_id: stage_id.to_string(),
            agent_id: agent_id.to_string(),
            name: artifact_name,
            contract_id: "validation_failure_record".to_string(),
            format: ArtifactFormat::Json,
            file_path: path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: Some(path.metadata().map(|meta| meta.len() as i64).unwrap_or(0)),
            provider: provider.to_string(),
            model,
            created_at: chrono::Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".to_string()),
            report_version: Some(1),
        };
        artifacts::insert(&self.pool, &artifact).await?;
        validation::insert(&self.pool, &record).await?;
        let _ = self.events.send(domain::events::DomainEvent::ArtifactCreated {
            run_id: run.id,
            artifact_id: artifact.id,
        });
        Ok(artifact)
    }

    async fn load_release_artifact<T: DeserializeOwned>(
        &self,
        run_id: RunId,
        name: &str,
    ) -> Result<T> {
        let rows = artifacts::list_by_run(&self.pool, run_id).await?;
        let artifact = rows
            .into_iter()
            .rev()
            .find(|artifact| artifact.name == name)
            .ok_or_else(|| anyhow::anyhow!("ConnectPublishService requires git_push_receipt and release_manifest inputs."))?;
        let json = std::fs::read_to_string(&artifact.file_path)
            .map_err(|e| anyhow::anyhow!("read artifact {}: {}", artifact.file_path, e))?;
        serde_json::from_str(&json)
            .map_err(|e| anyhow::anyhow!("decode artifact {}: {}", artifact.file_path, e))
    }

    async fn persist_json_artifact<T: Serialize>(
        &self,
        run: &domain::run::Run,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        name: &str,
        value: &T,
    ) -> Result<String> {
        let path = self.resolve_release_artifact_path(run, name);
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("create artifact dir {}: {}", parent.display(), e))?;
        }
        let json = serde_json::to_string_pretty(value)?;
        std::fs::write(&path, json)
            .map_err(|e| anyhow::anyhow!("write artifact {}: {}", path, e))?;

        let artifact = domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id: run.id,
            stage_id: stage_id.to_string(),
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            contract_id: name.to_string(),
            format: ArtifactFormat::Json,
            file_path: path.clone(),
            checksum_sha256: None,
            size_bytes: None,
            provider: provider.to_string(),
            model,
            created_at: chrono::Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;
        let _ = self.events.send(domain::events::DomainEvent::ArtifactCreated {
            run_id: run.id,
            artifact_id: artifact.id,
        });
        Ok(path)
    }

    async fn persist_delivery_receipt_if_absent(
        &self,
        run: &domain::run::Run,
        delivery_config: &DeliveryConfiguration,
        release_result: &ReleaseResult,
        idea_title: &str,
        review_status: Option<&str>,
        stage_id: &str,
        provider: &str,
        model: Option<String>,
    ) -> Result<Option<String>> {
        let receipt = match DeliveryReceiptBuilder::build_receipt(
            run,
            delivery_config,
            Some(release_result),
            idea_title,
            review_status,
        ) {
            Some(receipt) => receipt,
            None => return Ok(None),
        };
        let path = self.resolve_release_artifact_path(run, "delivery_receipt");
        if std::path::Path::new(&path).exists() {
            return Ok(None);
        }
        let _ = self
            .persist_json_artifact(
                run,
                stage_id,
                "system_delivery",
                provider,
                model,
                "delivery_receipt",
                &receipt,
            )
            .await?;
        Ok(Some(path))
    }

    fn resolve_release_artifact_path(&self, run: &domain::run::Run, name: &str) -> String {
        if let (Some(workflow_yaml_path), Some(agent_catalog_yaml_path)) =
            (&run.workflow_yaml_path, &run.agent_catalog_yaml_path)
        {
            if let Ok(plan) = workflow::compiler::compile(workflow_yaml_path, agent_catalog_yaml_path)
            {
                if let Some(template) = plan.artifact_paths.get(name) {
                    return crate::orchestrator::resolve_path_template(
                        template,
                        &run.workspace_root,
                    );
                }
            }
        }
        release_artifact_path(&run.artifact_root, name)
    }

    async fn backfill_delivery_receipt_if_eligible(&self, run_id: RunId) -> Result<()> {
        let run = match db::repos::runs::find_by_id(&self.pool, run_id).await? {
            Some(run) if run.status.is_terminal() => run,
            _ => return Ok(()),
        };

        if artifacts::list_by_run(&self.pool, run_id)
            .await?
            .iter()
            .any(|artifact| artifact.name == "delivery_receipt")
        {
            return Ok(());
        }

        let delivery_config = match self.load_delivery_configuration(&run).await {
            Ok(config) => config,
            Err(_) => return Ok(()),
        };
        let release_result = match self.reconstruct_release_result(&run).await? {
            Some(result) => result,
            None => return Ok(()),
        };
        let idea_title = ideas::find_by_id(&self.pool, run.idea_id)
            .await?
            .map(|idea| idea.title)
            .unwrap_or_else(|| "Unknown".to_string());

        let _ = self
            .persist_delivery_receipt_if_absent(
                &run,
                &delivery_config,
                &release_result,
                &idea_title,
                None,
                run.current_state.as_deref().unwrap_or("state_12_finalization"),
                "system",
                None,
            )
            .await?;

        Ok(())
    }

    async fn reconstruct_release_result(
        &self,
        run: &domain::run::Run,
    ) -> Result<Option<ReleaseResult>> {
        let artifacts_by_run = artifacts::list_by_run(&self.pool, run.id).await?;
        let git_manifest: Option<ReleaseManifest> =
            self.decode_latest_artifact(&artifacts_by_run, "release_manifest")?;
        let git_receipt: Option<GitPushReceipt> =
            self.decode_latest_artifact(&artifacts_by_run, "git_push_receipt")?;
        let bundle_manifest: Option<crate::release::connect::ReleaseBundleManifest> =
            self.decode_latest_artifact(&artifacts_by_run, "release_bundle_manifest")?;
        let upload_receipt: Option<crate::release::connect::ConnectUploadReceipt> =
            self.decode_latest_artifact(&artifacts_by_run, "connect_upload_receipt")?;

        let stage_executions = stages::list_by_run(&self.pool, run.id).await?;
        let attempted_commit = stage_executions
            .iter()
            .any(|stage| stage.stage_id == "commit_and_push_to_github");
        let attempted_publish = stage_executions
            .iter()
            .any(|stage| stage.stage_id == "build_archive_and_push_connect");
        let failed_commit = stage_executions.iter().any(|stage| {
            stage.stage_id == "commit_and_push_to_github"
                && matches!(stage.status, domain::stage::StageStatus::Failed)
        });
        let failed_publish = stage_executions.iter().any(|stage| {
            stage.stage_id == "build_archive_and_push_connect"
                && matches!(stage.status, domain::stage::StageStatus::Failed)
        });

        if git_manifest.is_none() && git_receipt.is_none() {
            if failed_commit || attempted_commit {
                return Ok(Some(ReleaseResult {
                    git_manifest: None,
                    git_receipt: None,
                    bundle_manifest: None,
                    upload_receipt: None,
                    succeeded: false,
                    failure_stage: Some("commit_and_push".to_string()),
                    failure_reason: None,
                }));
            }
            return Ok(None);
        }

        if bundle_manifest.is_some() && upload_receipt.is_some() {
            return Ok(Some(ReleaseResult {
                git_manifest,
                git_receipt,
                bundle_manifest,
                upload_receipt,
                succeeded: true,
                failure_stage: None,
                failure_reason: None,
            }));
        }

        if failed_publish || attempted_publish {
            return Ok(Some(ReleaseResult {
                git_manifest,
                git_receipt,
                bundle_manifest,
                upload_receipt,
                succeeded: false,
                failure_stage: Some("build_archive_and_push".to_string()),
                failure_reason: None,
            }));
        }

        Ok(None)
    }

    fn decode_latest_artifact<T: DeserializeOwned>(
        &self,
        artifacts_by_run: &[domain::artifact::Artifact],
        name: &str,
    ) -> Result<Option<T>> {
        let artifact = match artifacts_by_run.iter().rev().find(|artifact| artifact.name == name) {
            Some(artifact) => artifact,
            None => return Ok(None),
        };
        let json = std::fs::read_to_string(&artifact.file_path)
            .map_err(|e| anyhow::anyhow!("read artifact {}: {}", artifact.file_path, e))?;
        let decoded = serde_json::from_str(&json)
            .map_err(|e| anyhow::anyhow!("decode artifact {}: {}", artifact.file_path, e))?;
        Ok(Some(decoded))
    }
}

fn release_artifact_path(artifact_root: &str, name: &str) -> String {
    std::path::Path::new(artifact_root)
        .join(format!("{name}.json"))
        .to_string_lossy()
        .into_owned()
}

/// Copy artifacts from artifact_root to canonical workspace paths from the YAML
/// artifacts map. Scans artifact_root (and artifact_root/run_id/) for files whose
/// names match a known artifact name, then copies to the workspace-relative path.
fn normalize_artifacts(
    artifact_root: &str,
    workspace_root: &str,
    run_id: RunId,
    artifact_paths: &std::collections::HashMap<String, String>,
) {
    let run_dir = format!("{}/{}", artifact_root, run_id);
    let search_dirs = [artifact_root.to_string(), run_dir];

    for (artifact_name, path_template) in artifact_paths {
        let canonical = crate::orchestrator::resolve_path_template(path_template, workspace_root);

        // Already exists at canonical location — skip
        if std::path::Path::new(&canonical).exists() {
            continue;
        }

        // Search for the artifact in artifact_root locations
        for dir in &search_dirs {
            let candidate = format!("{}/{}", dir, artifact_name);
            if std::path::Path::new(&candidate).exists() {
                // Create parent directories
                if let Some(parent) = std::path::Path::new(&canonical).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::copy(&candidate, &canonical) {
                    Ok(_) => {
                        info!(
                            artifact = %artifact_name,
                            from = %candidate,
                            to = %canonical,
                            "Normalized artifact to canonical path"
                        );
                    }
                    Err(e) => {
                        error!(
                            artifact = %artifact_name,
                            from = %candidate,
                            to = %canonical,
                            error = %e,
                            "Failed to normalize artifact"
                        );
                    }
                }
                break;
            }
        }
    }
}
