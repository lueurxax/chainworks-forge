use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

use crate::release::{
    connect::ConnectPublishService,
    coordinator::ReleaseResult,
    git::{GitPushReceipt, GitReleaseService, ReleaseManifest},
    receipt::DeliveryReceiptBuilder,
};
use acp::AcpRuntimeManager;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, artifact_contracts,
    artifacts, ideas, projections, scheduler, sessions, stages, validation, work_items,
};
use db::work_item::{WorkItem, WorkItemKind};
use domain::agent::{
    AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, AgentStatus,
    OperatorActionHint,
};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{
    known_contract_id, parse_implementation_self_assessment_v2, ActiveArtifactGenerationInput,
    ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey, ContractParseContext,
    SourceGenerationImportDecision, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
};
use domain::ids::RunId;
use domain::provider::ProviderFamily;
use domain::run::DeliveryConfiguration;
use workflow::catalog::{AgentCatalogFile, AgentEntry};

use crate::contracts::{
    artifact_format_for_companion_output, artifact_format_for_machine_output,
    build_validation_failure_record, load_declared_output_bytes, validate_task_outputs,
    DeclaredOutput, TaskValidationSummary,
};
use crate::event_bus::EventSender;
use crate::failure_classifier::{classify_observation, observation_from_acp_error_message};
use crate::orchestrator::Orchestrator;
use crate::recovery::RecoveryService;
use crate::session::fingerprint::{
    binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
};
use crate::session::policy::{
    ensure_policy, ensure_policy_tx, SessionPolicyDecision, SessionPolicyInput,
};
use crate::work_queue::WorkQueue;

pub use domain::provider::InvokeAgentCapacityConfig;

pub struct BackgroundExecutor {
    pool: SqlitePool,
    work_queue: WorkQueue,
    orchestrator: Arc<Orchestrator>,
    acp: Arc<AcpRuntimeManager>,
    events: EventSender,
    steward_runtime_inputs: Option<Arc<crate::steward::config::StewardRuntimeInputs>>,
    invoke_agent_capacity: Arc<InvokeAgentCapacityConfig>,
}

fn is_first_party_acp_provider(provider: &str) -> bool {
    matches!(provider, "claude" | "codex" | "gemini" | "auggie" | "junie")
}

struct BackgroundStewardAgentExecutor {
    acp: Arc<AcpRuntimeManager>,
    runtime_inputs: Arc<crate::steward::config::StewardRuntimeInputs>,
}

#[derive(Clone, Debug)]
struct DeclaredContractImportResult {
    validation_summary: Option<TaskValidationSummary>,
    final_agent_status: AgentStatus,
    degraded_outputs_satisfy_stage: bool,
}

#[derive(Debug)]
enum PreparedDeclaredContractImport {
    RunStateAdvisory(ActiveArtifactGenerationInput),
    ContractGeneration(ActiveArtifactGenerationInput),
}

#[derive(Clone, Debug)]
pub struct ClaimedInvokeAgent {
    pub work_item: WorkItem,
    pub work_item_id: String,
    pub run_id: RunId,
    pub stage_execution_id: domain::ids::StageExecutionId,
    pub agent_execution_id: domain::ids::AgentExecutionId,
    pub source_work_item_id: String,
    pub session_generation_id: String,
    pub artifact_claim_key: ArtifactSourceGenerationClaimKey,
}

#[derive(Debug)]
struct InvokeAgentCapacitySnapshot {
    active_total: i64,
    active_provider: i64,
    active_run: i64,
    provider_cap: Option<usize>,
}

const INVOKE_AGENT_CANDIDATE_SCAN_LIMIT: i64 = 32;

fn scheduler_capacity_config(
    capacity: &InvokeAgentCapacityConfig,
) -> domain::provider::InvokeAgentCapacityConfig {
    capacity.clone()
}

pub async fn claim_next_invoke_agent_with_start(
    pool: &SqlitePool,
) -> Result<Option<ClaimedInvokeAgent>> {
    claim_next_invoke_agent_with_start_with_capacity(pool, &InvokeAgentCapacityConfig::default())
        .await
}

