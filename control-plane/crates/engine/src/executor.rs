use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use acp::AcpRuntimeManager;
use db::work_item::{WorkItem, WorkItemKind};
use db::repos::{agent_executions, artifacts, projections, stages};
use domain::ids::RunId;

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

    async fn run_loop(&self) {
        info!("BackgroundExecutor: starting work loop");
        loop {
            match self.work_queue.claim_next().await {
                Ok(Some(item)) => {
                    let item_id = item.id.clone();
                    let kind = item.kind.clone();
                    info!(item_id = %item_id, kind = %kind, "Processing work item");
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

                let req = acp::ExecutionRequest {
                    run_id,
                    stage_id: stage_id.clone(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model,
                    effort,
                    workspace_root: run.workspace_root.clone(),
                    prompt,
                };
                let result = self.acp.execute(req).await?;

                let completed_at = chrono::Utc::now();
                agent_executions::update_completed(
                    &self.pool,
                    agent_exec_id,
                    result.status.clone(),
                    completed_at,
                )
                .await?;

                // Persist artifacts returned by the ACP binary.
                for path in &result.artifact_paths {
                    // Derive name from the file path.
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("artifact")
                        .to_string();

                    // Derive format from extension (default to Json).
                    let format = std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .and_then(|ext| match ext {
                            "md" | "markdown" => {
                                Some(domain::artifact::ArtifactFormat::Markdown)
                            }
                            "diff" | "patch" => Some(domain::artifact::ArtifactFormat::Diff),
                            "json" => Some(domain::artifact::ArtifactFormat::Json),
                            _ => None,
                        })
                        .unwrap_or(domain::artifact::ArtifactFormat::Json);

                    // contract_id is provider-scoped, not a hard-coded stub.
                    let contract_id = format!("{}.output", provider);

                    let artifact = domain::artifact::Artifact {
                        id: domain::ids::ArtifactId::new(),
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        name,
                        contract_id,
                        format,
                        file_path: path.clone(),
                        checksum_sha256: None,
                        size_bytes: None,
                        provider: provider.clone(),
                        model: None,
                        created_at: completed_at,
                        is_pinned: false,
                        report_kind: None,
                        report_version: None,
                    };
                    artifacts::insert(&self.pool, &artifact).await?;
                    let _ = self.events.send(domain::events::DomainEvent::ArtifactCreated {
                        run_id,
                        artifact_id: artifact.id,
                    });
                }

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

                // Settle the stage based on ACP result status.
                let settlement_kind = match result.status {
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
                    status = ?result.status,
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
            }

            WorkItemKind::SettleStage => {
                let run_id = self.extract_run_id(&item)?;
                self.orchestrator.advance_run(run_id).await?;
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
