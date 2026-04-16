use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::release::{
    connect::ConnectPublishService,
    coordinator::ReleaseResult,
    git::{GitPushReceipt, GitReleaseService, ReleaseManifest},
    receipt::DeliveryReceiptBuilder,
};
use acp::AcpRuntimeManager;
use db::repos::{agent_executions, artifacts, ideas, projections, sessions, stages, validation};
use db::work_item::{WorkItem, WorkItemKind};
use domain::agent::AgentStatus;
use domain::artifact::ArtifactFormat;
use domain::ids::RunId;
use domain::run::DeliveryConfiguration;
use workflow::catalog::{AgentCatalogFile, AgentEntry};

use crate::contracts::{
    artifact_format_for_companion_output, artifact_format_for_machine_output,
    build_validation_failure_record, load_declared_output_bytes, validate_task_outputs,
    DeclaredOutput,
};
use crate::event_bus::EventSender;
use crate::orchestrator::Orchestrator;
use crate::recovery::RecoveryService;
use crate::session::fingerprint::{
    binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
};
use crate::session::policy::{ensure_policy, SessionPolicyDecision, SessionPolicyInput};
use crate::work_queue::WorkQueue;

pub struct BackgroundExecutor {
    pool: SqlitePool,
    work_queue: WorkQueue,
    orchestrator: Arc<Orchestrator>,
    acp: Arc<AcpRuntimeManager>,
    events: EventSender,
    steward_runtime_inputs: Option<Arc<crate::steward::config::StewardRuntimeInputs>>,
}

struct BackgroundStewardAgentExecutor {
    acp: Arc<AcpRuntimeManager>,
    runtime_inputs: Arc<crate::steward::config::StewardRuntimeInputs>,
}

#[async_trait::async_trait]
impl crate::steward::service::StewardAgentExecutor for BackgroundStewardAgentExecutor {
    async fn run_steward_agent(
        &self,
        invocation: crate::steward::service::StewardAgentInvocation,
    ) -> Result<()> {
        let catalog = workflow::catalog::load(
            self.runtime_inputs
                .agent_catalog_path
                .to_string_lossy()
                .as_ref(),
        )?;
        let agent = catalog
            .agents
            .as_ref()
            .and_then(|agents| agents.iter().find(|agent| agent.id == invocation.agent_id))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Steward agent '{}' not found in active catalog {}",
                    invocation.agent_id,
                    self.runtime_inputs.agent_catalog_path.display()
                )
            })?;
        let profiles = catalog
            .backend_profiles
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Active catalog has no backend_profiles"))?;
        let profile = profiles.get(&agent.backend_profile).ok_or_else(|| {
            anyhow::anyhow!(
                "Steward agent '{}' references unknown backend_profile '{}'",
                agent.id,
                agent.backend_profile
            )
        })?;
        let provider = normalize_steward_provider(&profile.provider);
        let requested_mcp_server_ids = profile.mcp.clone().unwrap_or_default();
        let mcp_resolution = crate::mcp::resolve_mcp_servers(
            &requested_mcp_server_ids,
            Some(&agent.backend_profile),
            &provider,
        );
        if !mcp_resolution.report.blocking_issues.is_empty() {
            anyhow::bail!(
                "Steward agent '{}' MCP resolution failed: {}",
                agent.id,
                serde_json::to_string(&mcp_resolution.report.blocking_issues)?
            );
        }

        let expected_output_paths =
            steward_expected_output_paths(&catalog, agent, &invocation.chainworks_meta_root);
        let prompt =
            build_steward_agent_prompt(&catalog, agent, &invocation, &expected_output_paths);
        let result = self
            .acp
            .execute(acp::ExecutionRequest {
                run_id: RunId::new(),
                stage_id: format!("steward_{}", agent.id),
                agent_id: agent.id.clone(),
                provider,
                model: profile.model.clone(),
                effort: profile.effort.clone(),
                workspace_root: invocation
                    .chainworks_meta_root
                    .to_string_lossy()
                    .into_owned(),
                prompt,
                worktree_root: None,
                worktree_write_enabled: false,
                worktree_strategy: None,
                expected_output_paths,
                keep_session_alive: false,
                reuse_existing_session: false,
                session_generation_id: None,
                provider_session_id: None,
                mcp_servers: mcp_resolution.payloads,
            })
            .await?;
        if result.status != AgentStatus::Completed {
            anyhow::bail!(
                "Steward agent '{}' finished with status {}",
                agent.id,
                result.status
            );
        }
        Ok(())
    }
}