pub async fn claim_next_invoke_agent_with_start_with_capacity(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<ClaimedInvokeAgent>> {
    claim_next_invoke_agent_with_start_inner(pool, true, capacity).await
}

async fn claim_next_session_backed_invoke_agent_with_start(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<ClaimedInvokeAgent>> {
    claim_next_invoke_agent_with_start_inner(pool, false, capacity).await
}

async fn claim_next_invoke_agent_with_start_inner(
    pool: &SqlitePool,
    fail_sessionless: bool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<ClaimedInvokeAgent>> {
    let now = chrono::Utc::now();
    if !has_capacity_eligible_pending_invoke_agent_for_start(pool, capacity).await? {
        return Ok(None);
    }

    let tx_started = Instant::now();
    let mut tx =
        db::pool::begin_immediate_with_retry(pool, "executor.claim_start_invoke_agent").await?;
    let pending_candidates = work_items::select_pending_invoke_agents_for_start_tx(
        &mut tx,
        now,
        INVOKE_AGENT_CANDIDATE_SCAN_LIMIT,
    )
    .await?;
    let mut selected = None;
    let mut backpressured = 0usize;

    for pending_item in pending_candidates {
        let payload: serde_json::Value = serde_json::from_str(&pending_item.payload_json)?;
        let run_id = pending_item
            .run_id
            .ok_or_else(|| anyhow::anyhow!("InvokeAgent work item missing run_id"))?;
        let provider = payload["provider"].as_str().unwrap_or("unknown");

        match invoke_agent_capacity_available_tx(&mut tx, capacity, run_id, provider).await? {
            Ok(snapshot) => {
                let service_state =
                    scheduler::get_service_state_tx(&mut tx, "run", &run_id.to_string()).await?;
                let candidate = (
                    pending_item,
                    payload,
                    snapshot,
                    service_state.and_then(|state| state.last_served_at),
                );
                selected = match selected {
                    Some(current) if eligible_candidate_precedes(&current, &candidate) => {
                        Some(current)
                    }
                    _ => Some(candidate),
                };
            }
            Err(reason) => {
                backpressured += 1;
                debug!(
                    item_id = %pending_item.id,
                    provider = %provider,
                    run_id = %run_id,
                    reason = reason,
                    "InvokeAgent claim backpressured"
                );
            }
        }
    }

    let Some((pending_item, payload, snapshot, _last_served_at)) = selected else {
        tx.commit().await?;
        db::pool::log_write_transaction("executor.claim_start_invoke_agent.empty", tx_started);
        return Ok(None);
    };
    if backpressured > 0 {
        let provider = payload["provider"].as_str().unwrap_or("unknown");
        debug!(
            skipped_backpressured_items = backpressured,
            item_id = %pending_item.id,
            provider = %provider,
            active_total = snapshot.active_total,
            active_provider = snapshot.active_provider,
            active_run = snapshot.active_run,
            provider_cap = snapshot.provider_cap.map(|cap| cap as i64),
            "InvokeAgent claim skipped backpressured candidates and selected eligible work"
        );
    }

    if payload
        .pointer("/p058_claimed/agent_execution_id")
        .is_some()
    {
        let run_id = pending_item
            .run_id
            .ok_or_else(|| anyhow::anyhow!("InvokeAgent work item missing run_id"))?;
        let agent_execution_id: domain::ids::AgentExecutionId = payload
            .pointer("/p058_claimed/agent_execution_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("P058 claimed payload missing agent_execution_id"))?
            .parse()
            .map_err(|e: uuid::Error| anyhow::anyhow!("{}", e))?;
        let artifact_claim_key: ArtifactSourceGenerationClaimKey = payload
            .pointer("/p058_claimed/artifact_claim_key")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("P058 claimed payload missing artifact_claim_key"))
            .and_then(|value| serde_json::from_value(value).map_err(anyhow::Error::from))?;
        let policy_decision: SessionPolicyDecision = payload
            .pointer("/p058_claimed/session_policy_decision")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("P058 claimed payload missing session_policy_decision"))
            .and_then(|value| serde_json::from_value(value).map_err(anyhow::Error::from))?;
        let claimed_item =
            work_items::mark_claimed_running_tx(&mut tx, &pending_item.id, now).await?;
        record_scheduler_service_state_tx(&mut tx, run_id, &claimed_item.id, now).await?;
        let scheduler_capacity = scheduler_capacity_config(capacity);
        scheduler::refresh_queue_summaries_for_notification_tx(
            &mut tx,
            &scheduler_capacity,
            chrono::Utc::now(),
            "executor.claim_start_invoke_agent.preclaimed",
            0,
        )
        .await?;
        tx.commit().await?;
        db::pool::log_write_transaction("executor.claim_start_invoke_agent.preclaimed", tx_started);
        return Ok(Some(ClaimedInvokeAgent {
            work_item_id: claimed_item.id.clone(),
            source_work_item_id: artifact_claim_key.source_work_item_id.clone(),
            work_item: claimed_item,
            run_id,
            stage_execution_id: artifact_claim_key.stage_execution_id,
            agent_execution_id,
            session_generation_id: policy_decision.generation.id,
            artifact_claim_key,
        }));
    }
    if payload["session_reuse_scope"].as_str().is_none() {
        if fail_sessionless {
            work_items::fail_tx(
                &mut tx,
                &pending_item.id,
                "InvokeAgent payload missing session_reuse_scope; P058 claim/start requires session ownership",
                now,
            )
            .await?;
            let scheduler_capacity = scheduler_capacity_config(capacity);
            scheduler::refresh_queue_summaries_for_notification_tx(
                &mut tx,
                &scheduler_capacity,
                chrono::Utc::now(),
                "executor.claim_start_invoke_agent.sessionless_rejected",
                0,
            )
            .await?;
            tx.commit().await?;
            db::pool::log_write_transaction(
                "executor.claim_start_invoke_agent.sessionless_rejected",
                tx_started,
            );
            return Ok(None);
        }
        let run_id = pending_item
            .run_id
            .ok_or_else(|| anyhow::anyhow!("InvokeAgent work item missing run_id"))?;
        let stage_execution_id: domain::ids::StageExecutionId = payload["stage_execution_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing 'stage_execution_id'"))?
            .parse()
            .map_err(|e: uuid::Error| anyhow::anyhow!("{}", e))?;
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        let claimed_item =
            work_items::mark_claimed_running_tx(&mut tx, &pending_item.id, now).await?;
        record_scheduler_service_state_tx(&mut tx, run_id, &claimed_item.id, now).await?;
        let artifact_claim_key = ArtifactSourceGenerationClaimKey {
            run_id,
            stage_execution_id,
            agent_execution_id,
            source_work_item_id: claimed_item.id.clone(),
        };
        let scheduler_capacity = scheduler_capacity_config(capacity);
        scheduler::refresh_queue_summaries_for_notification_tx(
            &mut tx,
            &scheduler_capacity,
            chrono::Utc::now(),
            "executor.claim_start_invoke_agent.sessionless",
            0,
        )
        .await?;
        tx.commit().await?;
        db::pool::log_write_transaction(
            "executor.claim_start_invoke_agent.sessionless",
            tx_started,
        );
        return Ok(Some(ClaimedInvokeAgent {
            work_item_id: claimed_item.id.clone(),
            source_work_item_id: claimed_item.id.clone(),
            work_item: claimed_item,
            run_id,
            stage_execution_id,
            agent_execution_id,
            session_generation_id: String::new(),
            artifact_claim_key,
        }));
    }
    let run_id = pending_item
        .run_id
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent work item missing run_id"))?;
    let run_id_str = run_id.to_string();
    let stage_id = payload["stage_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing 'stage_id'"))?
        .to_string();
    let stage_execution_id: domain::ids::StageExecutionId = payload["stage_execution_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing 'stage_execution_id'"))?
        .parse()
        .map_err(|e: uuid::Error| anyhow::anyhow!("{}", e))?;
    let agent_id = payload["agent_id"]
        .as_str()
        .unwrap_or(&stage_id)
        .to_string();
    let provider = payload["provider"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing 'provider'"))?
        .to_string();
    let model = payload["model"].as_str().map(String::from);
    let resolved_model = model.clone().unwrap_or_else(|| "default".into());
    let effort = payload["effort"].as_str().map(String::from);
    let prompt = payload["prompt"]
        .as_str()
        .unwrap_or(&format!("Execute stage {} for run {}", stage_id, run_id))
        .to_string();
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
        serde_json::from_value(payload["requested_mcp_server_ids"].clone()).unwrap_or_default();
    let output_contract = payload["output_contract"].as_str().map(String::from);
    let max_turns = payload["max_turns"].as_i64();
    let temperature = payload["temperature"].as_f64();
    let worktree_write_enabled = payload["worktree_write_enabled"].as_bool().unwrap_or(false);
    let worktree_strategy = payload["worktree_strategy"].as_str().map(String::from);
    let session_reuse_scope = payload["session_reuse_scope"].as_str().map(String::from);
    let session_family_id = payload["session_family_id"].as_str().map(String::from);

    let run_row = sqlx::query("SELECT workspace_root, worktree_root FROM runs WHERE id = ?1")
        .bind(&run_id_str)
        .fetch_one(&mut *tx)
        .await?;
    let workspace_root: String = run_row.get("workspace_root");
    let worktree_root: Option<String> = run_row.get("worktree_root");
    let effective_working_directory = if worktree_write_enabled
        || matches!(
            worktree_strategy.as_deref(),
            Some("dedicated") | Some("shared_implementation_worktree")
        ) {
        worktree_root.unwrap_or_else(|| workspace_root.clone())
    } else {
        workspace_root
    };
    let workspace_mode = if worktree_write_enabled {
        "write_enabled".to_string()
    } else {
        "read_only".to_string()
    };

    let owner_execution_lineage_id = stage_execution_id.to_string();
    let invocation_owner_key = invocation_owner_key(&InvocationOwnerKeyInput {
        run_id: &run_id_str,
        agent_id: &agent_id,
        stage_lineage_id: &stage_id,
        task_name: &task_name,
        owner_execution_lineage_id: &owner_execution_lineage_id,
    });
    let policy_input = SessionPolicyInput {
        run_id: run_id_str,
        agent_id: agent_id.clone(),
        provider: provider.clone(),
        model: resolved_model.clone(),
        working_directory: effective_working_directory.clone(),
        workspace_mode: workspace_mode.clone(),
        session_reuse_scope: session_reuse_scope.clone(),
        session_family_id: session_family_id.clone(),
        invocation_owner_key,
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
    let policy_decision = ensure_policy_tx(&mut tx, policy_input).await?;
    let mut claimed_item =
        work_items::mark_claimed_running_tx(&mut tx, &pending_item.id, now).await?;
    record_scheduler_service_state_tx(&mut tx, run_id, &claimed_item.id, now).await?;

    let agent_execution_id = domain::ids::AgentExecutionId::new();
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
    let mcp_blocking_issues_json = serde_json::to_string(&mcp_resolution.report.blocking_issues)?;

    let agent_exec = domain::agent::AgentExecution {
        id: agent_execution_id,
        stage_execution_id,
        agent_id,
        provider,
        model,
        status: domain::agent::AgentStatus::Running,
        started_at: now,
        completed_at: None,
        owner_execution_lineage_id: Some(owner_execution_lineage_id),
        session_lineage_id: Some(policy_decision.lineage.id.clone()),
        session_generation_id: Some(policy_decision.generation.id.clone()),
        rehydrated_from_checkpoint_artifact_id: policy_decision
            .generation
            .rehydrated_from_checkpoint_artifact_id
            .clone(),
        invocation_owner_key: Some(policy_decision.generation.invocation_owner_key.clone()),
        session_reuse_scope,
        session_family_id,
        session_reuse_disposition: serde_json::to_value(&policy_decision.disposition)
            .ok()
            .and_then(|value| value.as_str().map(String::from)),
        session_reset_reason: policy_decision.session_reset_reason.clone(),
        backend_profile_id,
        requested_mcp_extensions_json: Some(requested_mcp_extensions_json),
        predicted_mcp_extensions_json: Some(predicted_mcp_extensions_json),
        predicted_mcp_runtime_ids_json: Some(predicted_mcp_runtime_ids_json),
        actual_mcp_extensions_json: None,
        actual_mcp_runtime_ids_json: None,
        denied_mcp_extensions_json: Some(denied_mcp_extensions_json),
        mcp_blocking_issues_json: Some(mcp_blocking_issues_json),
        actual_mcp_observation_json: None,
        mcp_session_startup_latency_ms: None,
    };
    agent_executions::insert_tx(&mut tx, &agent_exec).await?;
    let mut runtime_facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    runtime_facts.session_reuse_reason =
        Some(session_reuse_reason_for_policy_decision(&policy_decision));
    agent_execution_runtime_facts::upsert_tx(&mut tx, &runtime_facts).await?;

    let artifact_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id,
        agent_execution_id,
        source_work_item_id: claimed_item.id.clone(),
    };
    artifact_contracts::insert_source_generation_claim_tx(
        &mut tx,
        ArtifactSourceGenerationClaim {
            key: artifact_claim_key.clone(),
            current_session_generation_id: Some(policy_decision.generation.id.clone()),
            claim_state: domain::agent::ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await?;
    artifact_contracts::finalize_pending_retry_supersession_tx(
        &mut tx,
        &claimed_item.id,
        agent_execution_id,
    )
    .await?;

    let mut claimed_payload = payload.clone();
    if let Some(object) = claimed_payload.as_object_mut() {
        object.insert(
            "p058_claimed".to_string(),
            serde_json::json!({
                "agent_execution_id": agent_execution_id.to_string(),
                "artifact_claim_key": artifact_claim_key,
                "session_policy_decision": policy_decision,
            }),
        );
    }
    claimed_item.payload_json = serde_json::to_string(&claimed_payload)?;
    work_items::update_payload_json_tx(&mut tx, &claimed_item.id, &claimed_item.payload_json)
        .await?;

    let scheduler_capacity = scheduler_capacity_config(capacity);
    scheduler::refresh_queue_summaries_for_notification_tx(
        &mut tx,
        &scheduler_capacity,
        chrono::Utc::now(),
        "executor.claim_start_invoke_agent",
        0,
    )
    .await?;

    tx.commit().await?;
    db::pool::log_write_transaction("executor.claim_start_invoke_agent", tx_started);

    Ok(Some(ClaimedInvokeAgent {
        work_item_id: claimed_item.id.clone(),
        source_work_item_id: claimed_item.id.clone(),
        work_item: claimed_item,
        run_id,
        stage_execution_id,
        agent_execution_id,
        session_generation_id: policy_decision.generation.id,
        artifact_claim_key,
    }))
}

fn eligible_candidate_precedes(
    current: &(
        WorkItem,
        serde_json::Value,
        InvokeAgentCapacitySnapshot,
        Option<chrono::DateTime<chrono::Utc>>,
    ),
    candidate: &(
        WorkItem,
        serde_json::Value,
        InvokeAgentCapacitySnapshot,
        Option<chrono::DateTime<chrono::Utc>>,
    ),
) -> bool {
    match (current.3.as_ref(), candidate.3.as_ref()) {
        (None, Some(_)) => return true,
        (Some(_), None) => return false,
        (Some(current_last_served), Some(candidate_last_served))
            if current_last_served != candidate_last_served =>
        {
            return current_last_served < candidate_last_served;
        }
        _ => {}
    }

    if current.0.scheduled_at != candidate.0.scheduled_at {
        return current.0.scheduled_at < candidate.0.scheduled_at;
    }
    current.0.id <= candidate.0.id
}

async fn record_scheduler_service_state_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    work_item_id: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    scheduler::upsert_service_state_tx(
        tx,
        &scheduler::SchedulerServiceState {
            scope: "run".into(),
            scope_id: run_id.to_string(),
            last_served_at: Some(now),
            last_claimed_work_item_id: Some(work_item_id.to_string()),
            updated_at: now,
        },
    )
    .await
}

pub async fn has_capacity_eligible_pending_invoke_agent_for_start(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<bool> {
    let pending_candidates = work_items::select_pending_invoke_agents_for_start(
        pool,
        chrono::Utc::now(),
        INVOKE_AGENT_CANDIDATE_SCAN_LIMIT,
    )
    .await?;

    for pending_item in pending_candidates {
        let Some(run_id) = pending_item.run_id else {
            return Ok(true);
        };
        let payload: serde_json::Value = match serde_json::from_str(&pending_item.payload_json) {
            Ok(payload) => payload,
            Err(_) => return Ok(true),
        };
        let provider = payload["provider"].as_str().unwrap_or("unknown");
        if invoke_agent_capacity_available(pool, capacity, run_id, provider)
            .await?
            .is_ok()
        {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn invoke_agent_capacity_available(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
    run_id: RunId,
    provider: &str,
) -> Result<std::result::Result<InvokeAgentCapacitySnapshot, &'static str>> {
    let running_status = AgentStatus::Running.to_string();
    let active_total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions
           WHERE status = ?1"#,
    )
    .bind(&running_status)
    .fetch_one(pool)
    .await?;
    if active_total >= capacity.global_active_agent_executions as i64 {
        return Ok(Err("global_capacity"));
    }

    let provider_family = ProviderFamily::resolve(provider)?;
    let active_provider: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions
           WHERE status = ?1 AND provider_family = ?2"#,
    )
    .bind(&running_status)
    .bind(provider_family.as_str())
    .fetch_one(pool)
    .await?;
    let provider_cap = capacity.provider_caps.get(&provider_family).copied();
    if let Some(provider_cap) = provider_cap {
        if active_provider >= provider_cap as i64 {
            return Ok(Err("provider_capacity"));
        }
    }

    let active_run: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions ae
           INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
           WHERE ae.status = ?1 AND se.run_id = ?2"#,
    )
    .bind(&running_status)
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await?;
    if active_run >= capacity.per_run_active_agent_executions as i64 {
        return Ok(Err("run_capacity"));
    }

    Ok(Ok(InvokeAgentCapacitySnapshot {
        active_total,
        active_provider,
        active_run,
        provider_cap,
    }))
}

async fn invoke_agent_capacity_available_tx(
    tx: &mut Transaction<'_, Sqlite>,
    capacity: &InvokeAgentCapacityConfig,
    run_id: RunId,
    provider: &str,
) -> Result<std::result::Result<InvokeAgentCapacitySnapshot, &'static str>> {
    let running_status = AgentStatus::Running.to_string();
    let active_total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions
           WHERE status = ?1"#,
    )
    .bind(&running_status)
    .fetch_one(&mut **tx)
    .await?;
    if active_total >= capacity.global_active_agent_executions as i64 {
        return Ok(Err("global_capacity"));
    }

    let provider_family = ProviderFamily::resolve(provider)?;
    let active_provider: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions
           WHERE status = ?1 AND provider_family = ?2"#,
    )
    .bind(&running_status)
    .bind(provider_family.as_str())
    .fetch_one(&mut **tx)
    .await?;
    let provider_cap = capacity.provider_caps.get(&provider_family).copied();
    if let Some(provider_cap) = provider_cap {
        if active_provider >= provider_cap as i64 {
            return Ok(Err("provider_capacity"));
        }
    }

    let active_run: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions ae
           INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
           WHERE ae.status = ?1 AND se.run_id = ?2"#,
    )
    .bind(&running_status)
    .bind(run_id.to_string())
    .fetch_one(&mut **tx)
    .await?;
    if active_run >= capacity.per_run_active_agent_executions as i64 {
        return Ok(Err("run_capacity"));
    }

    Ok(Ok(InvokeAgentCapacitySnapshot {
        active_total,
        active_provider,
        active_run,
        provider_cap,
    }))
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
                chainworks_meta_root: Some(
                    invocation
                        .chainworks_meta_root
                        .to_string_lossy()
                        .into_owned(),
                ),
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
    for declared in declared_outputs {
        if let Some(artifact) = find_discovered_artifact_for_output(
            discovered_artifacts,
            &declared.output_name,
            &declared.target_path,
        ) {
            write_discovered_output(&declared.target_path, &artifact.content)?;
        }

        if let (Some(companion_name), Some(companion_path)) = (
            declared.companion_output_name.as_deref(),
            declared.companion_path.as_deref(),
        ) {
            if let Some(artifact) = find_discovered_artifact_for_output(
                discovered_artifacts,
                companion_name,
                companion_path,
            ) {
                write_discovered_output(companion_path, &artifact.content)?;
            }
        }
    }

    Ok(())
}

