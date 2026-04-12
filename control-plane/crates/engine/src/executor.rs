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
                    .ok_or_else(|| anyhow::anyhow!("Missing stage_id"))?
                    .to_string();
                let stage_execution_id_str = payload["stage_execution_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let stage_execution_id: domain::ids::StageExecutionId = if stage_execution_id_str.is_empty() {
                    domain::ids::StageExecutionId::new()
                } else {
                    stage_execution_id_str.parse().map_err(|e| anyhow::anyhow!("{}", e))?
                };
                let agent_id = payload["agent_id"].as_str().unwrap_or("default").to_string();
                let provider = payload["provider"].as_str().unwrap_or("stub").to_string();

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

                // Invoke ACP adapter (stub returns mock result)
                let run = db::repos::runs::find_by_id(&self.pool, run_id).await?
                    .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;
                let req = acp::ExecutionRequest {
                    run_id,
                    stage_id: stage_id.clone(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    workspace_root: run.workspace_root.clone(),
                    prompt: format!("Execute stage {} for run {}", stage_id, run_id),
                };
                let result = self.acp.execute(req).await?;

                let completed_at = chrono::Utc::now();
                agent_executions::update_completed(
                    &self.pool,
                    agent_exec_id,
                    result.status.clone(),
                    completed_at,
                ).await?;

                // Persist any artifacts from ACP result
                for path in &result.artifact_paths {
                    let artifact = domain::artifact::Artifact {
                        id: domain::ids::ArtifactId::new(),
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        name: std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("artifact")
                            .to_string(),
                        contract_id: "stub".to_string(),
                        format: domain::artifact::ArtifactFormat::Json,
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

                // Settle stage as Completed
                stages::settle(
                    &self.pool,
                    stage_execution_id,
                    domain::stage::StageSettlementKind::Completed,
                    completed_at,
                ).await?;
                let _ = self.events.send(domain::events::DomainEvent::StageStatusChanged {
                    run_id,
                    stage_execution_id,
                    status: domain::stage::StageStatus::Completed,
                });

                // Rebuild projections
                projections::rebuild_all_for_run(&self.pool, run_id).await?;

                // Advance the run
                self.work_queue.enqueue(
                    WorkItemKind::AdvanceRun,
                    Some(run_id),
                    None,
                    serde_json::json!({ "run_id": run_id.to_string() }),
                ).await?;

                info!(run_id = %run_id, stage_id = %stage_id, "InvokeAgent completed");
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