fn write_discovered_output(path: &str, content: &[u8]) -> Result<()> {
    let path_obj = std::path::Path::new(path);
    if let Some(parent) = path_obj.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path_obj, content)?;
    Ok(())
}

fn materialize_declared_outputs_from_discovered_artifacts(
    declared_outputs: &[DeclaredOutput],
    discovered_artifacts: &[acp::DiscoveredArtifact],
) -> Result<()> {
    let artifact_map: std::collections::HashMap<&str, &acp::DiscoveredArtifact> =
        discovered_artifacts
            .iter()
            .map(|artifact| (artifact.name.as_str(), artifact))
            .collect();

    for declared in declared_outputs {
        if let Some(artifact) = artifact_map.get(declared.output_name.as_str()) {
            write_discovered_output(&declared.target_path, &artifact.content)?;
        }

        if let (Some(companion_name), Some(companion_path)) = (
            declared.companion_output_name.as_deref(),
            declared.companion_path.as_deref(),
        ) {
            if let Some(artifact) = artifact_map.get(companion_name) {
                write_discovered_output(companion_path, &artifact.content)?;
            }
        }
    }

    Ok(())
}

fn declared_machine_artifact_name<'a>(declared: &'a DeclaredOutput) -> &'a str {
    declared
        .schema
        .as_ref()
        .and_then(|schema| schema.normalized_artifact_name.as_deref())
        .unwrap_or(declared.output_name.as_str())
}

fn infer_artifact_format_from_content(content: &[u8]) -> (ArtifactFormat, &'static str) {
    if serde_json::from_slice::<serde_json::Value>(content).is_ok() {
        (ArtifactFormat::Json, "json")
    } else if std::str::from_utf8(content).is_ok() {
        (ArtifactFormat::Report, "txt")
    } else {
        (ArtifactFormat::Json, "bin")
    }
}

fn sanitize_artifact_name_for_path(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            _ => c,
        })
        .collect();
    if sanitized.is_empty() {
        "artifact".to_string()
    } else {
        sanitized
    }
}

fn normalize_steward_provider(provider: &str) -> String {
    match provider {
        "claude_acp" | "claude_agent_acp" => "claude".to_string(),
        "codex_acp" => "codex".to_string(),
        "gemini_cli_acp" | "gemini_acp" => "gemini".to_string(),
        "auggie_acp" => "auggie".to_string(),
        "junie_acp" => "junie".to_string(),
        other => other.to_string(),
    }
}

fn steward_expected_output_paths(
    catalog: &AgentCatalogFile,
    agent: &AgentEntry,
    meta_root: &std::path::Path,
) -> Vec<String> {
    agent
        .outputs
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|output| steward_artifact_path(catalog, output, meta_root))
        .collect()
}

fn steward_artifact_path(
    catalog: &AgentCatalogFile,
    artifact_name: &str,
    meta_root: &std::path::Path,
) -> String {
    let meta_root = meta_root.to_string_lossy();
    catalog
        .artifacts
        .as_ref()
        .and_then(|artifacts| artifacts.get(artifact_name))
        .map(|template| {
            template
                .replace("${CHAINWORKS_META_ROOT:-.chainworks}", &meta_root)
                .replace("${CHAINWORKS_META_ROOT}", &meta_root)
                .replace("$CHAINWORKS_META_ROOT", &meta_root)
        })
        .unwrap_or_else(|| {
            meta_root.as_ref().to_string() + "/" + &sanitize_artifact_name_for_path(artifact_name)
        })
}