fn degraded_policy_allows_valid_failed_outputs(
    policy: &workflow::plan::DegradedOutputPolicy,
    validation: &TaskValidationSummary,
    failure_kind: &str,
) -> bool {
    if policy.mode != "allow_valid_contract_outputs" {
        return false;
    }
    if policy.max_settlement != "valid_outputs_from_failed_execution" {
        return false;
    }
    if !policy.failure_kinds.is_empty()
        && !policy
            .failure_kinds
            .iter()
            .any(|allowed| allowed == failure_kind)
    {
        return false;
    }
    if validation.failure_class.is_some() || validation.output_results.is_empty() {
        return false;
    }
    validation.output_results.iter().all(|result| {
        result.status == domain::validation::ValidationStatus::Passed
            && result.contract_id.as_ref().is_some_and(|contract_id| {
                policy
                    .contracts
                    .iter()
                    .any(|allowed| allowed == contract_id)
            })
    })
}

fn runtime_facts_for_acp_error(
    agent_exec_id: domain::ids::AgentExecutionId,
    error: &Error,
    now: chrono::DateTime<chrono::Utc>,
) -> AgentExecutionRuntimeFacts {
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, now);
    let message = error.to_string();
    let classification = classify_observation(observation_from_acp_error_message(&message));
    facts.failure_kind = Some(classification.failure_kind);
    facts.operator_action_hint = Some(classification.operator_action_hint);
    facts.retry_after = classification.retry_after;
    facts.failure_message_redacted = Some(redact_runtime_message(&message));
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    facts.transport_error_code = classification.transport_error_code;
    facts.supervision_classification = classification.supervision_classification;
    facts
}

fn is_reused_live_session_transport_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no live acp session registered for generation id")
        || lower.contains("acp: send session/prompt")
        || lower.contains("write acp message to subprocess stdin")
        || lower.contains("broken pipe")
        || lower.contains("epipe")
        || lower.contains("stdout closed")
        || lower.contains("transport closed")
}

fn runtime_facts_for_execution_result(
    agent_exec_id: domain::ids::AgentExecutionId,
    result_status: AgentStatus,
    validation_summary: Option<&TaskValidationSummary>,
    observed_failure_kind: Option<AgentFailureKind>,
    now: chrono::DateTime<chrono::Utc>,
    close_diagnostic: Option<&acp::AcpCloseDiagnostic>,
) -> AgentExecutionRuntimeFacts {
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, now);
    facts.valid_required_outputs = validation_summary.is_some_and(|summary| {
        summary.failure_class.is_none()
            && !summary.output_results.is_empty()
            && summary
                .output_results
                .iter()
                .all(|result| result.status == domain::validation::ValidationStatus::Passed)
    });
    match validation_summary.and_then(|summary| summary.failure_class.as_ref()) {
        Some(domain::validation::ValidationFailureClass::NoOutputProduced) => {
            facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
            facts.operator_action_hint = Some(OperatorActionHint::Retry);
            facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        }
        Some(domain::validation::ValidationFailureClass::OutputContractMismatch)
        | Some(domain::validation::ValidationFailureClass::EmptyOutput)
        | Some(domain::validation::ValidationFailureClass::PersistenceFailure) => {
            facts.failure_kind = Some(AgentFailureKind::InvalidOutputContract);
            facts.operator_action_hint = Some(OperatorActionHint::Retry);
            facts.output_settlement = AgentOutputSettlement::InvalidRequiredOutputs;
        }
        None if result_status == AgentStatus::Failed && facts.valid_required_outputs => {
            facts.failure_kind =
                Some(observed_failure_kind.unwrap_or(AgentFailureKind::ProviderInternalError));
            facts.operator_action_hint = Some(OperatorActionHint::Retry);
            facts.output_settlement = AgentOutputSettlement::ValidOutputsFromFailedExecution;
        }
        None if result_status == AgentStatus::Failed => {
            facts.failure_kind =
                Some(observed_failure_kind.unwrap_or(AgentFailureKind::ProviderInternalError));
            facts.operator_action_hint = Some(OperatorActionHint::Retry);
            facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        }
        None => {
            facts.output_settlement = AgentOutputSettlement::ValidOutputsFromCompletedExecution;
        }
    }
    if let Some(close_diagnostic) = close_diagnostic {
        facts.transport_error_code = close_diagnostic
            .transport_error_code
            .clone()
            .or(facts.transport_error_code);
        facts.provider_exit_status = close_diagnostic
            .provider_exit_status
            .or(facts.provider_exit_status);
        facts.failure_message_redacted = Some(redact_runtime_message(&close_diagnostic.message));
        facts.supervision_classification = Some(
            if facts.valid_required_outputs {
                "nonblocking_close_after_valid_output"
            } else {
                "acp_close_diagnostic"
            }
            .to_string(),
        );
    }
    facts
}

fn session_reuse_reason_for_policy_decision(decision: &SessionPolicyDecision) -> String {
    match decision.disposition {
        domain::session::SessionReuseDisposition::Fresh => "legacy_unknown".to_string(),
        domain::session::SessionReuseDisposition::Reused => "same_family_within_run".to_string(),
        domain::session::SessionReuseDisposition::ReusedAfterResume => {
            "budget_guardrail".to_string()
        }
        domain::session::SessionReuseDisposition::FreshAfterReset => "operator_reset".to_string(),
        domain::session::SessionReuseDisposition::FreshAfterInvalidation => {
            "generation_superseded".to_string()
        }
        domain::session::SessionReuseDisposition::FreshAfterBudget => {
            "budget_guardrail".to_string()
        }
        domain::session::SessionReuseDisposition::FreshAfterCompaction => {
            "budget_guardrail".to_string()
        }
        domain::session::SessionReuseDisposition::FreshAfterTransportError => {
            "transport_error".to_string()
        }
        domain::session::SessionReuseDisposition::FreshAfterTimeout => "timeout".to_string(),
        domain::session::SessionReuseDisposition::FreshSessionRequired => {
            match decision.session_reset_reason.as_deref() {
                Some("provider_mismatch") | Some("binding_fingerprint_changed") => {
                    "provider_mismatch".to_string()
                }
                Some("policy_forbid")
                | Some("scope_none_requires_fresh_session")
                | Some("invocation_owner_changed") => "policy_forbid".to_string(),
                _ => "policy_forbid".to_string(),
            }
        }
        domain::session::SessionReuseDisposition::UnverifiableSessionHistory => {
            "unverifiable_history".to_string()
        }
    }
}

fn observed_failure_kind_for_execution_result(
    result_status: &AgentStatus,
    transcript_text: Option<&str>,
) -> Option<AgentFailureKind> {
    if *result_status != AgentStatus::Failed {
        return None;
    }
    transcript_text
        .map(|text| classify_observation(observation_from_acp_error_message(text)).failure_kind)
}

fn redact_runtime_message(message: &str) -> String {
    let mut scrubbed = message.to_string();
    for (name, value) in std::env::vars() {
        let key = name.to_ascii_lowercase();
        if value.len() >= 6
            && (key.contains("token")
                || key.contains("api_key")
                || key.contains("apikey")
                || key.contains("secret")
                || key.contains("password"))
        {
            scrubbed = scrubbed.replace(&value, "[redacted]");
        }
    }

    let tokens: Vec<&str> = scrubbed.split_whitespace().collect();
    let mut redacted = Vec::with_capacity(tokens.len());
    let mut redact_next = false;
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        let normalized = lower.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';'));
        if redact_next {
            redacted.push("[redacted]".to_string());
            redact_next = false;
            continue;
        }

        if normalized == "bearer" {
            redacted.push(token.to_string());
            redact_next = true;
            continue;
        }

        if matches!(
            normalized.trim_end_matches(':'),
            "token" | "api_key" | "apikey" | "secret" | "password"
        ) {
            redacted.push(token.to_string());
            redact_next = true;
            continue;
        }

        if normalized.ends_with("bearer") || normalized.contains("authorization:bearer") {
            redacted.push(token.to_string());
            redact_next = true;
            continue;
        }

        if let Some(redacted_assignment) = redact_sensitive_assignment(token) {
            redacted.push(redacted_assignment);
            continue;
        }

        if normalized.starts_with("sk-")
            || normalized.starts_with("ghp_")
            || normalized.contains("/.ssh/")
            || normalized.ends_with("/id_rsa")
            || normalized.contains("/id_rsa")
            || normalized.ends_with(".env")
            || normalized.contains("/.env")
        {
            redacted.push("[redacted]".to_string());
            continue;
        }

        redacted.push(token.to_string());
    }
    redacted.join(" ")
}

fn redact_sensitive_assignment(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    let separator = token.find('=').or_else(|| token.find(':'))?;
    if separator + 1 >= token.len() {
        return None;
    }
    let key = &lower[..separator];
    let sensitive = key.contains("token")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization");
    if !sensitive {
        return None;
    }
    Some(format!("{}[redacted]", &token[..=separator]))
}

fn find_discovered_artifact_for_output<'a>(
    discovered_artifacts: &'a [acp::DiscoveredArtifact],
    output_name: &str,
    target_path: &str,
) -> Option<&'a acp::DiscoveredArtifact> {
    discovered_artifacts
        .iter()
        .find(|artifact| artifact.name == output_name || artifact.name == target_path)
}

fn discovered_artifact_matches_declared_output(
    discovered: &acp::DiscoveredArtifact,
    declared: &DeclaredOutput,
) -> bool {
    discovered.name == declared.output_name
        || discovered.name == declared.target_path
        || declared
            .companion_output_name
            .as_deref()
            .is_some_and(|name| discovered.name == name)
        || declared
            .companion_path
            .as_deref()
            .is_some_and(|path| discovered.name == path)
}

fn declared_machine_artifact_name<'a>(declared: &'a DeclaredOutput) -> &'a str {
    declared
        .schema
        .as_ref()
        .and_then(|schema| schema.normalized_artifact_name.as_deref())
        .unwrap_or(declared.output_name.as_str())
}

fn extract_contract_status_from_file(contract_id: &str, path: &str) -> Result<Option<String>> {
    let bytes = std::fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if contract_id == IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID {
        let summary = parse_implementation_self_assessment_v2(
            &value,
            ContractParseContext {
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(path.to_string()),
                ..ContractParseContext::default()
            },
        );
        return Ok(Some(summary.status.to_string()));
    }
    let field_name = if contract_id == "audit_report_v1" {
        "implementation_status"
    } else {
        "status"
    };
    Ok(value
        .get(field_name)
        .or_else(|| value.get("status"))
        .and_then(|value| value.as_str())
        .map(str::to_string))
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
            '/' | '\\' | ':' => '_',
            c if c.is_ascii_control() => '_',
            _ => c,
        })
        .collect();
    if sanitized.is_empty() {
        "artifact".to_string()
    } else {
        sanitized
    }
}