fn build_steward_agent_prompt(
    catalog: &AgentCatalogFile,
    agent: &AgentEntry,
    invocation: &crate::steward::service::StewardAgentInvocation,
    expected_output_paths: &[String],
) -> String {
    let mut parts = Vec::new();
    if let Some(prompt) = agent
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        parts.push(format!("## System Instructions\n{prompt}"));
        parts.push("---".to_string());
    }
    if let Some(skill_ref) = agent.skill_ref.as_deref() {
        if let Some(skill) = catalog
            .skills
            .as_ref()
            .and_then(|skills| skills.get(skill_ref))
        {
            if let Some(description) = skill.description.as_deref() {
                parts.push(format!(
                    "## Skill: {skill_ref}\nRole: {}\n{}",
                    agent.skill_role.as_deref().unwrap_or("default"),
                    description
                ));
            }
        }
    }
    parts.push(format!(
        "## Steward Active-Catalog IO\nAgent ID: {}\nCHAINWORKS_META_ROOT: {}\nRead inputs and write outputs under this root. Do not write outside it.",
        agent.id,
        invocation.chainworks_meta_root.display()
    ));
    if let Some(inputs) = agent.inputs.as_deref() {
        let inputs = inputs
            .iter()
            .map(|input| steward_artifact_path(catalog, input, &invocation.chainworks_meta_root))
            .collect::<Vec<_>>()
            .join("\n- ");
        parts.push(format!("Input artifact paths:\n- {inputs}"));
    }
    if !expected_output_paths.is_empty() {
        parts.push(format!(
            "Required output artifact paths:\n- {}",
            expected_output_paths.join("\n- ")
        ));
    }
    parts.join("\n\n")
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
            steward_runtime_inputs: None,
        }
    }

    pub fn new_with_steward_runtime_inputs(
        pool: SqlitePool,
        work_queue: WorkQueue,
        orchestrator: Arc<Orchestrator>,
        acp: Arc<AcpRuntimeManager>,
        events: EventSender,
        steward_runtime_inputs: Arc<crate::steward::config::StewardRuntimeInputs>,
    ) -> Self {
        Self {
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            steward_runtime_inputs: Some(steward_runtime_inputs),
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
                                    if let Err(e2) =
                                        executor.work_queue.fail(&item_id, &e.to_string()).await
                                    {
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
                                if let Err(e2) =
                                    self.work_queue.fail(&item_id, &e.to_string()).await
                                {
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
                let task_name = payload["task_name"]
                    .as_str()
                    .unwrap_or(&stage_id)
                    .to_string();
                let task_inputs: Vec<String> =
                    serde_json::from_value(payload["task_inputs"].clone()).unwrap_or_default();
                let task_outputs: Vec<String> =
                    serde_json::from_value(payload["task_outputs"].clone()).unwrap_or_default();
                let backend_profile_id = payload["backend_profile_id"].as_str().map(String::from);
                let permission_profile = payload["permission_profile"].as_str().map(String::from);
                let skill_ref = payload["skill_ref"].as_str().map(String::from);
                let skill_role = payload["skill_role"].as_str().map(String::from);
                let skill_snapshot_hash = payload["skill_snapshot_hash"].as_str().map(String::from);
                let requested_mcp_server_ids: Vec<String> =
                    serde_json::from_value(payload["requested_mcp_server_ids"].clone())
                        .unwrap_or_default();
                let mcp_resolution = crate::mcp::resolve_mcp_servers(
                    &requested_mcp_server_ids,
                    backend_profile_id.as_deref(),
                    &provider,
                );
                let requested_mcp_extensions_json =
                    serde_json::to_string(&mcp_resolution.report.requested_extensions)?;
                let predicted_mcp_extensions_json =
                    serde_json::to_string(&mcp_resolution.report.predicted_effective_extensions)?;
                let predicted_mcp_runtime_ids_json =
                    serde_json::to_string(&mcp_resolution.report.predicted_effective_runtime_ids)?;
                let denied_mcp_extensions_json =
                    serde_json::to_string(&mcp_resolution.report.denied_extensions)?;
                let mcp_blocking_issues_json =
                    serde_json::to_string(&mcp_resolution.report.blocking_issues)?;
                let output_contract = payload["output_contract"].as_str().map(String::from);
                let max_turns = payload["max_turns"].as_i64();
                let temperature = payload["temperature"].as_f64();
                let worktree_write_enabled =
                    payload["worktree_write_enabled"].as_bool().unwrap_or(false);
                let worktree_strategy = payload["worktree_strategy"].as_str().map(String::from);
                let session_reuse_scope = payload["session_reuse_scope"].as_str().map(String::from);
                let session_family_id = payload["session_family_id"].as_str().map(String::from);
                let declared_outputs: Vec<DeclaredOutput> =
                    serde_json::from_value(payload["declared_outputs"].clone()).unwrap_or_default();
                let expected_output_paths: Vec<String> = declared_outputs
                    .iter()
                    .flat_map(|declared| {
                        std::iter::once(declared.target_path.clone())
                            .chain(declared.companion_path.clone().into_iter())
                    })
                    .collect();

                let effective_working_directory = if worktree_write_enabled
                    || matches!(
                        worktree_strategy.as_deref(),
                        Some("dedicated") | Some("shared_implementation_worktree")
                    ) {
                    run.worktree_root
                        .clone()
                        .unwrap_or_else(|| run.workspace_root.clone())
                } else {
                    run.workspace_root.clone()
                };
                let workspace_mode = if worktree_write_enabled {
                    "write_enabled".to_string()
                } else {
                    "read_only".to_string()
                };
                let resolved_model = model.clone().unwrap_or_else(|| "default".into());
                let now = chrono::Utc::now();
                let agent_exec_id = domain::ids::AgentExecutionId::new();
                let owner_execution_lineage_id = stage_execution_id.to_string();
                let policy_input = SessionPolicyInput {
                    run_id: run_id.to_string(),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model: resolved_model.clone(),
                    working_directory: effective_working_directory.clone(),
                    workspace_mode: workspace_mode.clone(),
                    session_reuse_scope: session_reuse_scope.clone(),
                    session_family_id: session_family_id.clone(),
                    invocation_owner_key: {
                        let run_id_str = run_id.to_string();
                        invocation_owner_key(&InvocationOwnerKeyInput {
                            run_id: &run_id_str,
                            agent_id: &agent_id,
                            stage_lineage_id: &stage_id,
                            task_name: &task_name,
                            owner_execution_lineage_id: &owner_execution_lineage_id,
                        })
                    },
                    binding_fingerprint: binding_fingerprint(&BindingFingerprintInput {
                        agent_id: &agent_id,
                        provider: &provider,
                        model: model.as_deref(),
                        effort: effort.as_deref(),
                        prompt: &prompt,
                        working_directory: &effective_working_directory,
                        workspace_mode: &workspace_mode,
                        worktree_write_enabled,
                        worktree_strategy: worktree_strategy.as_deref(),
                        inputs: &task_inputs,
                        outputs: &task_outputs,
                        backend_profile: backend_profile_id.as_deref(),
                        permission_profile: permission_profile.as_deref(),
                        mcp_servers: &requested_mcp_server_ids,
                        skill_snapshot_hash: skill_snapshot_hash.as_deref(),
                        skill_ref: skill_ref.as_deref(),
                        skill_role: skill_role.as_deref(),
                        output_contract: output_contract.as_deref(),
                        max_turns,
                        temperature,
                    }),
                };

                let mut policy_decision: Option<SessionPolicyDecision> =
                    if session_reuse_scope.is_some() {
                        Some(ensure_policy(&self.pool, policy_input.clone()).await?)
                    } else {
                        None
                    };
                if let Some(decision) = policy_decision.as_ref() {
                    if decision.should_reuse_live_session
                        && !self
                            .acp
                            .has_live_session(
                                &decision.generation.id,
                                decision.generation.provider_session_id.as_deref(),
                            )
                            .await
                    {
                        sessions::end_generation(
                            &self.pool,
                            &decision.generation.id,
                            domain::session::SessionGenerationStatus::Invalidated,
                            "transport_missing_live_handle",
                            now,
                        )
                        .await?;
                        sessions::insert_event(
                            &self.pool,
                            &domain::session::SessionEvent {
                                id: uuid::Uuid::new_v4().to_string(),
                                lineage_id: decision.lineage.id.clone(),
                                generation_id: decision.generation.id.clone(),
                                event_type: domain::session::SessionEventType::Invalidated,
                                recorded_at: now,
                                details_json: Some(
                                    serde_json::json!({ "reason": "transport_missing_live_handle" })
                                        .to_string(),
                                ),
                            },
                        )
                        .await?;
                        policy_decision = Some(ensure_policy(&self.pool, policy_input).await?);
                    }
                }
                if let Some(decision) = policy_decision.as_ref() {
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        agent_id = %agent_id,
                        lineage_id = %decision.lineage.id,
                        generation_id = %decision.generation.id,
                        disposition = ?decision.disposition,
                        reuse_live_session = decision.should_reuse_live_session,
                        "Session policy evaluated"
                    );
                }
                if let Some(decision) = policy_decision.as_ref() {
                    self.persist_session_checkpoint_artifact_if_needed(
                        &run,
                        &stage_id,
                        &agent_id,
                        &provider,
                        model.clone(),
                        &prompt,
                        now,
                        decision,
                    )
                    .await?;
                }

                let agent_exec = domain::agent::AgentExecution {
                    id: agent_exec_id,
                    stage_execution_id,
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    status: domain::agent::AgentStatus::Running,
                    started_at: now,
                    completed_at: None,
                    owner_execution_lineage_id: Some(owner_execution_lineage_id),
                    session_lineage_id: policy_decision
                        .as_ref()
                        .map(|decision| decision.lineage.id.clone()),
                    session_generation_id: policy_decision
                        .as_ref()
                        .map(|decision| decision.generation.id.clone()),
                    rehydrated_from_checkpoint_artifact_id: policy_decision.as_ref().and_then(
                        |decision| {
                            decision
                                .generation
                                .rehydrated_from_checkpoint_artifact_id
                                .clone()
                        },
                    ),
                    invocation_owner_key: policy_decision
                        .as_ref()
                        .map(|decision| decision.generation.invocation_owner_key.clone()),
                    session_reuse_scope: session_reuse_scope.clone(),
                    session_family_id: session_family_id.clone(),
                    session_reuse_disposition: policy_decision.as_ref().and_then(|decision| {
                        serde_json::to_value(&decision.disposition)
                            .ok()
                            .and_then(|value| value.as_str().map(String::from))
                    }),
                    session_reset_reason: policy_decision
                        .as_ref()
                        .and_then(|decision| decision.session_reset_reason.clone()),
                    backend_profile_id: backend_profile_id.clone(),
                    requested_mcp_extensions_json: Some(requested_mcp_extensions_json.clone()),
                    predicted_mcp_extensions_json: Some(predicted_mcp_extensions_json.clone()),
                    predicted_mcp_runtime_ids_json: Some(predicted_mcp_runtime_ids_json.clone()),
                    actual_mcp_extensions_json: None,
                    actual_mcp_runtime_ids_json: None,
                    denied_mcp_extensions_json: Some(denied_mcp_extensions_json.clone()),
                    mcp_blocking_issues_json: Some(mcp_blocking_issues_json.clone()),
                    actual_mcp_observation_json: None,
                    mcp_session_startup_latency_ms: None,
                };
                agent_executions::insert(&self.pool, &agent_exec).await?;

                if !mcp_resolution.report.blocking_issues.is_empty() {
                    let completed_at = chrono::Utc::now();
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Failed,
                        completed_at,
                    )
                    .await?;
                    let blocked_actual_extensions_json = "[]".to_string();
                    let blocked_actual_runtime_ids_json = "[]".to_string();
                    let blocked_actual_observation_json = serde_json::to_string(
                        &serde_json::json!({
                            "source": "mcp_resolution_blocked_before_session_new",
                            "trust_level": "authoritative_no_session",
                            "actual_equals_predicted": false,
                            "provider_session_id": serde_json::Value::Null,
                            "actual_extensions": [],
                            "actual_runtime_ids": [],
                            "requested_extensions": mcp_resolution.report.requested_extensions.clone(),
                            "predicted_extensions": mcp_resolution.report.predicted_effective_extensions.clone(),
                            "predicted_runtime_ids": mcp_resolution.report.predicted_effective_runtime_ids.clone(),
                            "denied_extensions": mcp_resolution.report.denied_extensions.clone(),
                            "blocking_issues": mcp_resolution.report.blocking_issues.clone(),
                            "notes": [
                                "ACP session/new was not attempted because MCP resolution failed closed before runtime startup."
                            ],
                        }),
                    )?;
                    agent_executions::update_mcp_actual(
                        &self.pool,
                        agent_exec_id,
                        Some(&blocked_actual_extensions_json),
                        Some(&blocked_actual_runtime_ids_json),
                        Some(&blocked_actual_observation_json),
                        None,
                    )
                    .await?;
                    crate::recovery::persist_failed_stage_recovery_snapshot(
                        &self.pool,
                        stage_execution_id,
                        completed_at,
                    )
                    .await?;
                    let evidence_artifact =
                        crate::evidence::build_and_persist_failed_stage_evidence(
                            &self.pool,
                            crate::evidence::FailedStageEvidenceInput {
                                run: &run,
                                stage_id: &stage_id,
                                stage_execution_id,
                                agent_id: &agent_id,
                                agent_execution_id: agent_exec_id,
                                provider: &provider,
                                model: model.clone(),
                                failed_at: completed_at,
                            },
                        )
                        .await?;
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::ArtifactCreated {
                            run_id,
                            artifact_id: evidence_artifact.id,
                        });
                    stages::settle(
                        &self.pool,
                        stage_execution_id,
                        domain::stage::StageSettlementKind::Failed,
                        completed_at,
                    )
                    .await?;
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::RuntimeStatusChanged {
                            run_id,
                            stage_id: stage_id.clone(),
                            agent_id: agent_id.clone(),
                            provider: provider.clone(),
                            event_kind: "mcp_resolution_blocked".to_string(),
                        });
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::StageStatusChanged {
                            run_id,
                            stage_execution_id,
                            status: domain::stage::StageStatus::Failed,
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
                        agent_id = %agent_id,
                        blocking_issues = ?mcp_resolution.report.blocking_issues,
                        "InvokeAgent blocked before ACP startup by MCP resolution"
                    );
                    return Ok(());
                }

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

                let estimated_prompt_tokens =
                    std::cmp::max(1_i64, (prompt.chars().count() as i64) / 4);
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
                    keep_session_alive: policy_decision.is_some(),
                    reuse_existing_session: policy_decision
                        .as_ref()
                        .map(|decision| decision.should_reuse_live_session)
                        .unwrap_or(false),
                    session_generation_id: policy_decision
                        .as_ref()
                        .map(|decision| decision.generation.id.clone()),
                    provider_session_id: policy_decision
                        .as_ref()
                        .and_then(|decision| decision.generation.provider_session_id.clone()),
                    mcp_servers: mcp_resolution.payloads,
                };
                // Runtime event: session starting
                let _ = self
                    .events
                    .send(domain::events::DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id: stage_id.clone(),
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        event_kind: "session_started".to_string(),
                    });

                let result = self.acp.execute(req).await?;

                if !requested_mcp_server_ids.is_empty() {
                    let actual_mcp_extensions_json =
                        serde_json::to_string(&result.actual_mcp_extensions)?;
                    let actual_mcp_runtime_ids_json =
                        serde_json::to_string(&result.actual_mcp_runtime_ids)?;
                    let actual_mcp_observation_json = result
                        .mcp_observation
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?;
                    agent_executions::update_mcp_actual(
                        &self.pool,
                        agent_exec_id,
                        Some(&actual_mcp_extensions_json),
                        Some(&actual_mcp_runtime_ids_json),
                        actual_mcp_observation_json.as_deref(),
                        result.mcp_session_startup_latency_ms,
                    )
                    .await?;
                }

                if !declared_outputs.is_empty() {
                    materialize_declared_outputs_from_discovered_artifacts(
                        &declared_outputs,
                        &result.discovered_artifacts,
                    )?;
                }

                if let (Some(decision), Some(provider_session_id)) = (
                    policy_decision.as_ref(),
                    result.provider_session_id.as_deref(),
                ) {
                    let actual_input_tokens = result
                        .usage
                        .as_ref()
                        .and_then(|usage| usage.input_tokens)
                        .unwrap_or(estimated_prompt_tokens);
                    sessions::update_generation_usage(
                        &self.pool,
                        &decision.generation.id,
                        provider_session_id,
                        decision.generation.turn_count + 1,
                        actual_input_tokens,
                        result.cost_cents.unwrap_or(0),
                        actual_input_tokens,
                        result
                            .usage
                            .as_ref()
                            .and_then(|usage| usage.cached_input_tokens),
                        result.usage.as_ref().and_then(|usage| usage.output_tokens),
                        result
                            .usage
                            .as_ref()
                            .and_then(|usage| usage.model_context_window),
                        chrono::Utc::now(),
                    )
                    .await?;
                    sessions::set_active_generation(
                        &self.pool,
                        &decision.lineage.id,
                        Some(&decision.generation.id),
                    )
                    .await?;
                }

                // Runtime event: session finished (completed or failed)
                let event_kind = match result.status {
                    domain::agent::AgentStatus::Completed => "session_completed",
                    domain::agent::AgentStatus::Failed => "session_failed",
                    _ => "session_completed",
                };
                let _ = self
                    .events
                    .send(domain::events::DomainEvent::RuntimeStatusChanged {
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

                let undeclared_artifacts = self
                    .persist_undeclared_envelope_artifacts(
                        &run,
                        &declared_outputs,
                        &result.discovered_artifacts,
                        &stage_id,
                        &agent_id,
                        &provider,
                        model.clone(),
                        completed_at,
                    )
                    .await?;
                persisted_artifacts.extend(undeclared_artifacts);

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
                    Some(validate_task_outputs(&load_declared_output_bytes(
                        &declared_outputs,
                    )?))
                };
                let validation_failed = validation_summary
                    .as_ref()
                    .and_then(|summary| summary.failure_class.as_ref())
                    .is_some();
                let final_agent_status =
                    if result.status == AgentStatus::Completed && !validation_failed {
                        AgentStatus::Completed
                    } else {
                        AgentStatus::Failed
                    };

                if let Some(summary) = validation_summary {
                    if summary.failure_class.is_some() {
                        let validation_failure_record = build_validation_failure_record(
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
                        )?;
                        let validation_failure_json =
                            serde_json::to_string_pretty(&validation_failure_record)?;
                        stages::update_validation_failure_json(
                            &self.pool,
                            stage_execution_id,
                            &validation_failure_json,
                        )
                        .await?;
                        let validation_artifact = self
                            .persist_validation_failure_artifact(
                                &run,
                                &stage_id,
                                &agent_id,
                                &provider,
                                model.clone(),
                                agent_exec_id,
                                stage_execution_id,
                                validation_failure_record,
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

                if final_agent_status == AgentStatus::Failed {
                    crate::recovery::persist_failed_stage_recovery_snapshot(
                        &self.pool,
                        stage_execution_id,
                        completed_at,
                    )
                    .await?;
                    let evidence_artifact =
                        crate::evidence::build_and_persist_failed_stage_evidence(
                            &self.pool,
                            crate::evidence::FailedStageEvidenceInput {
                                run: &run,
                                stage_id: &stage_id,
                                stage_execution_id,
                                agent_id: &agent_id,
                                agent_execution_id: agent_exec_id,
                                provider: &provider,
                                model: model.clone(),
                                failed_at: completed_at,
                            },
                        )
                        .await?;
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::ArtifactCreated {
                            run_id,
                            artifact_id: evidence_artifact.id,
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::StageStatusChanged {
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

            WorkItemKind::StewardAnalysis => {
                let payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;
                let reason = payload["reason"].as_str().unwrap_or("manual").to_string();
                let artifact_base = payload["artifact_base"]
                    .as_str()
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var("CHAINWORKS_META_ROOT")
                            .ok()
                            .map(PathBuf::from)
                    })
                    .unwrap_or_else(|| PathBuf::from(".chainworks"));
                let runtime_inputs = self
                    .steward_runtime_inputs
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Steward runtime inputs are not configured"))?;
                let steward_agent_executor = BackgroundStewardAgentExecutor {
                    acp: self.acp.clone(),
                    runtime_inputs: runtime_inputs.clone(),
                };
                crate::steward::service::run_steward_analysis_with_executor(
                    &self.pool,
                    runtime_inputs,
                    crate::steward::StewardAnalysisRequest {
                        reason,
                        artifact_base,
                    },
                    Some(&steward_agent_executor),
                )
                .await?;
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
                .commit_and_push(
                    &worktree_root,
                    &delivery_config.target_branch,
                    &commit_message,
                )
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::RuntimeStatusChanged {
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

                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::RuntimeStatusChanged {
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::StageStatusChanged {
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
            let git_receipt: GitPushReceipt = self
                .load_release_artifact(run_id, "git_push_receipt")
                .await?;
            let release_manifest: ReleaseManifest = self
                .load_release_artifact(run_id, "release_manifest")
                .await?;
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
                    let _ = self
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
                        .await?;
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::RuntimeStatusChanged {
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::StageStatusChanged {
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::RuntimeStatusChanged {
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
                    let _ = self
                        .events
                        .send(domain::events::DomainEvent::StageStatusChanged {
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
        serde_json::from_str(json)
            .map_err(|e| anyhow::anyhow!("Invalid delivery_configuration_json: {}", e))
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
                    declared_machine_artifact_name(declared),
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

    async fn persist_undeclared_envelope_artifacts(
        &self,
        run: &domain::run::Run,
        declared_outputs: &[DeclaredOutput],
        discovered_artifacts: &[acp::DiscoveredArtifact],
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<domain::artifact::Artifact>> {
        let declared_names: std::collections::HashSet<&str> = declared_outputs
            .iter()
            .flat_map(|declared| {
                std::iter::once(declared.output_name.as_str())
                    .chain(declared.companion_output_name.as_deref().into_iter())
            })
            .collect();
        let mut artifacts_out = Vec::new();

        for discovered in discovered_artifacts {
            if discovered.source_path.is_some() || declared_names.contains(discovered.name.as_str())
            {
                continue;
            }

            let (format, extension) = infer_artifact_format_from_content(&discovered.content);
            let file_stem = sanitize_artifact_name_for_path(&discovered.name);
            let artifact_id = domain::ids::ArtifactId::new();
            let path = std::path::Path::new(&run.artifact_root)
                .join("undeclared_envelope_outputs")
                .join(stage_id)
                .join(format!("{}-{}.{}", file_stem, artifact_id, extension));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    anyhow::anyhow!(
                        "create undeclared envelope artifact dir {}: {}",
                        parent.display(),
                        e
                    )
                })?;
            }
            std::fs::write(&path, &discovered.content).map_err(|e| {
                anyhow::anyhow!(
                    "write undeclared envelope artifact {}: {}",
                    path.display(),
                    e
                )
            })?;

            let artifact = domain::artifact::Artifact {
                id: artifact_id,
                run_id: run.id,
                stage_id: stage_id.to_string(),
                agent_id: agent_id.to_string(),
                name: discovered.name.clone(),
                contract_id: format!("{}.output", provider),
                format,
                file_path: path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(path.metadata().map(|meta| meta.len() as i64).unwrap_or(0)),
                provider: provider.to_string(),
                model: model.clone(),
                created_at,
                is_pinned: false,
                report_kind: None,
                report_version: None,
            };
            artifacts::insert(&self.pool, &artifact).await?;
            let _ = self
                .events
                .send(domain::events::DomainEvent::ArtifactCreated {
                    run_id: run.id,
                    artifact_id: artifact.id,
                });
            artifacts_out.push(artifact);
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
        let size_bytes = artifact_path.metadata().ok().map(|meta| meta.len() as i64);
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
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
                run_id,
                artifact_id: artifact.id,
            });
        Ok(Some(artifact))
    }

    async fn persist_session_checkpoint_artifact_if_needed(
        &self,
        run: &domain::run::Run,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        prompt: &str,
        created_at: chrono::DateTime<chrono::Utc>,
        decision: &SessionPolicyDecision,
    ) -> Result<()> {
        let Some(checkpoint_id_raw) = decision
            .generation
            .rehydrated_from_checkpoint_artifact_id
            .as_deref()
        else {
            return Ok(());
        };
        let checkpoint_id: domain::ids::ArtifactId = checkpoint_id_raw
            .parse()
            .map_err(|e| anyhow::anyhow!("parse checkpoint artifact id: {}", e))?;
        if artifacts::find_by_id(&self.pool, checkpoint_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        let path = std::path::Path::new(&run.artifact_root)
            .join("session_checkpoints")
            .join(stage_id)
            .join(format!("{checkpoint_id}.json"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!("create session checkpoint dir {}: {}", parent.display(), e)
            })?;
        }

        let disposition = serde_json::to_value(&decision.disposition)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string));
        let payload = serde_json::json!({
            "kind": "session_checkpoint",
            "session_lineage_id": decision.lineage.id,
            "session_generation_id": decision.generation.id,
            "session_reuse_disposition": disposition,
            "prompt": prompt,
            "runtime_provider": provider,
            "runtime_model": model,
            "created_at": created_at.to_rfc3339(),
        });
        let bytes = serde_json::to_vec_pretty(&payload)?;
        std::fs::write(&path, &bytes).map_err(|e| {
            anyhow::anyhow!(
                "write session checkpoint artifact {}: {}",
                path.display(),
                e
            )
        })?;

        let artifact = domain::artifact::Artifact {
            id: checkpoint_id,
            run_id: run.id,
            stage_id: stage_id.to_string(),
            agent_id: agent_id.to_string(),
            name: "session_checkpoint".into(),
            contract_id: "session_checkpoint_v1".into(),
            format: domain::artifact::ArtifactFormat::Json,
            file_path: path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: Some(bytes.len() as i64),
            provider: provider.to_string(),
            model,
            created_at,
            is_pinned: false,
            report_kind: Some("session_checkpoint".into()),
            report_version: Some(1),
        };
        artifacts::insert(&self.pool, &artifact).await?;
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
                run_id: run.id,
                artifact_id: artifact.id,
            });
        Ok(())
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
        let path = std::path::Path::new(&run.artifact_root).join(format!("{artifact_name}.json"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!("create validation artifact dir {}: {}", parent.display(), e)
            })?;
        }
        let encoded = serde_json::to_string_pretty(&record)?;
        std::fs::write(&path, encoded).map_err(|e| {
            anyhow::anyhow!(
                "write validation failure artifact {}: {}",
                path.display(),
                e
            )
        })?;

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
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
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
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ConnectPublishService requires git_push_receipt and release_manifest inputs."
                )
            })?;
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
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
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
            if let Ok(plan) =
                workflow::compiler::compile(workflow_yaml_path, agent_catalog_yaml_path)
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
                run.current_state
                    .as_deref()
                    .unwrap_or("state_12_finalization"),
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
        let artifact = match artifacts_by_run
            .iter()
            .rev()
            .find(|artifact| artifact.name == name)
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialize_declared_outputs_writes_machine_and_companion_payloads() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("outputs/proposal_review.json");
        let companion_path = tmp.path().join("outputs/proposal_review.md");
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some(companion_path.to_string_lossy().into_owned()),
        };
        let discovered = vec![
            acp::DiscoveredArtifact {
                name: "proposal_review".to_string(),
                content: br#"{"status":"green"}"#.to_vec(),
                source_path: None,
            },
            acp::DiscoveredArtifact {
                name: "proposal_review_raw".to_string(),
                content: b"# Review\n".to_vec(),
                source_path: None,
            },
        ];

        materialize_declared_outputs_from_discovered_artifacts(&[declared], &discovered)
            .expect("envelope-derived outputs should be materialized to canonical paths");

        assert_eq!(
            std::fs::read_to_string(machine_path).unwrap(),
            r#"{"status":"green"}"#
        );
        assert_eq!(
            std::fs::read_to_string(companion_path).unwrap(),
            "# Review\n"
        );
    }
}