fn is_implementation_self_assessment_artifact(artifact: &domain::artifact::Artifact) -> bool {
    artifact.contract_id
        == domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID
        || artifact.contract_id == "implementation_self_assessment_v1"
        || artifact.contract_id == "implementation_self_assessment"
        || artifact.name == "implementation_self_assessment"
        || artifact.name == "implementation_self_assessment_v2"
        || artifact
            .file_path
            .replace('\\', "/")
            .ends_with(domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH)
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
        Self::new_with_capacity(
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            InvokeAgentCapacityConfig::default(),
        )
    }

    pub fn new_with_capacity(
        pool: SqlitePool,
        work_queue: WorkQueue,
        orchestrator: Arc<Orchestrator>,
        acp: Arc<AcpRuntimeManager>,
        events: EventSender,
        invoke_agent_capacity: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            steward_runtime_inputs: None,
            invoke_agent_capacity: Arc::new(invoke_agent_capacity),
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
        Self::new_with_steward_runtime_inputs_and_capacity(
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            steward_runtime_inputs,
            InvokeAgentCapacityConfig::default(),
        )
    }

    pub fn new_with_steward_runtime_inputs_and_capacity(
        pool: SqlitePool,
        work_queue: WorkQueue,
        orchestrator: Arc<Orchestrator>,
        acp: Arc<AcpRuntimeManager>,
        events: EventSender,
        steward_runtime_inputs: Arc<crate::steward::config::StewardRuntimeInputs>,
        invoke_agent_capacity: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            steward_runtime_inputs: Some(steward_runtime_inputs),
            invoke_agent_capacity: Arc::new(invoke_agent_capacity),
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
        if let Some(claimed) = claim_next_session_backed_invoke_agent_with_start(
            &self.pool,
            &self.invoke_agent_capacity,
        )
        .await?
        {
            self.refresh_scheduler_projection_for_invoke_capacity()
                .await?;
            let item_id = claimed.work_item_id.clone();
            let run_id = Some(claimed.run_id);
            if claimed.session_generation_id.is_empty() {
                let payload: serde_json::Value =
                    serde_json::from_str(&claimed.work_item.payload_json)?;
                let provider = payload["provider"].as_str().unwrap_or_default();
                let agent_id = payload["agent_id"].as_str().unwrap_or_default();
                if !self.is_release_agent(agent_id)
                    && is_first_party_acp_provider(provider)
                    && self.acp.get_adapter(provider).is_none()
                {
                    self.work_queue
                        .fail(
                            &item_id,
                            "InvokeAgent payload missing session_reuse_scope; P058 claim/start requires session ownership",
                        )
                        .await?;
                    return Ok(false);
                }
            }
            info!(item_id = %item_id, kind = %WorkItemKind::InvokeAgent, "process_next_item: processing claimed InvokeAgent");
            match self.process_item(claimed.work_item).await {
                Ok(()) => {
                    self.work_queue.complete(&item_id).await?;
                    return Ok(true);
                }
                Err(e) => {
                    self.work_queue.fail(&item_id, &e.to_string()).await?;
                    self.enqueue_advance_after_invoke_failure(&item_id, run_id)
                        .await;
                    return Err(e);
                }
            }
        }

        self.refresh_scheduler_projection_if_needed().await?;

        match self.work_queue.claim_next().await? {
            Some(item) => {
                let item_id = item.id.clone();
                let kind = item.kind.clone();
                let run_id = item.run_id;
                info!(item_id = %item_id, kind = %kind, "process_next_item: processing");
                match self.process_item(item).await {
                    Ok(()) => {
                        self.work_queue.complete(&item_id).await?;
                        Ok(true)
                    }
                    Err(e) => {
                        self.work_queue.fail(&item_id, &e.to_string()).await?;
                        if matches!(kind, WorkItemKind::InvokeAgent) {
                            self.enqueue_advance_after_invoke_failure(&item_id, run_id)
                                .await;
                        }
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
            match claim_next_session_backed_invoke_agent_with_start(
                &self.pool,
                &self.invoke_agent_capacity,
            )
            .await
            {
                Ok(Some(claimed)) => {
                    if let Err(e) = self
                        .refresh_scheduler_projection_for_invoke_capacity()
                        .await
                    {
                        error!(error = %e, "Failed to refresh scheduler projection after InvokeAgent claim");
                    }
                    let item_id = claimed.work_item_id.clone();
                    let run_id = Some(claimed.run_id);
                    info!(item_id = %item_id, kind = %WorkItemKind::InvokeAgent, "Processing claimed InvokeAgent work item");
                    let executor = Arc::clone(self);
                    tokio::spawn(async move {
                        match executor.process_item(claimed.work_item).await {
                            Ok(()) => {
                                if let Err(e) = executor.work_queue.complete(&item_id).await {
                                    error!(item_id = %item_id, error = %e, "Failed to mark work item complete");
                                }
                            }
                            Err(e) => {
                                error!(item_id = %item_id, kind = %WorkItemKind::InvokeAgent, error = %e, "Work item failed");
                                if let Err(e2) =
                                    executor.work_queue.fail(&item_id, &e.to_string()).await
                                {
                                    error!(item_id = %item_id, error = %e2, "Failed to mark work item failed");
                                }
                                executor
                                    .enqueue_advance_after_invoke_failure(&item_id, run_id)
                                    .await;
                            }
                        }
                    });
                    continue;
                }
                Ok(None) => {
                    if let Err(e) = self.refresh_scheduler_projection_if_needed().await {
                        error!(error = %e, "Failed to refresh scheduler projection after InvokeAgent backpressure scan");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error claiming InvokeAgent work item");
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            }

            match self.work_queue.claim_next().await {
                Ok(Some(item)) => {
                    let item_id = item.id.clone();
                    let kind = item.kind.clone();
                    let run_id = item.run_id;
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
                                    executor
                                        .enqueue_advance_after_invoke_failure(&item_id, run_id)
                                        .await;
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

    async fn refresh_scheduler_projection_for_invoke_capacity(&self) -> Result<()> {
        let capacity = scheduler_capacity_config(&self.invoke_agent_capacity);
        self.work_queue
            .refresh_scheduler_projection_with_capacity(&capacity)
            .await
    }

    async fn refresh_scheduler_projection_if_needed(&self) -> Result<()> {
        let has_pending_invoke =
            !work_items::select_pending_invoke_agents_for_start(&self.pool, chrono::Utc::now(), 1)
                .await?
                .is_empty();
        let latest_state = scheduler::latest_health_snapshot(&self.pool)
            .await?
            .map(|snapshot| snapshot.sustained_backpressure_state)
            .unwrap_or_else(|| "clear".to_string());

        if has_pending_invoke || latest_state != "clear" {
            self.refresh_scheduler_projection_for_invoke_capacity()
                .await?;
        }
        Ok(())
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
                let preclaimed_agent_exec_id: Option<domain::ids::AgentExecutionId> = payload
                    .pointer("/p058_claimed/agent_execution_id")
                    .and_then(|value| value.as_str())
                    .map(|raw| {
                        raw.parse()
                            .map_err(|e: uuid::Error| anyhow::anyhow!("{}", e))
                    })
                    .transpose()?;
                let preclaimed_artifact_claim_key: Option<ArtifactSourceGenerationClaimKey> =
                    payload
                        .pointer("/p058_claimed/artifact_claim_key")
                        .cloned()
                        .map(serde_json::from_value)
                        .transpose()?;
                let preclaimed_policy_decision: Option<SessionPolicyDecision> = payload
                    .pointer("/p058_claimed/session_policy_decision")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?;
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
                let declared_outputs: Vec<DeclaredOutput> = match payload
                    .get("declared_outputs")
                    .filter(|value| !value.is_null())
                {
                    Some(value) => serde_json::from_value(value.clone())
                        .map_err(|e| anyhow::anyhow!("parse InvokeAgent declared_outputs: {e}"))?,
                    None => Vec::new(),
                };
                let stage_degraded_output_policy: workflow::plan::DegradedOutputPolicy =
                    serde_json::from_value(payload["stage_degraded_output_policy"].clone())
                        .unwrap_or_default();
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
                let agent_exec_id =
                    preclaimed_agent_exec_id.unwrap_or_else(domain::ids::AgentExecutionId::new);
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
                    if let Some(decision) = preclaimed_policy_decision {
                        Some(decision)
                    } else if session_reuse_scope.is_some() {
                        Some(ensure_policy(&self.pool, policy_input.clone()).await?)
                    } else {
                        None
                    };
                let p058_preclaimed = preclaimed_agent_exec_id.is_some();
                if !p058_preclaimed {
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
                            policy_decision =
                                Some(ensure_policy(&self.pool, policy_input.clone()).await?);
                        }
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

                let mut agent_exec = if p058_preclaimed {
                    agent_executions::find_by_id(&self.pool, agent_exec_id)
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "P058 preclaimed InvokeAgent missing agent_execution row: {}",
                                agent_exec_id
                            )
                        })?
                } else {
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
                        predicted_mcp_runtime_ids_json: Some(
                            predicted_mcp_runtime_ids_json.clone(),
                        ),
                        actual_mcp_extensions_json: None,
                        actual_mcp_runtime_ids_json: None,
                        denied_mcp_extensions_json: Some(denied_mcp_extensions_json.clone()),
                        mcp_blocking_issues_json: Some(mcp_blocking_issues_json.clone()),
                        actual_mcp_observation_json: None,
                        mcp_session_startup_latency_ms: None,
                    };
                    agent_executions::insert(&self.pool, &agent_exec).await?;
                    agent_exec
                };
                let artifact_claim_key = if let Some(key) = preclaimed_artifact_claim_key {
                    key
                } else {
                    let key = domain::artifact_contracts::ArtifactSourceGenerationClaimKey {
                        run_id,
                        stage_execution_id,
                        agent_execution_id: agent_exec_id,
                        source_work_item_id: item.id.clone(),
                    };
                    if let Some(session_generation_id) = agent_exec.session_generation_id.clone() {
                        let claim_now = chrono::Utc::now();
                        db::repos::artifact_contracts::insert_source_generation_claim(
                            &self.pool,
                            domain::artifact_contracts::ArtifactSourceGenerationClaim {
                                key: key.clone(),
                                current_session_generation_id: Some(session_generation_id),
                                claim_state: domain::agent::ArtifactSourceClaimState::Active,
                                superseding_work_item_id: None,
                                superseded_by_agent_execution_id: None,
                                supersession_journal_id: None,
                                superseded_at: None,
                                closed_at: None,
                                created_at: claim_now,
                                updated_at: claim_now,
                            },
                        )
                        .await?;
                    }
                    key
                };

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
                    if agent_exec.session_generation_id.is_some() {
                        db::repos::artifact_contracts::close_source_generation_claim(
                            &self.pool,
                            &artifact_claim_key,
                        )
                        .await?;
                    }
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
                let mut req = acp::ExecutionRequest {
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
                    chainworks_meta_root: run.chainworks_meta_root.clone(),
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

                let mut execution_result = self.acp.execute(req.clone()).await;
                if let (Err(error), Some(decision)) =
                    (execution_result.as_ref(), policy_decision.as_ref())
                {
                    if decision.should_reuse_live_session
                        && is_reused_live_session_transport_error(&error.to_string())
                    {
                        let fallback_at = chrono::Utc::now();
                        sessions::end_generation(
                            &self.pool,
                            &decision.generation.id,
                            domain::session::SessionGenerationStatus::Invalidated,
                            "transport_missing_live_handle",
                            fallback_at,
                        )
                        .await?;
                        sessions::insert_event(
                            &self.pool,
                            &domain::session::SessionEvent {
                                id: uuid::Uuid::new_v4().to_string(),
                                lineage_id: decision.lineage.id.clone(),
                                generation_id: decision.generation.id.clone(),
                                event_type: domain::session::SessionEventType::Invalidated,
                                recorded_at: fallback_at,
                                details_json: Some(
                                    serde_json::json!({ "reason": "transport_missing_live_handle" })
                                        .to_string(),
                                ),
                            },
                        )
                        .await?;
                        let fallback_decision =
                            ensure_policy(&self.pool, policy_input.clone()).await?;
                        let fallback_disposition =
                            serde_json::to_value(&fallback_decision.disposition)
                                .ok()
                                .and_then(|value| value.as_str().map(String::from));
                        agent_executions::update_session_provenance(
                            &self.pool,
                            agent_exec_id,
                            Some(&fallback_decision.lineage.id),
                            Some(&fallback_decision.generation.id),
                            fallback_decision
                                .generation
                                .rehydrated_from_checkpoint_artifact_id
                                .as_deref(),
                            Some(&fallback_decision.generation.invocation_owner_key),
                            fallback_disposition.as_deref(),
                            fallback_decision.session_reset_reason.as_deref(),
                        )
                        .await?;
                        agent_exec.session_lineage_id = Some(fallback_decision.lineage.id.clone());
                        agent_exec.session_generation_id =
                            Some(fallback_decision.generation.id.clone());
                        agent_exec.rehydrated_from_checkpoint_artifact_id = fallback_decision
                            .generation
                            .rehydrated_from_checkpoint_artifact_id
                            .clone();
                        agent_exec.invocation_owner_key =
                            Some(fallback_decision.generation.invocation_owner_key.clone());
                        agent_exec.session_reuse_disposition = fallback_disposition;
                        agent_exec.session_reset_reason =
                            fallback_decision.session_reset_reason.clone();
                        let mut runtime_facts =
                            AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, fallback_at);
                        runtime_facts.session_reuse_reason =
                            Some(session_reuse_reason_for_policy_decision(&fallback_decision));
                        agent_execution_runtime_facts::upsert(&self.pool, &runtime_facts).await?;
                        req.reuse_existing_session = fallback_decision.should_reuse_live_session;
                        req.session_generation_id = Some(fallback_decision.generation.id.clone());
                        req.provider_session_id =
                            fallback_decision.generation.provider_session_id.clone();
                        policy_decision = Some(fallback_decision);
                        execution_result = self.acp.execute(req.clone()).await;
                    }
                }

                let result = match execution_result {
                    Ok(result) => result,
                    Err(error) => {
                        let completed_at = chrono::Utc::now();
                        let mut facts =
                            runtime_facts_for_acp_error(agent_exec_id, &error, completed_at);
                        if facts.failure_kind == Some(AgentFailureKind::ProviderQuota) {
                            match agent_retry_budget_ledger::upsert_quota_failure(
                                &self.pool,
                                run_id,
                                stage_execution_id,
                                agent_exec_id,
                                facts.retry_after,
                            )
                            .await
                            {
                                Ok(row) => {
                                    facts.quota_ledger_id = Some(row.id);
                                }
                                Err(ledger_error) => {
                                    error!(
                                        run_id = %run_id,
                                        stage_id = %stage_id,
                                        agent_id = %agent_id,
                                        error = %ledger_error,
                                        "Failed to persist P058 quota retry-budget ledger row"
                                    );
                                }
                            }
                        }
                        if let Err(facts_error) =
                            self.persist_runtime_facts(agent_exec_id, facts).await
                        {
                            error!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                error = %facts_error,
                                "Failed to persist P058 runtime facts after ACP startup error"
                            );
                        }
                        let _ =
                            self.events
                                .send(domain::events::DomainEvent::RuntimeStatusChanged {
                                    run_id,
                                    stage_id: stage_id.clone(),
                                    agent_id: agent_id.clone(),
                                    provider: provider.clone(),
                                    event_kind: "session_failed".to_string(),
                                });
                        if let Err(update_error) = agent_executions::update_completed(
                            &self.pool,
                            agent_exec_id,
                            AgentStatus::Failed,
                            completed_at,
                        )
                        .await
                        {
                            error!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                error = %update_error,
                                "Failed to mark agent execution failed after ACP startup error"
                            );
                        }
                        if let Err(close_error) =
                            db::repos::artifact_contracts::close_source_generation_claim(
                                &self.pool,
                                &artifact_claim_key,
                            )
                            .await
                        {
                            error!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                error = %close_error,
                                "Failed to close P058 artifact claim after ACP startup error"
                            );
                        }
                        if let Err(projection_error) =
                            projections::rebuild_all_for_run(&self.pool, run_id).await
                        {
                            error!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                error = %projection_error,
                                "Failed to rebuild projections after ACP startup error"
                            );
                        }
                        return Err(error);
                    }
                };

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
                let mut persisted_artifacts = Vec::new();
                let transcript_artifact = self
                    .persist_transcript_artifact_if_present(
                        &run,
                        &stage_id,
                        &agent_id,
                        &provider,
                        model.clone(),
                        agent_exec_id,
                        completed_at,
                        result.transcript_text.as_deref(),
                    )
                    .await?;
                let transcript_exists = transcript_artifact.is_some();
                if let Some(artifact) = transcript_artifact {
                    persisted_artifacts.push(artifact);
                }
                let declared_artifacts = self.prepare_declared_output_artifacts(
                    &declared_outputs,
                    run_id,
                    &stage_id,
                    &agent_id,
                    &provider,
                    model.clone(),
                    completed_at,
                    &mut persisted_paths,
                )?;
                persisted_artifacts.extend(declared_artifacts.clone());

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

                let import_result = self
                    .import_declared_contract_outputs(
                        &declared_outputs,
                        &persisted_artifacts,
                        &declared_artifacts,
                        stage_execution_id,
                        agent_exec_id,
                        &item.id,
                        &artifact_claim_key,
                        agent_exec.session_generation_id.as_deref(),
                        result.status.clone(),
                        observed_failure_kind_for_execution_result(
                            &result.status,
                            result.transcript_text.as_deref(),
                        ),
                        result.close_diagnostic.as_ref(),
                        &stage_degraded_output_policy,
                        completed_at,
                    )
                    .await?;
                let validation_summary = import_result.validation_summary;
                let final_agent_status = import_result.final_agent_status;
                let degraded_outputs_satisfy_stage = import_result.degraded_outputs_satisfy_stage;

                if let Some(summary) = validation_summary.as_ref() {
                    if summary.failure_class.is_some() {
                        let validation_failure_record = build_validation_failure_record(
                            domain::ids::ArtifactId::new(),
                            run_id,
                            stage_id.clone(),
                            stage_execution_id,
                            agent_id.clone(),
                            agent_exec_id,
                            summary.clone(),
                            persisted_artifacts
                                .iter()
                                .any(|artifact| artifact.name.contains("receipt")),
                            transcript_exists,
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
                            run.chainworks_meta_root.as_deref(),
                        );
                    }
                }

                // If this is a multi-task stage (fan-out), don't settle the stage
                // here — let the orchestrator settle after ALL tasks complete.
                // Only settle for single-task stages (backward compat).
                let total_tasks = payload["total_tasks"].as_u64().unwrap_or(1);

                // Settle the stage based on ACP result status.
                let settlement_kind = if degraded_outputs_satisfy_stage {
                    domain::stage::StageSettlementKind::Completed
                } else {
                    match final_agent_status {
                        domain::agent::AgentStatus::Completed => {
                            domain::stage::StageSettlementKind::Completed
                        }
                        domain::agent::AgentStatus::Failed => {
                            domain::stage::StageSettlementKind::Failed
                        }
                        _ => domain::stage::StageSettlementKind::Failed,
                    }
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

                info!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    status = ?final_agent_status,
                    "InvokeAgent completed"
                );
            }

            WorkItemKind::StartupRepair => {
                let recovery = RecoveryService::new_with_capacity(
                    self.pool.clone(),
                    self.work_queue.clone(),
                    self.events.clone(),
                    (*self.invoke_agent_capacity).clone(),
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

    async fn enqueue_advance_after_invoke_failure(&self, item_id: &str, run_id: Option<RunId>) {
        let Some(run_id) = run_id else {
            error!(
                item_id = %item_id,
                "InvokeAgent failed without run_id; cannot enqueue AdvanceRun"
            );
            return;
        };

        if let Err(error) = self
            .work_queue
            .enqueue(
                WorkItemKind::AdvanceRun,
                Some(run_id),
                None,
                serde_json::json!({ "run_id": run_id.to_string() }),
            )
            .await
        {
            error!(
                item_id = %item_id,
                run_id = %run_id,
                error = %error,
                "Failed to enqueue AdvanceRun after InvokeAgent failure"
            );
        }
    }

    async fn persist_runtime_facts(
        &self,
        agent_exec_id: domain::ids::AgentExecutionId,
        mut facts: AgentExecutionRuntimeFacts,
    ) -> Result<()> {
        facts.agent_execution_id = agent_exec_id;
        facts.updated_at = chrono::Utc::now();
        agent_execution_runtime_facts::upsert(&self.pool, &facts).await
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
        let delivery_config = match self.load_delivery_configuration(&run).await {
            Ok(delivery_config) => delivery_config,
            Err(error) => {
                self.fail_release_agent_before_receipt(
                    run_id,
                    &stage_id,
                    stage_execution_id,
                    agent_exec_id,
                    &agent_id,
                    &provider,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
        let worktree_root = match run.worktree_root.clone().ok_or_else(|| {
            anyhow::anyhow!("Release agent requires a provisioned worktree but none is available.")
        }) {
            Ok(worktree_root) => worktree_root,
            Err(error) => {
                self.fail_release_agent_before_receipt(
                    run_id,
                    &stage_id,
                    stage_execution_id,
                    agent_exec_id,
                    &agent_id,
                    &provider,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
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

    async fn fail_release_agent_before_receipt(
        &self,
        run_id: RunId,
        stage_id: &str,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_exec_id: domain::ids::AgentExecutionId,
        agent_id: &str,
        provider: &str,
        error: &Error,
    ) -> Result<()> {
        let completed_at = chrono::Utc::now();
        let _ = self
            .events
            .send(domain::events::DomainEvent::RuntimeStatusChanged {
                run_id,
                stage_id: stage_id.to_string(),
                agent_id: agent_id.to_string(),
                provider: provider.to_string(),
                event_kind: "session_failed".to_string(),
            });
        agent_executions::update_completed(
            &self.pool,
            agent_exec_id,
            AgentStatus::Failed,
            completed_at,
        )
        .await?;
        stages::settle(
            &self.pool,
            stage_execution_id,
            domain::stage::StageSettlementKind::Failed,
            completed_at,
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
            agent_id = %agent_id,
            error = %error,
            "Release agent failed before delivery receipt eligibility"
        );
        Ok(())
    }

    fn prepare_declared_output_artifacts(
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
            if let Some(artifact) = self.prepare_artifact_if_present(
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
            )? {
                persisted_paths.insert(declared.target_path.clone());
                artifacts_out.push(artifact);
            }

            if let (Some(companion_name), Some(companion_path), Some(schema)) = (
                declared.companion_output_name.as_deref(),
                declared.companion_path.as_deref(),
                declared.schema.as_ref(),
            ) {
                if let Some(artifact) = self.prepare_artifact_if_present(
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
                )? {
                    persisted_paths.insert(companion_path.to_string());
                    artifacts_out.push(artifact);
                }
            }
        }
        Ok(artifacts_out)
    }

    async fn import_declared_contract_outputs(
        &self,
        declared_outputs: &[DeclaredOutput],
        persisted_artifacts: &[Artifact],
        declared_artifacts_to_insert: &[Artifact],
        stage_execution_id: domain::ids::StageExecutionId,
        agent_exec_id: domain::ids::AgentExecutionId,
        work_item_id: &str,
        artifact_claim_key: &ArtifactSourceGenerationClaimKey,
        session_generation_id: Option<&str>,
        result_status: AgentStatus,
        observed_failure_kind: Option<AgentFailureKind>,
        close_diagnostic: Option<&acp::AcpCloseDiagnostic>,
        stage_degraded_output_policy: &workflow::plan::DegradedOutputPolicy,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<DeclaredContractImportResult> {
        let validation_summary = if declared_outputs.is_empty() {
            None
        } else {
            Some(validate_task_outputs(&load_declared_output_bytes(
                declared_outputs,
            )?))
        };
        let validation_failed = validation_summary
            .as_ref()
            .and_then(|summary| summary.failure_class.as_ref())
            .is_some();
        let mut runtime_facts = runtime_facts_for_execution_result(
            agent_exec_id,
            result_status.clone(),
            validation_summary.as_ref(),
            observed_failure_kind,
            completed_at,
            close_diagnostic,
        );
        let policy_failure_kind = runtime_facts
            .failure_kind
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| AgentFailureKind::Unknown.to_string());
        let degraded_outputs_satisfy_stage = result_status == AgentStatus::Failed
            && validation_summary.as_ref().is_some_and(|summary| {
                degraded_policy_allows_valid_failed_outputs(
                    stage_degraded_output_policy,
                    summary,
                    &policy_failure_kind,
                )
            });
        let final_agent_status = if result_status == AgentStatus::Completed && !validation_failed {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };
        let activate_valid_outputs =
            final_agent_status == AgentStatus::Completed || degraded_outputs_satisfy_stage;
        let source_session_generation_id = session_generation_id.unwrap_or("");
        let mut prepared_imports = Vec::new();
        for declared in declared_outputs {
            let Some(schema) = declared.schema.as_ref() else {
                continue;
            };
            if !known_contract_id(&schema.contract_id)
                || schema.contract_id == "run_state_projection_v1"
            {
                if schema.contract_id == "run_state_projection_v1" {
                    if let Some(artifact) = persisted_artifacts.iter().find(|artifact| {
                        artifact.contract_id == schema.contract_id
                            && artifact.file_path == declared.target_path
                            && artifact.name == declared_machine_artifact_name(declared)
                    }) {
                        let generation_input = ActiveArtifactGenerationInput {
                            run_id: artifact_claim_key.run_id,
                            artifact_id: artifact.id,
                            contract_id: schema.contract_id.clone(),
                            canonical_path: declared_machine_artifact_name(declared).to_string(),
                            raw_path: declared.target_path.clone(),
                            raw_status: "superseded_advisory".to_string(),
                            generation_id: format!("{}:{}", agent_exec_id, artifact.id),
                            source_agent_execution_id: Some(agent_exec_id.to_string()),
                            source_stage_execution_id: Some(stage_execution_id.to_string()),
                            source_session_generation_id: session_generation_id.map(str::to_string),
                            source_work_item_id: Some(work_item_id.to_string()),
                            supersedes_generation_id: None,
                            output_settlement: AgentOutputSettlement::None,
                            partial: false,
                            warnings: vec![
                                "agent-authored state/run-state.json is advisory only; sqlite projection remains canonical"
                                    .to_string(),
                            ],
                        };
                        prepared_imports.push(PreparedDeclaredContractImport::RunStateAdvisory(
                            generation_input,
                        ));
                    }
                }
                continue;
            }
            let Some(artifact) = persisted_artifacts.iter().find(|artifact| {
                artifact.contract_id == schema.contract_id
                    && artifact.file_path == declared.target_path
                    && artifact.name == declared_machine_artifact_name(declared)
            }) else {
                continue;
            };

            let output_result = validation_summary.as_ref().and_then(|summary| {
                summary
                    .output_results
                    .iter()
                    .find(|result| result.output_name == declared.output_name)
            });
            let output_valid = output_result.is_some_and(|result| {
                result.status == domain::validation::ValidationStatus::Passed
            });
            let raw_status = if output_valid && activate_valid_outputs {
                extract_contract_status_from_file(&schema.contract_id, &declared.target_path)?
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "invalid".to_string()
            };
            let mut warnings = Vec::new();
            if output_valid && !activate_valid_outputs {
                warnings.push(
                    "valid declared output was not promoted because the execution settlement does not allow it"
                        .to_string(),
                );
            }
            if !output_valid {
                warnings.push(
                    output_result
                        .and_then(|result| result.validation_error.clone())
                        .unwrap_or_else(|| "declared output failed validation".to_string()),
                );
            }

            let generation_input = ActiveArtifactGenerationInput {
                run_id: artifact_claim_key.run_id,
                artifact_id: artifact.id,
                contract_id: schema.contract_id.clone(),
                canonical_path: declared_machine_artifact_name(declared).to_string(),
                raw_path: declared.target_path.clone(),
                raw_status,
                generation_id: format!("{}:{}", agent_exec_id, artifact.id),
                source_agent_execution_id: Some(agent_exec_id.to_string()),
                source_stage_execution_id: Some(stage_execution_id.to_string()),
                source_session_generation_id: session_generation_id.map(str::to_string),
                source_work_item_id: Some(work_item_id.to_string()),
                supersedes_generation_id: None,
                output_settlement: runtime_facts.output_settlement.clone(),
                partial: runtime_facts.output_settlement
                    == AgentOutputSettlement::ValidOutputsFromFailedExecution,
                warnings,
            };
            prepared_imports.push(PreparedDeclaredContractImport::ContractGeneration(
                generation_input,
            ));
        }

        let tx_started = Instant::now();
        let mut tx =
            db::pool::begin_immediate_with_retry(&self.pool, "executor.import_declared_outputs")
                .await?;
        for artifact in declared_artifacts_to_insert {
            artifacts::insert_tx(&mut tx, artifact).await?;
        }
        let mut projection_dirty = false;
        for prepared_import in prepared_imports {
            let decision = match prepared_import {
                PreparedDeclaredContractImport::RunStateAdvisory(generation_input) => {
                    artifact_contracts::record_run_state_advisory_tx(&mut tx, generation_input)
                        .await?;
                    SourceGenerationImportDecision::Activated
                }
                PreparedDeclaredContractImport::ContractGeneration(generation_input) => {
                    if session_generation_id.is_some() {
                        artifact_contracts::import_generation_with_claim_cas_tx(
                            &mut tx,
                            artifact_claim_key,
                            source_session_generation_id,
                            generation_input,
                        )
                        .await?
                    } else {
                        artifact_contracts::upsert_generation_and_rebuild_tx(
                            &mut tx,
                            generation_input,
                        )
                        .await?;
                        SourceGenerationImportDecision::Activated
                    }
                }
            };
            projection_dirty = true;
            if decision == SourceGenerationImportDecision::IgnoredLateOutputs {
                runtime_facts.output_settlement = AgentOutputSettlement::IgnoredLateOutputs;
                runtime_facts.late_output_count += 1;
                runtime_facts.ignored_late_output_count += 1;
                runtime_facts.valid_required_outputs = false;
            }
        }
        agent_execution_runtime_facts::upsert_tx(&mut tx, &runtime_facts).await?;
        if session_generation_id.is_some() {
            artifact_contracts::close_source_generation_claim_tx(&mut tx, artifact_claim_key)
                .await?;
        }
        tx.commit().await?;
        db::pool::log_write_transaction("executor.import_declared_outputs", tx_started);
        for artifact in declared_artifacts_to_insert {
            self.persist_implementation_self_assessment_summary_if_applicable(artifact)
                .await?;
            let _ = self
                .events
                .send(domain::events::DomainEvent::ArtifactCreated {
                    run_id: artifact.run_id,
                    artifact_id: artifact.id,
                });
        }
        if projection_dirty {
            artifact_contracts::export_projection_files(&self.pool, artifact_claim_key.run_id)
                .await?;
        }
        Ok(DeclaredContractImportResult {
            validation_summary,
            final_agent_status,
            degraded_outputs_satisfy_stage,
        })
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
            if discovered.source_path.is_some()
                || declared_names.contains(discovered.name.as_str())
                || declared_outputs.iter().any(|declared| {
                    discovered_artifact_matches_declared_output(discovered, declared)
                })
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
            self.persist_implementation_self_assessment_summary_if_applicable(&artifact)
                .await?;
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

    async fn persist_transcript_artifact_if_present(
        &self,
        run: &domain::run::Run,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        model: Option<String>,
        agent_execution_id: domain::ids::AgentExecutionId,
        created_at: chrono::DateTime<chrono::Utc>,
        transcript_text: Option<&str>,
    ) -> Result<Option<domain::artifact::Artifact>> {
        if std::env::var("CHAINWORKS_PERSIST_ACP_TRANSCRIPTS")
            .ok()
            .as_deref()
            != Some("1")
        {
            return Ok(None);
        }
        let Some(transcript_text) = transcript_text.filter(|text| !text.trim().is_empty()) else {
            return Ok(None);
        };
        if transcript_text.contains("\"CHAINWORKS_OUTPUT\"")
            || transcript_text.contains("<<<CHAINWORKS_OUTPUT")
        {
            return Ok(None);
        }

        let artifact_id = domain::ids::ArtifactId::new();
        let path = std::path::Path::new(&run.artifact_root)
            .join("session_transcripts")
            .join(stage_id)
            .join(format!("{agent_id}-{agent_execution_id}.md"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!("create transcript artifact dir {}: {}", parent.display(), e)
            })?;
        }
        std::fs::write(&path, transcript_text)
            .map_err(|e| anyhow::anyhow!("write transcript artifact {}: {}", path.display(), e))?;

        let artifact = domain::artifact::Artifact {
            id: artifact_id,
            run_id: run.id,
            stage_id: stage_id.to_string(),
            agent_id: agent_id.to_string(),
            name: format!("{agent_id}_transcript"),
            contract_id: "acp_transcript_v1".to_string(),
            format: ArtifactFormat::Markdown,
            file_path: path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: Some(path.metadata().map(|meta| meta.len() as i64).unwrap_or(0)),
            provider: provider.to_string(),
            model,
            created_at,
            is_pinned: false,
            report_kind: Some("agent_transcript".to_string()),
            report_version: Some(1),
        };
        artifacts::insert(&self.pool, &artifact).await?;
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
                run_id: run.id,
                artifact_id: artifact.id,
            });
        Ok(Some(artifact))
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
        let Some(artifact) = self.prepare_artifact_if_present(
            path,
            run_id,
            stage_id,
            agent_id,
            name,
            contract_id,
            format,
            provider,
            model,
            report_kind,
            created_at,
        )?
        else {
            return Ok(None);
        };
        artifacts::insert(&self.pool, &artifact).await?;
        self.persist_implementation_self_assessment_summary_if_applicable(&artifact)
            .await?;
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
                run_id,
                artifact_id: artifact.id,
            });
        Ok(Some(artifact))
    }

    fn prepare_artifact_if_present(
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
        self.persist_implementation_self_assessment_summary_if_applicable(&artifact)
            .await?;
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
        self.persist_implementation_self_assessment_summary_if_applicable(&artifact)
            .await?;
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
        self.persist_implementation_self_assessment_summary_if_applicable(&artifact)
            .await?;
        let _ = self
            .events
            .send(domain::events::DomainEvent::ArtifactCreated {
                run_id: run.id,
                artifact_id: artifact.id,
            });
        Ok(path)
    }

    pub async fn persist_implementation_self_assessment_summary_if_applicable(
        &self,
        artifact: &domain::artifact::Artifact,
    ) -> Result<()> {
        if !is_implementation_self_assessment_artifact(artifact) {
            return Ok(());
        }

        let raw = std::fs::read_to_string(&artifact.file_path)
            .map_err(|e| anyhow::anyhow!("read implementation self-assessment artifact: {}", e))?;
        let declared_v2 = artifact.contract_id
            == domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID;
        let legacy_v1 = artifact.contract_id == "implementation_self_assessment_v1"
            || artifact.contract_id == "implementation_self_assessment";
        let context = domain::artifact_contracts::ContractParseContext {
            run_id: artifact.run_id.to_string(),
            run_age: None,
            declared_contract_id: Some(artifact.contract_id.clone()),
            canonical_artifact_path:
                domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.to_string(),
            raw_artifact_path: Some(artifact.file_path.clone()),
            source_generation_id: None,
            artifact_created_at: Some(artifact.created_at),
            v2_generation_seen_for_run: declared_v2,
            legacy_v1_generation_available: legacy_v1,
        };
        let summary = match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(json) => {
                domain::artifact_contracts::parse_implementation_self_assessment_v2(&json, context)
            }
            Err(error) => {
                domain::artifact_contracts::invalid_implementation_self_assessment_summary(
                    context,
                    "malformed_json",
                    format!("implementation self-assessment artifact is not valid JSON: {error}"),
                    "",
                )
            }
        };
        artifact_contracts::persist_implementation_self_assessment_summary(
            &self.pool,
            artifact.run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await?;
        Ok(())
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
                        run.chainworks_meta_root.as_deref(),
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
    meta_root: Option<&str>,
) {
    let run_dir = format!("{}/{}", artifact_root, run_id);
    // P050: Post-P050 runs search only the run-scoped artifact dir.
    // Legacy runs (meta_root = None) keep the old flat-root fallback.
    let search_dirs = if meta_root.is_some() {
        vec![run_dir]
    } else {
        vec![artifact_root.to_string(), run_dir]
    };

    for (artifact_name, path_template) in artifact_paths {
        let canonical =
            crate::orchestrator::resolve_path_template(path_template, workspace_root, meta_root);

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
    fn proposal_057_degraded_policy_allows_only_valid_declared_contract_outputs() {
        let policy = workflow::plan::DegradedOutputPolicy {
            mode: "allow_valid_contract_outputs".into(),
            contracts: vec!["prepush_review_v1".into()],
            failure_kinds: vec!["provider_quota".into()],
            max_settlement: "valid_outputs_from_failed_execution".into(),
        };
        let validation = TaskValidationSummary {
            output_results: vec![domain::validation::OutputValidationResult {
                output_name: "prepush_review_v1".into(),
                contract_id: Some("prepush_review_v1".into()),
                status: domain::validation::ValidationStatus::Passed,
                missing_fields: vec![],
                validation_error: None,
                raw_payload_size: 32,
            }],
            contract_metadata: vec![],
            raw_output_exists: true,
            failure_class: None,
            failure_summary: None,
        };

        assert!(degraded_policy_allows_valid_failed_outputs(
            &policy,
            &validation,
            "provider_quota"
        ));
        assert!(!degraded_policy_allows_valid_failed_outputs(
            &workflow::plan::DegradedOutputPolicy::default(),
            &validation,
            "provider_quota"
        ));
        assert!(!degraded_policy_allows_valid_failed_outputs(
            &policy,
            &validation,
            "provider_internal_error"
        ));

        let facts = runtime_facts_for_execution_result(
            domain::ids::AgentExecutionId::new(),
            AgentStatus::Failed,
            Some(&validation),
            Some(AgentFailureKind::ProviderQuota),
            chrono::Utc::now(),
            None,
        );
        assert_eq!(facts.failure_kind, Some(AgentFailureKind::ProviderQuota));
        assert!(degraded_policy_allows_valid_failed_outputs(
            &policy,
            &validation,
            &facts.failure_kind.unwrap().to_string()
        ));
    }

    #[test]
    fn proposal_058_close_after_valid_output_records_nonblocking_runtime_fact() {
        let validation = TaskValidationSummary {
            output_results: vec![domain::validation::OutputValidationResult {
                output_name: "prepush_review_v1".into(),
                contract_id: Some("prepush_review_v1".into()),
                status: domain::validation::ValidationStatus::Passed,
                missing_fields: vec![],
                validation_error: None,
                raw_payload_size: 32,
            }],
            contract_metadata: vec![],
            raw_output_exists: true,
            failure_class: None,
            failure_summary: None,
        };
        let close_diagnostic = acp::AcpCloseDiagnostic {
            transport_error_code: Some("EPIPE".into()),
            provider_exit_status: Some(141),
            message: "write EPIPE after session/close".into(),
        };

        let facts = runtime_facts_for_execution_result(
            domain::ids::AgentExecutionId::new(),
            AgentStatus::Completed,
            Some(&validation),
            None,
            chrono::Utc::now(),
            Some(&close_diagnostic),
        );

        assert_eq!(
            facts.output_settlement,
            AgentOutputSettlement::ValidOutputsFromCompletedExecution
        );
        assert_eq!(facts.transport_error_code.as_deref(), Some("EPIPE"));
        assert_eq!(facts.provider_exit_status, Some(141));
        assert_eq!(
            facts.supervision_classification.as_deref(),
            Some("nonblocking_close_after_valid_output")
        );
        assert_eq!(
            facts.failure_message_redacted.as_deref(),
            Some("write EPIPE after session/close")
        );
        assert_eq!(facts.failure_kind, None);
    }

    #[test]
    fn proposal_058_runtime_message_redaction_covers_bearer_and_secret_shapes() {
        let message = "Authorization: Bearer sk-live-secret token=abc123 api_key:sk-api-secret secret plain-secret path=/Users/me/.ssh/id_rsa";
        let redacted = redact_runtime_message(message);

        assert!(redacted.contains("Authorization: Bearer [redacted]"));
        assert!(redacted.contains("token=[redacted]"));
        assert!(redacted.contains("api_key:[redacted]"));
        assert!(!redacted.contains("sk-live-secret"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("sk-api-secret"));
        assert!(!redacted.contains("plain-secret"));
        assert!(!redacted.contains("id_rsa"));
    }

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

    #[test]
    fn materialize_declared_outputs_accepts_target_path_keys_from_json_envelope() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("implementation/self-assessment.json");
        let declared = DeclaredOutput {
            output_name: "implementation_self_assessment".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            companion_output_name: None,
            companion_path: None,
        };
        let discovered = vec![acp::DiscoveredArtifact {
            name: machine_path.to_string_lossy().into_owned(),
            content: br#"{"seemingly_complete":true}"#.to_vec(),
            source_path: None,
        }];

        materialize_declared_outputs_from_discovered_artifacts(&[declared], &discovered)
            .expect("path-keyed JSON envelope outputs should materialize to canonical paths");

        assert_eq!(
            std::fs::read_to_string(machine_path).unwrap(),
            r#"{"seemingly_complete":true}"#
        );
    }
}
