use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

use crate::release::{
    connect::ConnectPublishService,
    coordinator::ReleaseResult,
    git::{GitPushReceipt, GitReleaseService, ReleaseManifest},
    receipt::DeliveryReceiptBuilder,
};
use acp::AcpRuntimeManager;
use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts, agent_executions,
    agent_retry_budget_ledger, artifact_contracts, artifacts, ideas, legacy_discovery_overrides,
    projections, scheduler, sessions, stages, validation, work_items, workflow_conflicts,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{
    AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, AgentStatus,
    OperatorActionHint,
};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{
    contract_status_allowed_values, known_contract_id, parse_implementation_self_assessment_v2,
    ActiveArtifactGenerationInput, ArtifactSourceGenerationClaimKey, ContractParseContext,
    SourceGenerationImportDecision, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
};
use domain::discovery::{
    AgentExecutionDiscoveryDiagnostics, DiscoveryDiagnosticsV1, DiscoveryFilesystem,
    DiscoveryPathKind, ExpectedOutputSpec, ExpectedPathBaselineStatus, LegacyBroadDiscoveryPolicy,
    NoopDiscoveryOperationRecorder, OutputDiscoveryDecision, OutputDiscoveryProvenance,
    OutputDiscoveryReason, OutputDiscoveryStatus, OutputReusePolicy, OutputRootClass,
    PrePromptExpectedOutputMetadata, SourceGenerationOwner, StdDiscoveryFilesystem,
    DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION,
};
use domain::ids::RunId;
use domain::provider::ProviderFamily;
use domain::run::DeliveryConfiguration;
use workflow::catalog::{AgentCatalogFile, AgentEntry};

use crate::contracts::{
    artifact_format_for_companion_output, artifact_format_for_machine_output,
    build_captured_outputs_from_discovery_decisions, build_expected_output_specs,
    build_validation_failure_record, validate_task_outputs, CapturedOutput, DeclaredOutput,
    TaskValidationSummary,
};
use crate::event_bus::EventSender;
use crate::failure_classifier::{
    classify_observation, observation_from_acp_error_message, RuntimeFailureClassification,
};
use crate::git_manifest::generate_changed_files_manifest_if_declared;
use crate::housekeeping::{GeneratedStateHousekeeper, GeneratedStateHousekeepingConfig};
use crate::orchestrator::Orchestrator;
use crate::recovery::RecoveryService;
use crate::session::fingerprint::{
    binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
};
use crate::session::policy::{ensure_policy, SessionPolicyDecision, SessionPolicyInput};
use crate::work_queue::WorkQueue;

const ACTIVE_PROMPT_CLOSE_AUTO_RECOVERY_MAX_ATTEMPTS: i64 = 3;

#[derive(Debug)]
struct WorkItemRequeued {
    work_item_id: String,
    reason: &'static str,
}

impl fmt::Display for WorkItemRequeued {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "work item {} requeued for {}",
            self.work_item_id, self.reason
        )
    }
}

impl std::error::Error for WorkItemRequeued {}

struct DbXcodeRuntimeObservationSink {
    pool: SqlitePool,
    events: EventSender,
}

#[async_trait::async_trait]
impl acp::XcodeRuntimeObservationSink for DbXcodeRuntimeObservationSink {
    async fn append_xcode_runtime_observation(
        &self,
        agent_execution_id: domain::ids::AgentExecutionId,
        update: domain::xcode_runtime::XcodeRuntimeObservationUpdate,
    ) -> Result<()> {
        agent_executions::append_xcode_runtime_observation(&self.pool, agent_execution_id, update)
            .await?;
        self.publish_xcode_runtime_observation_append(agent_execution_id)
            .await;
        Ok(())
    }
}

impl DbXcodeRuntimeObservationSink {
    async fn publish_xcode_runtime_observation_append(
        &self,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) {
        let execution = match agent_executions::find_by_id(&self.pool, agent_execution_id).await {
            Ok(Some(execution)) => execution,
            Ok(None) => {
                warn!(
                    agent_execution_id = %agent_execution_id,
                    "Xcode runtime observation append notification skipped: agent execution missing"
                );
                return;
            }
            Err(error) => {
                warn!(
                    agent_execution_id = %agent_execution_id,
                    error = %error,
                    "Xcode runtime observation append notification skipped: agent execution lookup failed"
                );
                return;
            }
        };
        let Some(stage_execution_id) = execution.stage_execution_id else {
            return;
        };
        let stage = match stages::find_by_id(&self.pool, stage_execution_id).await {
            Ok(Some(stage)) => stage,
            Ok(None) => {
                warn!(
                    agent_execution_id = %agent_execution_id,
                    stage_execution_id = %stage_execution_id,
                    "Xcode runtime observation append notification skipped: stage missing"
                );
                return;
            }
            Err(error) => {
                warn!(
                    agent_execution_id = %agent_execution_id,
                    stage_execution_id = %stage_execution_id,
                    error = %error,
                    "Xcode runtime observation append notification skipped: stage lookup failed"
                );
                return;
            }
        };
        let _ = self
            .events
            .send(domain::events::DomainEvent::StageStatusChanged {
                run_id: stage.run_id,
                stage_execution_id,
                status: stage.status,
            });
    }
}

struct DbAcpPromptProgressSink {
    pool: SqlitePool,
    events: EventSender,
}

#[async_trait::async_trait]
impl acp::AcpPromptProgressSink for DbAcpPromptProgressSink {
    async fn record_acp_prompt_progress(&self, update: acp::AcpPromptProgressUpdate) -> Result<()> {
        let Some(generation_id) = update.session_generation_id.as_deref() else {
            return Ok(());
        };
        sessions::touch_generation_activity(&self.pool, generation_id, chrono::Utc::now()).await?;
        if let Some(stage_execution_id) = update.stage_execution_id.as_deref() {
            match stage_execution_id.parse::<domain::ids::StageExecutionId>() {
                Ok(stage_execution_id) => {
                    if let Ok(Some(stage)) =
                        stages::find_by_id(&self.pool, stage_execution_id).await
                    {
                        let _ = self
                            .events
                            .send(domain::events::DomainEvent::StageStatusChanged {
                                run_id: stage.run_id,
                                stage_execution_id,
                                status: stage.status,
                            });
                    }
                }
                Err(error) => {
                    warn!(
                        stage_execution_id = %stage_execution_id,
                        error = %error,
                        "ACP prompt progress notification skipped: invalid stage execution id"
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimedInvokeAgentStart {
    pub work_item_id: String,
    pub source_work_item_id: String,
    pub run_id: domain::ids::RunId,
    pub stage_execution_id: domain::ids::StageExecutionId,
    pub agent_execution_id: domain::ids::AgentExecutionId,
    pub session_generation_id: Option<String>,
    pub artifact_claim_key: domain::artifact_contracts::ArtifactSourceGenerationClaimKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokeAgentCapacityConfig {
    pub max_active_total: usize,
    pub max_active_per_run: usize,
    pub provider_caps: std::collections::HashMap<String, usize>,
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

impl InvokeAgentCapacityConfig {
    fn unbounded() -> Self {
        Self {
            max_active_total: usize::MAX,
            max_active_per_run: usize::MAX,
            provider_caps: std::collections::HashMap::new(),
        }
    }
}

pub async fn claim_next_invoke_agent_with_start(
    pool: &SqlitePool,
) -> Result<Option<ClaimedInvokeAgentStart>> {
    Ok(
        claim_next_invoke_agent_with_start_internal(pool, &InvokeAgentCapacityConfig::unbounded())
            .await?
            .map(|(claimed, _)| claimed),
    )
}

pub async fn claim_next_invoke_agent_with_start_with_capacity(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<ClaimedInvokeAgentStart>> {
    Ok(claim_next_invoke_agent_with_start_internal(pool, capacity)
        .await?
        .map(|(claimed, _)| claimed))
}

pub async fn has_capacity_eligible_pending_invoke_agent_for_start(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<bool> {
    let candidates =
        work_items::select_pending_invoke_agents_for_start(pool, chrono::Utc::now(), 128).await?;
    for item in candidates {
        if invoke_item_has_start_capacity(pool, &item, capacity).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn claim_next_invoke_agent_with_start_internal(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<(ClaimedInvokeAgentStart, WorkItem)>> {
    let candidates =
        work_items::select_pending_invoke_agents_for_start(pool, chrono::Utc::now(), 128).await?;
    let mut ranked_candidates = Vec::with_capacity(candidates.len());
    for item in candidates {
        let last_served_at = if let Some(run_id) = item.run_id {
            scheduler::get_service_state(pool, "run", &run_id.to_string())
                .await?
                .and_then(|state| state.last_served_at)
        } else {
            None
        };
        ranked_candidates.push((last_served_at, item));
    }
    ranked_candidates.sort_by(|(left_last_served, left), (right_last_served, right)| {
        match (left_last_served, right_last_served) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(left_at), Some(right_at)) if left_at != right_at => left_at.cmp(right_at),
            _ => left
                .scheduled_at
                .cmp(&right.scheduled_at)
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id)),
        }
    });

    for (_, item) in ranked_candidates {
        if !invoke_item_has_start_capacity(pool, &item, capacity).await? {
            continue;
        }
        if let Some(claimed) = claim_invoke_agent_work_item_with_start(pool, item).await? {
            if let Some(run_id) = claimed.1.run_id {
                let now = chrono::Utc::now();
                scheduler::upsert_service_state(
                    pool,
                    &scheduler::SchedulerServiceState {
                        scope: "run".into(),
                        scope_id: run_id.to_string(),
                        last_served_at: Some(now),
                        last_claimed_work_item_id: Some(claimed.0.work_item_id.clone()),
                        updated_at: now,
                    },
                )
                .await?;
            }
            scheduler::refresh_queue_summaries(
                pool,
                &scheduler_capacity_config_from_start_capacity(capacity),
            )
            .await?;
            return Ok(Some(claimed));
        }
    }
    Ok(None)
}

async fn invoke_item_has_start_capacity(
    pool: &SqlitePool,
    item: &WorkItem,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<bool> {
    let payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;
    let provider = payload
        .get("provider")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if provider.is_empty() {
        return Ok(false);
    }
    let provider_family =
        ProviderFamily::canonicalize_known_alias(provider).unwrap_or_else(|| provider.to_string());

    let running_status = AgentStatus::Running.to_string();
    let total_active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = ?1")
            .bind(&running_status)
            .fetch_one(pool)
            .await?;
    if total_active as usize >= capacity.max_active_total {
        return Ok(false);
    }

    if let Some(run_id) = item.run_id {
        let run_active: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)
               FROM agent_executions ae
               INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = ?1 AND se.run_id = ?2"#,
        )
        .bind(&running_status)
        .bind(run_id.to_string())
        .fetch_one(pool)
        .await?;
        if run_active as usize >= capacity.max_active_per_run {
            return Ok(false);
        }
    }

    if let Some(provider_cap) = capacity.provider_caps.get(&provider_family) {
        let provider_active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_executions WHERE status = ?1 AND COALESCE(provider_family, provider) = ?2",
        )
        .bind(&running_status)
        .bind(&provider_family)
        .fetch_one(pool)
        .await?;
        if provider_active as usize >= *provider_cap {
            return Ok(false);
        }
    }

    Ok(true)
}

fn scheduler_capacity_config_from_start_capacity(
    capacity: &InvokeAgentCapacityConfig,
) -> domain::provider::InvokeAgentCapacityConfig {
    let provider_caps = capacity
        .provider_caps
        .iter()
        .filter_map(|(provider, cap)| {
            ProviderFamily::resolve(provider)
                .ok()
                .map(|family| (family, *cap))
        })
        .collect();
    domain::provider::InvokeAgentCapacityConfig {
        global_active_agent_executions: capacity.max_active_total,
        per_run_active_agent_executions: capacity.max_active_per_run,
        provider_caps,
    }
}

async fn claim_invoke_agent_work_item_with_start(
    pool: &SqlitePool,
    item: WorkItem,
) -> Result<Option<(ClaimedInvokeAgentStart, WorkItem)>> {
    let mut payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;
    if !payload
        .as_object()
        .is_some_and(|object| object.contains_key("session_reuse_scope"))
    {
        work_items::fail(
            pool,
            &item.id,
            "InvokeAgent payload missing session_reuse_scope; refusing legacy sessionless ACP fallback",
        )
        .await?;
        return Ok(None);
    }

    let session_reuse_scope = payload
        .get("session_reuse_scope")
        .and_then(|value| value.as_str())
        .map(String::from);
    let session_family_id = payload
        .get("session_family_id")
        .and_then(|value| value.as_str())
        .map(String::from);

    if let Some(existing) = payload.get("p058_claimed") {
        let mut claimed = claimed_invoke_agent_start_from_payload(&item, existing)?;
        let mut running_item = item;
        running_item.status = WorkItemStatus::Running;
        running_item.attempt_count += 1;
        if session_reuse_scope.is_none() && claimed.session_generation_id.is_some() {
            claimed.session_generation_id = None;
            if let Some(claimed_object) = payload
                .get_mut("p058_claimed")
                .and_then(|value| value.as_object_mut())
            {
                claimed_object.remove("session_generation_id");
                claimed_object.remove("session_policy_decision");
            }
            let payload_json = serde_json::to_string(&payload)?;
            update_invoke_work_item_claimed_payload_and_running(
                pool,
                &running_item.id,
                &payload_json,
            )
            .await?;
            agent_executions::update_session_policy(
                pool,
                claimed.agent_execution_id,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            artifact_contracts::update_source_generation_claim_session(
                pool,
                &claimed.artifact_claim_key,
                None,
            )
            .await?;
            running_item.payload_json = payload_json;
        } else {
            mark_invoke_work_item_running(pool, &running_item.id).await?;
        }
        return Ok(Some((claimed, running_item)));
    }

    let run_id = item
        .run_id
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent work item missing run_id"))?;
    let stage_execution_id: domain::ids::StageExecutionId = payload
        .get("stage_execution_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing stage_execution_id"))?
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let stage_id = payload
        .get("stage_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing stage_id"))?
        .to_string();
    let agent_id = payload
        .get("agent_id")
        .and_then(|value| value.as_str())
        .unwrap_or(&stage_id)
        .to_string();
    let provider = payload
        .get("provider")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("InvokeAgent payload missing provider"))?
        .to_string();
    let model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .map(String::from);
    let task_name = payload
        .get("task_name")
        .and_then(|value| value.as_str())
        .unwrap_or(&stage_id)
        .to_string();
    let now = chrono::Utc::now();
    let agent_execution_id = domain::ids::AgentExecutionId::new();
    let session_generation_id = session_reuse_scope
        .as_ref()
        .map(|_| uuid::Uuid::new_v4().to_string());
    let owner_execution_lineage_id = stage_execution_id.to_string();
    let run_id_str = run_id.to_string();
    let invocation_owner_key = invocation_owner_key(&InvocationOwnerKeyInput {
        run_id: &run_id_str,
        agent_id: &agent_id,
        stage_lineage_id: &stage_id,
        task_name: &task_name,
        owner_execution_lineage_id: &owner_execution_lineage_id,
    });

    agent_executions::insert(
        pool,
        &domain::agent::AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(stage_execution_id),
            agent_id,
            provider,
            model,
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: Some(owner_execution_lineage_id),
            session_lineage_id: session_generation_id.clone(),
            session_generation_id: session_generation_id.clone(),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some(invocation_owner_key),
            session_reuse_scope,
            session_family_id,
            session_reuse_disposition: session_generation_id.as_ref().map(|_| "fresh".into()),
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
            owner_kind: Some("stage_execution".to_string()),
            owner_id: Some(stage_execution_id.to_string()),
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
    .await?;

    let mut facts =
        domain::agent::AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.session_reuse_reason = Some("legacy_unknown".into());
    agent_execution_runtime_facts::upsert(pool, &facts).await?;

    let artifact_claim_key = domain::artifact_contracts::ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: domain::mediation::OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: item.id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        pool,
        domain::artifact_contracts::ArtifactSourceGenerationClaim {
            key: artifact_claim_key.clone(),
            current_session_generation_id: session_generation_id.clone(),
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
    artifact_contracts::finalize_pending_retry_supersession_for_work_item(
        pool,
        &item.id,
        agent_execution_id,
    )
    .await?;

    let claimed = ClaimedInvokeAgentStart {
        work_item_id: item.id.clone(),
        source_work_item_id: item.id.clone(),
        run_id,
        stage_execution_id,
        agent_execution_id,
        session_generation_id,
        artifact_claim_key,
    };
    let mut claimed_payload = serde_json::json!({
        "agent_execution_id": claimed.agent_execution_id.to_string(),
        "artifact_claim_key": claimed.artifact_claim_key,
    });
    if let Some(session_generation_id) = claimed.session_generation_id.as_deref() {
        claimed_payload["session_generation_id"] = serde_json::json!(session_generation_id);
        claimed_payload["session_policy_decision"] = serde_json::json!({
            "generation": {
                "id": session_generation_id
            }
        });
    }
    payload["p058_claimed"] = claimed_payload;
    let payload_json = serde_json::to_string(&payload)?;
    update_invoke_work_item_claimed_payload_and_running(pool, &item.id, &payload_json).await?;
    let mut running_item = item;
    running_item.status = WorkItemStatus::Running;
    running_item.payload_json = payload_json;
    running_item.attempt_count += 1;
    Ok(Some((claimed, running_item)))
}

fn claimed_invoke_agent_start_from_payload(
    item: &WorkItem,
    claimed: &serde_json::Value,
) -> Result<ClaimedInvokeAgentStart> {
    let artifact_claim_key: domain::artifact_contracts::ArtifactSourceGenerationClaimKey =
        serde_json::from_value(
            claimed
                .get("artifact_claim_key")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("p058_claimed missing artifact_claim_key"))?,
        )?;
    let agent_execution_id: domain::ids::AgentExecutionId = claimed
        .get("agent_execution_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("p058_claimed missing agent_execution_id"))?
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let session_generation_id = claimed
        .pointer("/session_policy_decision/generation/id")
        .or_else(|| claimed.get("session_generation_id"))
        .and_then(|value| value.as_str())
        .map(String::from);
    Ok(ClaimedInvokeAgentStart {
        work_item_id: item.id.clone(),
        source_work_item_id: artifact_claim_key.source_work_item_id.clone(),
        run_id: artifact_claim_key.run_id,
        stage_execution_id: artifact_claim_key.stage_execution_id.ok_or_else(|| {
            anyhow::anyhow!("preclaimed InvokeAgent start requires stage-owned artifact claim")
        })?,
        agent_execution_id,
        session_generation_id,
        artifact_claim_key,
    })
}

async fn mark_invoke_work_item_running(pool: &SqlitePool, work_item_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let updated = sqlx::query(
        "UPDATE work_items SET status = ?1, started_at = ?2, failed_at = NULL, last_error = NULL, attempt_count = attempt_count + 1 WHERE id = ?3 AND status = ?4",
    )
    .bind(WorkItemStatus::Running.to_string())
    .bind(now)
    .bind(work_item_id)
    .bind(WorkItemStatus::Pending.to_string())
    .execute(pool)
    .await?
    .rows_affected();
    if updated != 1 {
        anyhow::bail!("claim/start CAS failed for InvokeAgent work item {work_item_id}");
    }
    Ok(())
}

async fn update_invoke_work_item_claimed_payload_and_running(
    pool: &SqlitePool,
    work_item_id: &str,
    payload_json: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let updated = sqlx::query(
        "UPDATE work_items SET payload_json = ?1, status = ?2, started_at = ?3, failed_at = NULL, last_error = NULL, attempt_count = attempt_count + 1 WHERE id = ?4 AND status = ?5",
    )
    .bind(payload_json)
    .bind(WorkItemStatus::Running.to_string())
    .bind(now)
    .bind(work_item_id)
    .bind(WorkItemStatus::Pending.to_string())
    .execute(pool)
    .await?
    .rows_affected();
    if updated != 1 {
        anyhow::bail!("claim/start CAS failed for InvokeAgent work item {work_item_id}");
    }
    Ok(())
}

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
                agent_execution_id: None,
                run_id: RunId::new(),
                stage_execution_id: None,
                stage_id: format!("steward_{}", agent.id),
                attempt_number: 1,
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
                expected_outputs: Vec::new(),
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
                legacy_broad_discovery_policy:
                    domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
                xcode_shim_injection_signal: false,
                requires_xcode_host_execution: false,
                owner_kind: "stage_execution".to_string(),
                owner_id: None,
                origin_stage_id: None,
                origin_stage_execution_id: None,
                mediation_record_id: None,
                toolchain_home: None,
                toolchain_go_scope_enabled: false,
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

struct DeclaredOutputDiscoverySettlement {
    decisions: Vec<OutputDiscoveryDecision>,
    accepted_payloads: HashMap<String, Vec<u8>>,
    idempotency_key: Option<String>,
    accepted_aggregate_bytes: u64,
    aggregate_cap_hit: bool,
}

fn build_declared_output_discovery_settlement(
    expected_outputs: &[ExpectedOutputSpec],
    discovered_artifacts: &[acp::DiscoveredArtifact],
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
) -> DeclaredOutputDiscoverySettlement {
    let filesystem = StdDiscoveryFilesystem;
    build_declared_output_discovery_settlement_with_filesystem(
        expected_outputs,
        discovered_artifacts,
        pre_prompt_expected_outputs,
        &filesystem,
    )
}

fn build_declared_output_discovery_settlement_with_filesystem(
    expected_outputs: &[ExpectedOutputSpec],
    discovered_artifacts: &[acp::DiscoveredArtifact],
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
    filesystem: &dyn DiscoveryFilesystem,
) -> DeclaredOutputDiscoverySettlement {
    let mut decisions = Vec::with_capacity(expected_outputs.len());
    let mut accepted_payloads = HashMap::new();
    let mut accepted_aggregate_bytes = 0u64;
    let mut aggregate_cap_hit = false;

    for spec in expected_outputs {
        let envelope = find_provider_envelope_for_spec(discovered_artifacts, spec);
        let exact_path = find_exact_path_artifact_for_spec(discovered_artifacts, spec);
        let candidate = if let Some(artifact) = envelope {
            Some((
                OutputDiscoveryReason::ProviderEnvelope,
                OutputDiscoveryProvenance::ProviderEnvelope,
                Some(artifact.source_kind),
                provider_envelope_payload_ref(&spec.output_name),
                artifact.content.clone(),
                None,
                None,
            ))
        } else if spec.source_generation_owner == SourceGenerationOwner::ControlPlane {
            read_control_plane_generated_output(spec, filesystem)
        } else if let Some(artifact) = exact_path {
            Some((
                OutputDiscoveryReason::ExactPathChanged,
                OutputDiscoveryProvenance::ExactPath,
                Some(artifact.source_kind),
                exact_path_payload_ref(&spec.output_name),
                artifact.content.clone(),
                artifact.source_path.clone(),
                Some(ExpectedPathBaselineStatus::RegularContentCaptured),
            ))
        } else if spec.reuse_policy == OutputReusePolicy::AllowUnchangedExisting {
            read_declared_reuse_policy_output(spec, pre_prompt_expected_outputs, filesystem)
        } else {
            None
        };

        let Some((
            reason,
            provenance,
            source_kind,
            payload_ref,
            bytes,
            source_path,
            baseline_status,
        )) = candidate
        else {
            decisions.push(
                stale_expected_output_decision(spec, pre_prompt_expected_outputs, filesystem)
                    .unwrap_or_else(|| missing_output_decision(spec)),
            );
            continue;
        };

        if matches!(
            provenance,
            OutputDiscoveryProvenance::ExactPath | OutputDiscoveryProvenance::DeclaredReusePolicy
        ) {
            if let Some((reason, baseline_status)) =
                exact_path_rejection_for_spec(spec, source_path.as_deref(), filesystem)
            {
                decisions.push(rejected_output_decision(
                    spec,
                    reason,
                    Some(provenance),
                    source_path,
                    Some(baseline_status),
                    Some(bytes.len() as u64),
                    Some(spec.max_bytes),
                    None,
                    filesystem,
                ));
                continue;
            }
        }

        if bytes.len() as u64 > spec.max_bytes {
            decisions.push(rejected_output_decision(
                spec,
                oversized_reason_for_provenance(provenance, source_kind),
                Some(provenance),
                source_path,
                baseline_status,
                Some(bytes.len() as u64),
                Some(spec.max_bytes),
                None,
                filesystem,
            ));
            continue;
        }

        let aggregate_after = accepted_aggregate_bytes.saturating_add(bytes.len() as u64);
        if aggregate_after > spec.aggregate_acceptance_cap_bytes {
            aggregate_cap_hit = true;
            decisions.push(rejected_output_decision(
                spec,
                OutputDiscoveryReason::AggregateExactOutputCap,
                Some(provenance),
                source_path,
                baseline_status,
                Some(bytes.len() as u64),
                Some(spec.max_bytes),
                Some(accepted_aggregate_bytes),
                filesystem,
            ));
            continue;
        }

        accepted_aggregate_bytes = aggregate_after;
        let digest = sha256_digest(&bytes);
        accepted_payloads.insert(payload_ref.clone(), bytes.clone());
        decisions.push(OutputDiscoveryDecision {
            output_name: spec.output_name.clone(),
            output_role: spec.output_role,
            target_path: spec.target_path.clone(),
            companion_of: spec.companion_of.clone(),
            status: OutputDiscoveryStatus::Accepted,
            reason,
            provenance: Some(provenance),
            canonical_path: canonical_path_for_decision(
                source_path.as_deref(),
                &spec.target_path,
                filesystem,
            ),
            root_class: spec.authorized_roots.first().map(|root| root.root_class),
            baseline_status,
            size_bytes: Some(bytes.len() as u64),
            content_digest: Some(digest.clone()),
            max_bytes_applied: Some(spec.max_bytes),
            aggregate_bytes_after_acceptance: Some(aggregate_after),
            accepted_payload_ref: Some(payload_ref),
            accepted_bytes_sha256: Some(digest),
            generated_by: (spec.source_generation_owner == SourceGenerationOwner::ControlPlane)
                .then_some("control_plane".to_string()),
            diagnostics: Default::default(),
            decision_at: chrono::Utc::now(),
        });
    }

    DeclaredOutputDiscoverySettlement {
        decisions,
        accepted_payloads,
        idempotency_key: discovery_settlement_idempotency_key(pre_prompt_expected_outputs),
        accepted_aggregate_bytes,
        aggregate_cap_hit,
    }
}

fn discovery_settlement_idempotency_key(
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
) -> Option<String> {
    pre_prompt_expected_outputs.first().map(|metadata| {
        format!(
            "{}:{}",
            metadata.agent_execution_id, metadata.discovery_generation_id
        )
    })
}

fn settle_agent_outputs_from_discovery_decisions(
    declared_outputs: &[DeclaredOutput],
    expected_outputs: &[ExpectedOutputSpec],
    discovered_artifacts: &[acp::DiscoveredArtifact],
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
) -> Result<DeclaredOutputDiscoverySettlement> {
    let settlement = build_declared_output_discovery_settlement(
        expected_outputs,
        discovered_artifacts,
        pre_prompt_expected_outputs,
    );
    for declared in declared_outputs {
        if let Some(artifact) = find_accepted_provider_artifact_for_output(
            discovered_artifacts,
            &settlement.decisions,
            &declared.output_name,
            &declared.target_path,
        ) {
            write_discovered_output(&declared.target_path, &artifact.content)?;
        }

        if let (Some(companion_name), Some(companion_path)) = (
            declared.companion_output_name.as_deref(),
            declared.companion_path.as_deref(),
        ) {
            if let Some(artifact) = find_accepted_provider_artifact_for_output(
                discovered_artifacts,
                &settlement.decisions,
                companion_name,
                companion_path,
            ) {
                write_discovered_output(companion_path, &artifact.content)?;
            }
        }
    }

    Ok(settlement)
}

fn exact_path_rejection_for_spec(
    spec: &ExpectedOutputSpec,
    source_path: Option<&str>,
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<(OutputDiscoveryReason, ExpectedPathBaselineStatus)> {
    let source_path = source_path?;
    let source = Path::new(source_path);
    let recorder = NoopDiscoveryOperationRecorder;
    let source_metadata = match filesystem.path_metadata_with_recorder(source, &recorder) {
        Some(metadata) => metadata,
        None => {
            return Some((
                OutputDiscoveryReason::ReadError,
                ExpectedPathBaselineStatus::Unreadable,
            ));
        }
    };
    let source_is_symlink = source_metadata.kind == DiscoveryPathKind::Symlink;
    let canonical_source = match filesystem.canonicalize_path_with_recorder(source, &recorder) {
        Some(path) => path,
        None => {
            return Some((
                OutputDiscoveryReason::ReadError,
                ExpectedPathBaselineStatus::Unreadable,
            ));
        }
    };
    if filesystem
        .path_metadata_with_recorder(&canonical_source, &recorder)
        .is_none_or(|metadata| metadata.kind != DiscoveryPathKind::RegularFile)
    {
        return Some((
            OutputDiscoveryReason::NotRegularFile,
            ExpectedPathBaselineStatus::NotRegularFile,
        ));
    }

    if authorized_root_class_for_canonical_path(spec, &canonical_source, filesystem).is_some() {
        return None;
    }

    if source_is_symlink {
        return Some((
            OutputDiscoveryReason::SymlinkEscape,
            ExpectedPathBaselineStatus::SymlinkEscape,
        ));
    }

    if spec
        .authorized_roots
        .iter()
        .any(|root| root.root_class == OutputRootClass::ChainworksMetaRoot)
        && path_mentions_chainworks_run(&canonical_source)
    {
        return Some((
            OutputDiscoveryReason::WrongRunMetaRoot,
            ExpectedPathBaselineStatus::UnauthorizedRoot,
        ));
    }

    Some((
        OutputDiscoveryReason::UnauthorizedRoot,
        ExpectedPathBaselineStatus::UnauthorizedRoot,
    ))
}

fn authorized_root_class_for_canonical_path(
    spec: &ExpectedOutputSpec,
    canonical_path: &Path,
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<OutputRootClass> {
    let recorder = NoopDiscoveryOperationRecorder;
    spec.authorized_roots.iter().find_map(|root| {
        let root_path = Path::new(&root.root_path);
        let canonical_root = filesystem
            .canonicalize_path_with_recorder(root_path, &recorder)
            .unwrap_or_else(|| {
                if root_path.is_absolute() {
                    root_path.to_path_buf()
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(root_path))
                        .unwrap_or_else(|_| root_path.to_path_buf())
                }
            });
        canonical_path
            .starts_with(&canonical_root)
            .then_some(root.root_class)
    })
}

fn path_mentions_chainworks_run(path: &Path) -> bool {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    components
        .windows(2)
        .any(|window| window == [".chainworks", "runs"])
}

fn declared_output_has_accepted_discovery_decision(
    decisions: Option<&[OutputDiscoveryDecision]>,
    output_name: &str,
    output_role: domain::discovery::ExpectedOutputRole,
) -> bool {
    decisions.map_or(true, |decisions| {
        decisions.iter().any(|decision| {
            decision.output_name == output_name
                && decision.output_role == output_role
                && decision.status == OutputDiscoveryStatus::Accepted
        })
    })
}

fn find_provider_envelope_for_spec<'a>(
    discovered_artifacts: &'a [acp::DiscoveredArtifact],
    spec: &ExpectedOutputSpec,
) -> Option<&'a acp::DiscoveredArtifact> {
    discovered_artifacts.iter().find(|artifact| {
        artifact.source_path.is_none()
            && matches!(
                artifact.source_kind,
                acp::DiscoveredArtifactSourceKind::ProviderEnvelope
                    | acp::DiscoveredArtifactSourceKind::ChainworksOutput
            )
            && (artifact.name == spec.output_name || artifact.name == spec.target_path)
    })
}

fn find_exact_path_artifact_for_spec<'a>(
    discovered_artifacts: &'a [acp::DiscoveredArtifact],
    spec: &ExpectedOutputSpec,
) -> Option<&'a acp::DiscoveredArtifact> {
    discovered_artifacts.iter().find(|artifact| {
        artifact
            .source_path
            .as_deref()
            .is_some_and(|source_path| source_path == spec.target_path)
    })
}

fn read_control_plane_generated_output(
    spec: &ExpectedOutputSpec,
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<(
    OutputDiscoveryReason,
    OutputDiscoveryProvenance,
    Option<acp::DiscoveredArtifactSourceKind>,
    String,
    Vec<u8>,
    Option<String>,
    Option<ExpectedPathBaselineStatus>,
)> {
    let recorder = NoopDiscoveryOperationRecorder;
    let bytes = filesystem.read_file_with_cap_and_recorder(
        Path::new(&spec.target_path),
        spec.max_bytes,
        &recorder,
    )?;
    Some((
        OutputDiscoveryReason::ControlPlaneGenerated,
        OutputDiscoveryProvenance::ControlPlaneGenerated,
        None,
        control_plane_payload_ref(&spec.output_name),
        bytes,
        Some(spec.target_path.clone()),
        Some(ExpectedPathBaselineStatus::RegularContentCaptured),
    ))
}

fn read_declared_reuse_policy_output(
    spec: &ExpectedOutputSpec,
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<(
    OutputDiscoveryReason,
    OutputDiscoveryProvenance,
    Option<acp::DiscoveredArtifactSourceKind>,
    String,
    Vec<u8>,
    Option<String>,
    Option<ExpectedPathBaselineStatus>,
)> {
    let metadata = pre_prompt_expected_outputs.iter().find(|metadata| {
        metadata.output_name == spec.output_name
            && metadata.target_path == spec.target_path
            && metadata.baseline_status == ExpectedPathBaselineStatus::RegularContentCaptured
    })?;
    let pre_prompt_digest = metadata.content_digest.as_deref()?;
    if exact_path_rejection_for_spec(spec, Some(&spec.target_path), filesystem).is_some() {
        return None;
    }
    let recorder = NoopDiscoveryOperationRecorder;
    let bytes = filesystem.read_file_with_cap_and_recorder(
        Path::new(&spec.target_path),
        spec.max_bytes,
        &recorder,
    )?;
    if sha256_digest(&bytes) != pre_prompt_digest {
        return None;
    }
    Some((
        OutputDiscoveryReason::DeclaredReusePolicy,
        OutputDiscoveryProvenance::DeclaredReusePolicy,
        None,
        declared_reuse_policy_payload_ref(&spec.output_name),
        bytes,
        Some(spec.target_path.clone()),
        Some(metadata.baseline_status),
    ))
}

fn find_accepted_provider_artifact_for_output<'a>(
    discovered_artifacts: &'a [acp::DiscoveredArtifact],
    decisions: &[OutputDiscoveryDecision],
    output_name: &str,
    target_path: &str,
) -> Option<&'a acp::DiscoveredArtifact> {
    let decision = decisions.iter().find(|decision| {
        decision.output_name == output_name
            && decision.status == OutputDiscoveryStatus::Accepted
            && decision.provenance == Some(OutputDiscoveryProvenance::ProviderEnvelope)
    })?;
    let payload_ref = decision.accepted_payload_ref.as_deref()?;
    (payload_ref == provider_envelope_payload_ref(output_name)).then(|| {
        find_discovered_artifact_for_output(discovered_artifacts, output_name, target_path)
            .filter(|artifact| artifact.source_path.is_none())
    })?
}

fn missing_output_decision(spec: &ExpectedOutputSpec) -> OutputDiscoveryDecision {
    OutputDiscoveryDecision {
        output_name: spec.output_name.clone(),
        output_role: spec.output_role,
        target_path: spec.target_path.clone(),
        companion_of: spec.companion_of.clone(),
        status: OutputDiscoveryStatus::Missing,
        reason: OutputDiscoveryReason::MissingAfterPrompt,
        provenance: None,
        canonical_path: None,
        root_class: spec.authorized_roots.first().map(|root| root.root_class),
        baseline_status: None,
        size_bytes: None,
        content_digest: None,
        max_bytes_applied: Some(spec.max_bytes),
        aggregate_bytes_after_acceptance: None,
        accepted_payload_ref: None,
        accepted_bytes_sha256: None,
        generated_by: None,
        diagnostics: Default::default(),
        decision_at: chrono::Utc::now(),
    }
}

fn stale_expected_output_decision(
    spec: &ExpectedOutputSpec,
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<OutputDiscoveryDecision> {
    if spec.reuse_policy != OutputReusePolicy::MustProduce {
        return None;
    }
    let metadata = pre_prompt_expected_outputs.iter().find(|metadata| {
        metadata.output_name == spec.output_name
            && metadata.target_path == spec.target_path
            && metadata.baseline_status == ExpectedPathBaselineStatus::RegularContentCaptured
    })?;
    let pre_prompt_digest = metadata.content_digest.as_deref()?;
    if exact_path_rejection_for_spec(spec, Some(&spec.target_path), filesystem).is_some() {
        return None;
    }
    let recorder = NoopDiscoveryOperationRecorder;
    let bytes = filesystem.read_file_with_cap_and_recorder(
        Path::new(&spec.target_path),
        spec.max_bytes,
        &recorder,
    )?;
    let current_digest = sha256_digest(&bytes);
    if current_digest != pre_prompt_digest {
        return None;
    }

    Some(OutputDiscoveryDecision {
        output_name: spec.output_name.clone(),
        output_role: spec.output_role,
        target_path: spec.target_path.clone(),
        companion_of: spec.companion_of.clone(),
        status: OutputDiscoveryStatus::Missing,
        reason: OutputDiscoveryReason::StaleExpectedOutput,
        provenance: Some(OutputDiscoveryProvenance::ExactPath),
        canonical_path: canonical_path_for_decision(
            Some(&spec.target_path),
            &spec.target_path,
            filesystem,
        ),
        root_class: spec.authorized_roots.first().map(|root| root.root_class),
        baseline_status: Some(metadata.baseline_status),
        size_bytes: Some(bytes.len() as u64),
        content_digest: Some(current_digest),
        max_bytes_applied: Some(spec.max_bytes),
        aggregate_bytes_after_acceptance: None,
        accepted_payload_ref: None,
        accepted_bytes_sha256: None,
        generated_by: None,
        diagnostics: Default::default(),
        decision_at: chrono::Utc::now(),
    })
}

fn rejected_output_decision(
    spec: &ExpectedOutputSpec,
    reason: OutputDiscoveryReason,
    provenance: Option<OutputDiscoveryProvenance>,
    source_path: Option<String>,
    baseline_status: Option<ExpectedPathBaselineStatus>,
    size_bytes: Option<u64>,
    max_bytes_applied: Option<u64>,
    aggregate_bytes_after_acceptance: Option<u64>,
    filesystem: &dyn DiscoveryFilesystem,
) -> OutputDiscoveryDecision {
    OutputDiscoveryDecision {
        output_name: spec.output_name.clone(),
        output_role: spec.output_role,
        target_path: spec.target_path.clone(),
        companion_of: spec.companion_of.clone(),
        status: OutputDiscoveryStatus::Rejected,
        reason,
        provenance,
        canonical_path: canonical_path_for_decision(
            source_path.as_deref(),
            &spec.target_path,
            filesystem,
        ),
        root_class: spec.authorized_roots.first().map(|root| root.root_class),
        baseline_status,
        size_bytes,
        content_digest: None,
        max_bytes_applied,
        aggregate_bytes_after_acceptance,
        accepted_payload_ref: None,
        accepted_bytes_sha256: None,
        generated_by: None,
        diagnostics: Default::default(),
        decision_at: chrono::Utc::now(),
    }
}

fn oversized_reason_for_provenance(
    provenance: OutputDiscoveryProvenance,
    source_kind: Option<acp::DiscoveredArtifactSourceKind>,
) -> OutputDiscoveryReason {
    match (provenance, source_kind) {
        (
            OutputDiscoveryProvenance::ProviderEnvelope,
            Some(acp::DiscoveredArtifactSourceKind::ChainworksOutput),
        ) => OutputDiscoveryReason::ChainworksOutputOversized,
        (OutputDiscoveryProvenance::ProviderEnvelope, _) => {
            OutputDiscoveryReason::ProviderEnvelopeOversized
        }
        _ => OutputDiscoveryReason::Oversized,
    }
}

fn canonical_path_for_decision(
    source_path: Option<&str>,
    target_path: &str,
    filesystem: &dyn DiscoveryFilesystem,
) -> Option<String> {
    let path = source_path.unwrap_or(target_path);
    let recorder = NoopDiscoveryOperationRecorder;
    filesystem
        .canonicalize_path_with_recorder(Path::new(path), &recorder)
        .map(|path| path.to_string_lossy().into_owned())
}

fn provider_envelope_payload_ref(output_name: &str) -> String {
    format!("provider_envelope:{output_name}")
}

fn exact_path_payload_ref(output_name: &str) -> String {
    format!("exact_path:{output_name}")
}

fn declared_reuse_policy_payload_ref(output_name: &str) -> String {
    format!("declared_reuse_policy:{output_name}")
}

fn control_plane_payload_ref(output_name: &str) -> String {
    format!("control_plane:{output_name}")
}

fn bounded_meta_root_artifact_paths(
    chainworks_meta_root: Option<&str>,
) -> domain::discovery::BoundedMetaRootDiscovery {
    let filesystem = StdDiscoveryFilesystem;
    bounded_meta_root_artifact_paths_with_filesystem(chainworks_meta_root, &filesystem)
}

fn bounded_meta_root_artifact_paths_with_filesystem(
    chainworks_meta_root: Option<&str>,
    filesystem: &dyn DiscoveryFilesystem,
) -> domain::discovery::BoundedMetaRootDiscovery {
    let Some(meta_root) = chainworks_meta_root.filter(|root| !root.trim().is_empty()) else {
        warn!("P053 bounded meta-root discovery skipped: chainworks_meta_root absent");
        return domain::discovery::BoundedMetaRootDiscovery {
            root_path: String::new(),
            artifact_paths: Vec::new(),
            files_visited: 0,
            total_bytes: 0,
            latency_ms: None,
            truncated_by_file_cap: false,
            truncated_by_file_size: false,
            truncated_by_total_bytes: false,
            warnings: vec!["meta_root_absent".to_string()],
        };
    };
    let meta_root_started = Instant::now();
    let recorder = NoopDiscoveryOperationRecorder;
    let mut discovery = filesystem
        .discover_bounded_meta_root_artifacts_with_recorder(Path::new(meta_root), &recorder);
    let latency_ms = meta_root_started.elapsed().as_millis() as u64;
    discovery.latency_ms = Some(latency_ms);
    info!(
        chainworks_meta_root = %discovery.root_path,
        acp_meta_root_discovery_latency_ms = latency_ms,
        files_visited = discovery.files_visited,
        total_bytes = discovery.total_bytes,
        truncated_by_file_cap = discovery.truncated_by_file_cap,
        truncated_by_file_size = discovery.truncated_by_file_size,
        truncated_by_total_bytes = discovery.truncated_by_total_bytes,
        "P053 bounded meta-root discovery measured"
    );
    if discovery.truncated_by_file_cap
        || discovery.truncated_by_file_size
        || discovery.truncated_by_total_bytes
    {
        warn!(
            chainworks_meta_root = %discovery.root_path,
            files_visited = discovery.files_visited,
            total_bytes = discovery.total_bytes,
            truncated_by_file_cap = discovery.truncated_by_file_cap,
            truncated_by_file_size = discovery.truncated_by_file_size,
            truncated_by_total_bytes = discovery.truncated_by_total_bytes,
            "P053 bounded meta-root discovery hit caps"
        );
    }
    for warning in &discovery.warnings {
        warn!(
            chainworks_meta_root = %discovery.root_path,
            warning = %warning,
            "P053 bounded meta-root discovery warning"
        );
    }
    discovery
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
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

fn suppress_interactive_review_xcode_mcp_for_invocation(
    agent_id: &str,
    backend_profile_id: Option<&str>,
    permission_profile: Option<&str>,
) -> bool {
    let readonly_review_permission =
        matches!(permission_profile, Some("RO_VERIFY" | "RO_PREPUSH_VERIFY"));
    let proposal_authoring = matches!(agent_id, "proposal_writer")
        || matches!(permission_profile, Some("PROPOSAL_WRITE"));
    proposal_authoring
        || (readonly_review_permission
            && (matches!(
                agent_id,
                "proposal_implementation_auditor" | "prepush_code_reviewer"
            ) || matches!(
                backend_profile_id,
                Some("codex_audit_high" | "claude_prepush_medium")
            )))
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
        || lower.contains("session closed during active prompt")
}

fn is_active_prompt_closed_transport_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("session closed during active prompt")
}

fn is_work_item_requeued(error: &Error) -> bool {
    error.downcast_ref::<WorkItemRequeued>().is_some()
}

fn is_transient_persistence_contention_error(error: &Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    message.contains("error returned from database: (code: 5)")
        || message.contains("error returned from database: (code: 6)")
        || message.contains("sqlite_busy")
        || message.contains("sqlite_locked")
        || message.contains("database is locked")
        || message.contains("database is busy")
}

fn runtime_facts_for_execution_result(
    agent_exec_id: domain::ids::AgentExecutionId,
    result_status: AgentStatus,
    validation_summary: Option<&TaskValidationSummary>,
    observed_failure_classification: Option<RuntimeFailureClassification>,
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
    let provider_quota_classification = observed_failure_classification
        .as_ref()
        .filter(|classification| classification.failure_kind == AgentFailureKind::ProviderQuota);
    match validation_summary.and_then(|summary| summary.failure_class.as_ref()) {
        Some(domain::validation::ValidationFailureClass::NoOutputProduced)
            if provider_quota_classification.is_some() =>
        {
            let classification = provider_quota_classification.expect("checked above");
            facts.failure_kind = Some(classification.failure_kind.clone());
            facts.operator_action_hint = Some(classification.operator_action_hint.clone());
            facts.retry_after = classification.retry_after;
            facts.transport_error_code = classification.transport_error_code.clone();
            facts.supervision_classification = classification.supervision_classification.clone();
            facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        }
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
            if let Some(classification) = observed_failure_classification.as_ref() {
                facts.failure_kind = Some(classification.failure_kind.clone());
                facts.operator_action_hint = Some(classification.operator_action_hint.clone());
                facts.retry_after = classification.retry_after;
                facts.transport_error_code = classification.transport_error_code.clone();
                facts.supervision_classification =
                    classification.supervision_classification.clone();
            } else {
                facts.failure_kind = Some(AgentFailureKind::ProviderInternalError);
                facts.operator_action_hint = Some(OperatorActionHint::Retry);
            }
            facts.output_settlement = AgentOutputSettlement::ValidOutputsFromFailedExecution;
        }
        None if result_status == AgentStatus::Failed => {
            if let Some(classification) = observed_failure_classification.as_ref() {
                facts.failure_kind = Some(classification.failure_kind.clone());
                facts.operator_action_hint = Some(classification.operator_action_hint.clone());
                facts.retry_after = classification.retry_after;
                facts.transport_error_code = classification.transport_error_code.clone();
                facts.supervision_classification =
                    classification.supervision_classification.clone();
            } else {
                facts.failure_kind = Some(AgentFailureKind::ProviderInternalError);
                facts.operator_action_hint = Some(OperatorActionHint::Retry);
            }
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

struct ExecutionDiscoveryMetrics {
    acp_pre_initialize_local_latency_ms: Option<u64>,
    acp_initialize_latency_ms: Option<u64>,
    acp_session_new_latency_ms: Option<u64>,
    acp_prompt_duration_ms: Option<u64>,
    acp_pre_prompt_metadata_latency_ms: Option<u64>,
    acp_pre_prompt_metadata_timeout: bool,
    acp_pre_prompt_metadata_digest_bytes: u64,
    acp_expected_output_spec_count: usize,
    acp_control_plane_manifest_latency_ms: Option<u64>,
    acp_git_changed_files_latency_ms: Option<u64>,
    acp_git_manifest_status: Option<String>,
    acp_exact_output_acceptance_latency_ms: Option<u64>,
    acp_exact_output_acceptance_timeout: bool,
    acp_exact_output_aggregate_bytes: u64,
    acp_exact_output_aggregate_cap_hit: bool,
    acp_legacy_broad_discovery_policy: String,
    acp_legacy_broad_discovery_used: bool,
    acp_discovery_override_status: String,
    acp_legacy_broad_discovery_truncation_reason: Option<String>,
    acp_resume_discovery_warning: Option<String>,
    acp_cap_validation_sample_size: Option<u64>,
    acp_cap_validation_p90_output_bytes: Option<u64>,
    acp_cap_validation_p90_aggregate_bytes: Option<u64>,
}

fn discovery_diagnostics_for_execution_result(
    agent_exec_id: domain::ids::AgentExecutionId,
    decisions: &[OutputDiscoveryDecision],
    pre_prompt_expected_outputs: &[PrePromptExpectedOutputMetadata],
    bounded_meta_root_discovery: Option<domain::discovery::BoundedMetaRootDiscovery>,
    metrics: ExecutionDiscoveryMetrics,
    now: chrono::DateTime<chrono::Utc>,
) -> AgentExecutionDiscoveryDiagnostics {
    let acp_meta_root_discovery_latency_ms = bounded_meta_root_discovery
        .as_ref()
        .and_then(|discovery| discovery.latency_ms);
    let payload = DiscoveryDiagnosticsV1 {
        schema_version: DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string(),
        agent_execution_id: agent_exec_id.to_string(),
        decisions: decisions.to_vec(),
        pre_prompt_expected_outputs: pre_prompt_expected_outputs.to_vec(),
        legacy_broad_discovery_used: metrics.acp_legacy_broad_discovery_used,
        bounded_meta_root_discovery,
        git_manifest_status: metrics.acp_git_manifest_status.clone(),
        resume_warnings: metrics
            .acp_resume_discovery_warning
            .iter()
            .cloned()
            .collect(),
        warnings: Vec::new(),
        generated_at: now,
        acp_pre_initialize_local_latency_ms: metrics.acp_pre_initialize_local_latency_ms,
        acp_initialize_latency_ms: metrics.acp_initialize_latency_ms,
        acp_session_new_latency_ms: metrics.acp_session_new_latency_ms,
        acp_prompt_duration_ms: metrics.acp_prompt_duration_ms,
        acp_pre_prompt_metadata_latency_ms: metrics.acp_pre_prompt_metadata_latency_ms,
        acp_pre_prompt_metadata_timeout: Some(metrics.acp_pre_prompt_metadata_timeout),
        acp_pre_prompt_metadata_digest_bytes: Some(metrics.acp_pre_prompt_metadata_digest_bytes),
        acp_expected_output_spec_count: Some(metrics.acp_expected_output_spec_count as u64),
        acp_control_plane_manifest_latency_ms: metrics.acp_control_plane_manifest_latency_ms,
        acp_exact_output_acceptance_latency_ms: metrics.acp_exact_output_acceptance_latency_ms,
        acp_meta_root_discovery_latency_ms,
        acp_git_changed_files_latency_ms: metrics.acp_git_changed_files_latency_ms,
        acp_expected_outputs_found_count: None,
        acp_expected_outputs_missing_count: None,
        acp_expected_outputs_stale_count: None,
        acp_expected_outputs_rejected_count: None,
        acp_meta_discovery_truncated: None,
        acp_meta_discovery_truncation_reason: None,
        acp_legacy_broad_discovery_policy: Some(metrics.acp_legacy_broad_discovery_policy),
        acp_legacy_broad_discovery_used: Some(metrics.acp_legacy_broad_discovery_used),
        acp_git_manifest_status: metrics.acp_git_manifest_status,
        acp_resume_discovery_warning: metrics.acp_resume_discovery_warning,
        acp_discovery_schema_version: Some(DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string()),
        acp_discovery_override_status: Some(metrics.acp_discovery_override_status),
        acp_missing_required_output_count: None,
        acp_rejected_output_count: None,
        acp_stale_output_count: None,
        acp_exact_output_acceptance_timeout: Some(metrics.acp_exact_output_acceptance_timeout),
        acp_exact_output_aggregate_bytes: Some(metrics.acp_exact_output_aggregate_bytes),
        acp_exact_output_aggregate_cap_hit: Some(metrics.acp_exact_output_aggregate_cap_hit),
        acp_cap_validation_sample_size: metrics.acp_cap_validation_sample_size,
        acp_cap_validation_p90_output_bytes: metrics.acp_cap_validation_p90_output_bytes,
        acp_cap_validation_p90_aggregate_bytes: metrics.acp_cap_validation_p90_aggregate_bytes,
        acp_legacy_broad_discovery_timeout_ms: Some(5_000),
        acp_legacy_broad_discovery_truncation_reason: metrics
            .acp_legacy_broad_discovery_truncation_reason,
        acp_reconciliation_pending: Some(false),
    };
    AgentExecutionDiscoveryDiagnostics::from_payload(payload, now)
}

fn legacy_broad_discovery_policy_name(
    policy: domain::discovery::LegacyBroadDiscoveryPolicy,
) -> String {
    match policy {
        domain::discovery::LegacyBroadDiscoveryPolicy::Disabled => "disabled".to_string(),
        domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn => {
            "workflow_opt_in".to_string()
        }
    }
}

fn legacy_broad_discovery_truncation_reason(
    snapshot: Option<&domain::discovery::LegacyBroadDiscoverySnapshot>,
) -> Option<String> {
    let snapshot = snapshot?;
    if snapshot.timed_out {
        Some("timeout".to_string())
    } else if snapshot.truncated_by_file_cap {
        Some("file_cap".to_string())
    } else if snapshot.truncated_by_file_size {
        Some("per_file_bytes".to_string())
    } else if snapshot.truncated_by_total_bytes {
        Some("total_bytes".to_string())
    } else {
        None
    }
}

fn load_p053_cap_validation_metrics(
    workspace_root: &str,
) -> (Option<u64>, Option<u64>, Option<u64>) {
    let path = std::path::Path::new(workspace_root).join(
        "docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/cap-validation.json",
    );
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (None, None, None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (None, None, None);
    };
    (
        value["sampled_execution_ids"]
            .as_array()
            .map(|items| items.len() as u64),
        value["per_output_bytes_p90"].as_u64(),
        value["aggregate_bytes_p90"].as_u64(),
    )
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

fn observed_failure_classification_for_execution_result(
    result_status: &AgentStatus,
    transcript_text: Option<&str>,
) -> Option<RuntimeFailureClassification> {
    if *result_status != AgentStatus::Failed {
        return None;
    }
    transcript_text.map(|text| classify_observation(observation_from_acp_error_message(text)))
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

fn raw_status_from_artifact_path(path: &str) -> String {
    extract_contract_status_from_file("", path)
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string())
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
        acp.set_xcode_runtime_observation_sink(Arc::new(DbXcodeRuntimeObservationSink {
            pool: pool.clone(),
            events: events.clone(),
        }));
        acp.set_prompt_progress_sink(Arc::new(DbAcpPromptProgressSink {
            pool: pool.clone(),
            events: events.clone(),
        }));
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
        acp.set_xcode_runtime_observation_sink(Arc::new(DbXcodeRuntimeObservationSink {
            pool: pool.clone(),
            events: events.clone(),
        }));
        acp.set_prompt_progress_sink(Arc::new(DbAcpPromptProgressSink {
            pool: pool.clone(),
            events: events.clone(),
        }));
        Self {
            pool,
            work_queue,
            orchestrator,
            acp,
            events,
            steward_runtime_inputs: Some(steward_runtime_inputs),
        }
    }

    pub async fn persist_implementation_self_assessment_summary_if_applicable(
        &self,
        artifact: &domain::artifact::Artifact,
    ) -> Result<()> {
        if artifact.contract_id
            != domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID
            && artifact.name != "implementation_self_assessment"
            && artifact.name != "implementation_self_assessment_v2"
        {
            return Ok(());
        }

        let bytes = std::fs::read(&artifact.file_path).with_context(|| {
            format!(
                "read implementation self-assessment artifact {}",
                artifact.file_path
            )
        })?;
        let context = domain::artifact_contracts::ContractParseContext {
            run_id: artifact.run_id.to_string(),
            declared_contract_id: Some(artifact.contract_id.clone()),
            canonical_artifact_path:
                domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_artifact_path: Some(artifact.file_path.clone()),
            artifact_created_at: Some(artifact.created_at),
            v2_generation_seen_for_run: true,
            ..domain::artifact_contracts::ContractParseContext::default()
        };
        let summary = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => {
                domain::artifact_contracts::parse_implementation_self_assessment_v2(&value, context)
            }
            Err(error) => {
                domain::artifact_contracts::invalid_implementation_self_assessment_summary(
                    context,
                    "malformed_json",
                    format!("artifact is not valid JSON: {error}"),
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

    /// Start the background loop. Returns a JoinHandle.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let housekeeping_executor = Arc::clone(&self);
        tokio::spawn(async move {
            housekeeping_executor
                .run_generated_state_housekeeping_loop()
                .await;
        });

        let mediation_expiry_executor = Arc::clone(&self);
        tokio::spawn(async move {
            mediation_expiry_executor
                .run_mediation_expiry_watchdog()
                .await;
        });

        tokio::spawn(async move {
            self.run_loop().await;
        })
    }

    async fn run_generated_state_housekeeping_loop(self: Arc<Self>) {
        let config = GeneratedStateHousekeepingConfig::from_env();
        if !config.enabled {
            info!("Generated-state housekeeping disabled");
            return;
        }

        info!(
            interval_secs = config.interval.as_secs(),
            min_age_secs = config.min_age.as_secs(),
            "Generated-state housekeeping loop started"
        );

        loop {
            if let Err(error) = GeneratedStateHousekeeper::run_once(&self.pool, &config).await {
                warn!(error = %error, "Generated-state housekeeping failed");
            }
            sleep(config.interval).await;
        }
    }

    /// P017 Phase B: Engine-owned deadline expiry watchdog for mediation confirmations.
    /// Periodically checks for pending confirmations past their deadline_at and settles
    /// them as expired, which also settles the linked mediation as terminal_unverifiable.
    async fn run_mediation_expiry_watchdog(self: Arc<Self>) {
        let interval = Duration::from_secs(
            std::env::var("P017_MEDIATION_EXPIRY_CHECK_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
        );

        // CL-010: Use a longer interval when the flag is off to avoid
        // waking every 60s for no work. Check at 5x the normal interval.
        let disabled_interval = Duration::from_secs(interval.as_secs().saturating_mul(5).max(300));

        if !crate::mediation::feature_flag::is_phase_b_mediation_enabled() {
            info!("P017 mediation expiry watchdog: Phase B not enabled, checking at reduced frequency");
        }

        loop {
            if crate::mediation::feature_flag::is_phase_b_mediation_enabled() {
                sleep(interval).await;
            } else {
                sleep(disabled_interval).await;
                continue;
            }

            let now = chrono::Utc::now();
            // MF-PRE-ENABLE-001: Find candidates first, then expire + settle
            // each one atomically in a single IMMEDIATE transaction.
            match db::repos::lead_mediation_confirmations::find_pending_past_deadline(
                &self.pool, now,
            )
            .await
            {
                Ok(candidates) => {
                    if candidates.is_empty() {
                        continue;
                    }
                    let mut settled_count = 0u64;
                    for candidate in &candidates {
                        let now = chrono::Utc::now();
                        match db::pool::begin_immediate_with_retry(
                            &self.pool,
                            "mediation.expire_and_settle",
                        )
                        .await
                        {
                            Ok(mut tx) => {
                                // Expire the confirmation within the tx (CAS: only if still pending).
                                let rows =
                                    match db::repos::lead_mediation_confirmations::expire_one_tx(
                                        &mut tx,
                                        &candidate.id,
                                        now,
                                    )
                                    .await
                                    {
                                        Ok(r) => r,
                                        Err(e) => {
                                            warn!(
                                                confirmation_id = %candidate.id,
                                                error = %e,
                                                "Failed to expire confirmation in tx"
                                            );
                                            continue;
                                        }
                                    };
                                if rows == 0 {
                                    // Already resolved/expired by a concurrent path — skip.
                                    continue;
                                }
                                // Settle the linked mediation in the same tx.
                                if let Err(e) = crate::mediation::settlement::settle_expired_tx(
                                    &mut tx,
                                    &candidate.mediation_record_id,
                                    now,
                                )
                                .await
                                {
                                    warn!(
                                        confirmation_id = %candidate.id,
                                        mediation_id = %candidate.mediation_record_id,
                                        error = %e,
                                        "Failed to settle expired mediation in tx"
                                    );
                                    // tx drops without commit — both expire and settle are rolled back.
                                    continue;
                                }
                                if let Err(e) = tx.commit().await {
                                    warn!(
                                        confirmation_id = %candidate.id,
                                        error = %e,
                                        "Failed to commit expire+settle tx"
                                    );
                                    continue;
                                }
                                settled_count += 1;
                                info!(
                                    confirmation_id = %candidate.id,
                                    mediation_id = %candidate.mediation_record_id,
                                    "Atomically expired confirmation and settled mediation"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    confirmation_id = %candidate.id,
                                    error = %e,
                                    "Failed to begin tx for expire+settle"
                                );
                            }
                        }
                    }
                    if settled_count > 0 {
                        info!(
                            count = settled_count,
                            "P017 mediation expiry watchdog: atomically expired and settled confirmations"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "P017 mediation expiry watchdog: failed to check for expired confirmations"
                    );
                }
            }
        }
    }

    async fn mark_agent_execution_failed_if_running(
        &self,
        agent_execution_id: domain::ids::AgentExecutionId,
        item_id: &str,
        error_message: &str,
    ) {
        match agent_executions::find_by_id(&self.pool, agent_execution_id).await {
            Ok(Some(execution)) if execution.status == AgentStatus::Running => {
                if let Err(update_error) = agent_executions::update_completed(
                    &self.pool,
                    agent_execution_id,
                    AgentStatus::Failed,
                    chrono::Utc::now(),
                )
                .await
                {
                    error!(
                        item_id = %item_id,
                        agent_execution_id = %agent_execution_id,
                        error = %update_error,
                        "Failed to close running agent execution after InvokeAgent work item failure"
                    );
                } else {
                    warn!(
                        item_id = %item_id,
                        agent_execution_id = %agent_execution_id,
                        failure = %error_message,
                        "Closed stale running agent execution after InvokeAgent work item failure"
                    );
                }
            }
            Ok(Some(_)) | Ok(None) => {}
            Err(find_error) => {
                error!(
                    item_id = %item_id,
                    agent_execution_id = %agent_execution_id,
                    error = %find_error,
                    "Failed to inspect agent execution after InvokeAgent work item failure"
                );
            }
        }
    }

    /// Claim and process the next pending work item. Returns `Ok(true)` if an
    /// item was processed, `Ok(false)` if the queue was empty.
    /// Intended for test use — the production path uses `start()`.
    pub async fn process_next_item(&self) -> Result<bool> {
        match self.claim_next_processing_item().await? {
            Some(item) => {
                let item_id = item.id.clone();
                let kind = item.kind.clone();
                info!(item_id = %item_id, kind = %kind, "process_next_item: processing");
                match self.process_item(item).await {
                    Ok(()) => {
                        if let Err(e) = self.work_queue.complete(&item_id).await {
                            if is_transient_persistence_contention_error(&e) {
                                let message = e.to_string();
                                let requeued = self
                                    .work_queue
                                    .requeue_after_transient_persistence_contention(
                                        &item_id, &message,
                                    )
                                    .await?;
                                if requeued {
                                    warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention during completion");
                                    return Ok(true);
                                }
                            }
                            return Err(e);
                        }
                        Ok(true)
                    }
                    Err(e) if is_work_item_requeued(&e) => {
                        info!(item_id = %item_id, kind = %kind, reason = %e, "Work item requeued");
                        Ok(true)
                    }
                    Err(e) if is_transient_persistence_contention_error(&e) => {
                        let message = e.to_string();
                        let requeued = self
                            .work_queue
                            .requeue_after_transient_persistence_contention(&item_id, &message)
                            .await?;
                        if requeued {
                            warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention");
                            Ok(true)
                        } else {
                            self.work_queue.fail(&item_id, &message).await?;
                            Err(e)
                        }
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

    async fn claim_next_processing_item(&self) -> Result<Option<WorkItem>> {
        if let Some(item) = self.work_queue.claim_next().await? {
            return Ok(Some(item));
        }

        Ok(claim_next_invoke_agent_with_start_internal(
            &self.pool,
            &InvokeAgentCapacityConfig::unbounded(),
        )
        .await?
        .map(|(_, item)| item))
    }

    async fn auto_requeue_active_prompt_close(
        &self,
        item: &WorkItem,
        claimed: &ClaimedInvokeAgentStart,
        policy_decision: Option<&SessionPolicyDecision>,
        run_id: RunId,
        stage_id: &str,
        agent_id: &str,
        provider: &str,
        error: &Error,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool> {
        let message = error.to_string();
        if !is_active_prompt_closed_transport_error(&message)
            || item.attempt_count >= ACTIVE_PROMPT_CLOSE_AUTO_RECOVERY_MAX_ATTEMPTS
        {
            return Ok(false);
        }

        let runtime_facts =
            runtime_facts_for_acp_error(claimed.agent_execution_id, error, completed_at);
        agent_executions::update_completed(
            &self.pool,
            claimed.agent_execution_id,
            AgentStatus::Failed,
            completed_at,
        )
        .await?;
        agent_execution_runtime_facts::upsert(&self.pool, &runtime_facts).await?;

        if let Some(decision) = policy_decision {
            let _ = self.acp.close_session(&decision.generation.id).await;
            sessions::end_generation(
                &self.pool,
                &decision.generation.id,
                domain::session::SessionGenerationStatus::Invalidated,
                "active_prompt_transport_closed",
                completed_at,
            )
            .await?;
            sessions::insert_event(
                &self.pool,
                &domain::session::SessionEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    lineage_id: decision.lineage.id.clone(),
                    generation_id: decision.generation.id.clone(),
                    event_type: domain::session::SessionEventType::Invalidated,
                    recorded_at: completed_at,
                    details_json: Some(
                        serde_json::json!({ "reason": "active_prompt_transport_closed" })
                            .to_string(),
                    ),
                },
            )
            .await?;
        }

        let requeued = work_items::requeue_running_invoke_agent_after_active_prompt_close(
            &self.pool,
            &item.id,
            &claimed.artifact_claim_key,
            policy_decision.map(|decision| decision.generation.id.as_str()),
            completed_at,
            "active_prompt_transport_closed",
        )
        .await?;
        if !requeued {
            return Ok(false);
        }

        self.work_queue.refresh_scheduler_projection().await?;
        let _ = self
            .events
            .send(domain::events::DomainEvent::RuntimeStatusChanged {
                run_id,
                stage_id: stage_id.to_string(),
                agent_id: agent_id.to_string(),
                provider: provider.to_string(),
                event_kind: "session_requeued_after_transport_closed".to_string(),
            });
        warn!(
            run_id = %run_id,
            stage_id = %stage_id,
            agent_id = %agent_id,
            agent_execution_id = %claimed.agent_execution_id,
            work_item_id = %item.id,
            attempt_count = item.attempt_count,
            max_attempts = ACTIVE_PROMPT_CLOSE_AUTO_RECOVERY_MAX_ATTEMPTS,
            "ACP active prompt closed; requeued InvokeAgent with a fresh session"
        );
        Ok(true)
    }

    async fn run_loop(self: &Arc<Self>) {
        info!("BackgroundExecutor: starting work loop");
        loop {
            match self.claim_next_processing_item().await {
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
                                        if is_transient_persistence_contention_error(&e) {
                                            let message = e.to_string();
                                            match executor
                                                .work_queue
                                                .requeue_after_transient_persistence_contention(
                                                    &item_id, &message,
                                                )
                                                .await
                                            {
                                                Ok(true) => {
                                                    warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention during completion");
                                                }
                                                Ok(false) => {
                                                    error!(item_id = %item_id, error = %message, "Transient SQLite contention during completion but work item was no longer running");
                                                }
                                                Err(e2) => {
                                                    error!(item_id = %item_id, error = %e2, "Failed to requeue work item after transient SQLite contention during completion");
                                                }
                                            }
                                        } else {
                                            error!(item_id = %item_id, error = %e, "Failed to mark work item complete");
                                        }
                                    }
                                }
                                Err(e) if is_work_item_requeued(&e) => {
                                    info!(item_id = %item_id, kind = %kind, reason = %e, "Work item requeued");
                                }
                                Err(e) if is_transient_persistence_contention_error(&e) => {
                                    let message = e.to_string();
                                    match executor
                                        .work_queue
                                        .requeue_after_transient_persistence_contention(
                                            &item_id, &message,
                                        )
                                        .await
                                    {
                                        Ok(true) => {
                                            warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention");
                                        }
                                        Ok(false) => {
                                            if let Err(e2) =
                                                executor.work_queue.fail(&item_id, &message).await
                                            {
                                                error!(item_id = %item_id, error = %e2, "Failed to mark work item failed after transient contention requeue no-op");
                                            }
                                        }
                                        Err(e2) => {
                                            error!(item_id = %item_id, error = %e2, "Failed to requeue work item after transient SQLite contention");
                                        }
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
                                    if is_transient_persistence_contention_error(&e) {
                                        let message = e.to_string();
                                        match self
                                            .work_queue
                                            .requeue_after_transient_persistence_contention(
                                                &item_id, &message,
                                            )
                                            .await
                                        {
                                            Ok(true) => {
                                                warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention during completion");
                                            }
                                            Ok(false) => {
                                                error!(item_id = %item_id, error = %message, "Transient SQLite contention during completion but work item was no longer running");
                                            }
                                            Err(e2) => {
                                                error!(item_id = %item_id, error = %e2, "Failed to requeue work item after transient SQLite contention during completion");
                                            }
                                        }
                                    } else {
                                        error!(item_id = %item_id, error = %e, "Failed to mark work item complete");
                                    }
                                }
                            }
                            Err(e) if is_work_item_requeued(&e) => {
                                info!(item_id = %item_id, kind = %kind, reason = %e, "Work item requeued");
                            }
                            Err(e) if is_transient_persistence_contention_error(&e) => {
                                let message = e.to_string();
                                match self
                                    .work_queue
                                    .requeue_after_transient_persistence_contention(
                                        &item_id, &message,
                                    )
                                    .await
                                {
                                    Ok(true) => {
                                        warn!(item_id = %item_id, kind = %kind, error = %message, "Work item requeued after transient SQLite contention");
                                    }
                                    Ok(false) => {
                                        if let Err(e2) =
                                            self.work_queue.fail(&item_id, &message).await
                                        {
                                            error!(item_id = %item_id, error = %e2, "Failed to mark work item failed after transient contention requeue no-op");
                                        }
                                    }
                                    Err(e2) => {
                                        error!(item_id = %item_id, error = %e2, "Failed to requeue work item after transient SQLite contention");
                                    }
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

                // P017: Owner-aware execution identity. Defaults to stage_execution
                // for backwards compatibility with all existing work items.
                let owner_kind = payload["owner_kind"]
                    .as_str()
                    .unwrap_or("stage_execution")
                    .to_string();
                let owner_id = payload["owner_id"].as_str().map(String::from);

                // MF-PRE-ENABLE-003: For mediation-owned executions, stage_execution_id
                // must remain None — never synthesize a fake StageExecutionId.
                let is_mediation_owned = owner_kind == "lead_conflict_mediation";

                let stage_execution_id_str = payload["stage_execution_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let stage_execution_id: Option<domain::ids::StageExecutionId> =
                    if stage_execution_id_str.is_empty() {
                        if is_mediation_owned {
                            if owner_id.is_none() {
                                return Err(anyhow::anyhow!(
                                    "Mediation-owned execution requires owner_id in payload"
                                ));
                            }
                            None
                        } else {
                            Some(domain::ids::StageExecutionId::new())
                        }
                    } else {
                        Some(
                            stage_execution_id_str
                                .parse()
                                .map_err(|e| anyhow::anyhow!("{}", e))?,
                        )
                    };

                let origin_stage_id = payload["origin_stage_id"].as_str().map(String::from);
                let origin_stage_execution_id = payload["origin_stage_execution_id"]
                    .as_str()
                    .map(String::from);
                let mediation_record_id = payload["mediation_record_id"].as_str().map(String::from);

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
                let mut requested_mcp_server_ids: Vec<String> =
                    serde_json::from_value(payload["requested_mcp_server_ids"].clone())
                        .unwrap_or_default();
                if suppress_interactive_review_xcode_mcp_for_invocation(
                    &agent_id,
                    payload["backend_profile_id"].as_str(),
                    payload["permission_profile"].as_str(),
                ) {
                    let before = requested_mcp_server_ids.len();
                    requested_mcp_server_ids.retain(|id| id != "xcode");
                    if requested_mcp_server_ids.len() != before {
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            agent_id = %agent_id,
                            "Suppressing interactive Xcode MCP lease for read-only review/audit invocation"
                        );
                    }
                }
                let mut mcp_resolution = crate::mcp::resolve_mcp_servers(
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
                let mut legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy = payload
                    .get("legacy_broad_discovery_policy")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| anyhow::anyhow!("parse legacy_broad_discovery_policy: {e}"))?
                    .unwrap_or_default();
                let session_reuse_scope = payload["session_reuse_scope"].as_str().map(String::from);
                let session_family_id = payload["session_family_id"].as_str().map(String::from);
                let xcode_broker_required = !suppress_interactive_review_xcode_mcp_for_invocation(
                    &agent_id,
                    backend_profile_id.as_deref(),
                    permission_profile.as_deref(),
                ) && payload["xcode_broker_required"]
                    .as_bool()
                    .unwrap_or_else(|| requested_mcp_server_ids.iter().any(|id| id == "xcode"));
                let xcode_shim_injection_signal = payload["xcode_shim_injection_signal"]
                    .as_bool()
                    .unwrap_or(false);
                let requires_xcode_host_execution = payload["requires_xcode_host_execution"]
                    .as_bool()
                    .unwrap_or(false);
                let xcode_shim_required =
                    xcode_shim_injection_signal || requires_xcode_host_execution;
                let declared_outputs: Vec<DeclaredOutput> =
                    serde_json::from_value(payload["declared_outputs"].clone()).unwrap_or_default();
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
                let expected_outputs = build_expected_output_specs(
                    &declared_outputs,
                    &run.workspace_root,
                    run.worktree_root.as_deref(),
                    run.chainworks_meta_root.as_deref(),
                    worktree_write_enabled,
                );

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
                crate::mcp::attach_xcode_broker_execution_context(
                    &mut mcp_resolution.payloads,
                    &run.workspace_root,
                    permission_profile.as_deref(),
                );
                let xcode_broker_contract_hash =
                    crate::mcp::xcode_broker_contract_hash(&mcp_resolution.payloads);
                let resolved_model = model.clone().unwrap_or_else(|| "default".into());
                let now = chrono::Utc::now();
                let preclaimed_start = payload
                    .get("p058_claimed")
                    .map(|claimed| claimed_invoke_agent_start_from_payload(&item, claimed))
                    .transpose()?;
                let agent_exec_id = preclaimed_start
                    .as_ref()
                    .map(|claimed| claimed.agent_execution_id)
                    .unwrap_or_else(domain::ids::AgentExecutionId::new);
                // P017: Owner-aware lineage. For mediation-owned executions, the
                // owner_id (mediation record id) is the lineage anchor; for stage-owned
                // executions, the stage_execution_id remains the lineage anchor.
                let owner_execution_lineage_id = if owner_kind == "lead_conflict_mediation" {
                    owner_id.clone().ok_or_else(|| {
                        anyhow::anyhow!("Mediation-owned execution requires owner_id in payload")
                    })?
                } else {
                    stage_execution_id
                        .ok_or_else(|| {
                            anyhow::anyhow!("Stage-owned execution requires stage_execution_id")
                        })?
                        .to_string()
                };
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
                        xcode_broker_contract_hash: xcode_broker_contract_hash.as_deref(),
                        xcode_broker_required,
                        xcode_shim_injection_signal,
                        requires_xcode_host_execution,
                        skill_snapshot_hash: skill_snapshot_hash.as_deref(),
                        skill_ref: skill_ref.as_deref(),
                        skill_role: skill_role.as_deref(),
                        output_contract: output_contract.as_deref(),
                        max_turns,
                        temperature,
                    }),
                };

                let mut policy_decision: Option<SessionPolicyDecision> =
                    if session_reuse_scope.is_some() && !xcode_shim_required {
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
                if let (Some(claimed), Some(decision)) =
                    (preclaimed_start.as_ref(), policy_decision.as_ref())
                {
                    let disposition = serde_json::to_value(&decision.disposition)
                        .ok()
                        .and_then(|value| value.as_str().map(String::from));
                    agent_executions::update_session_policy(
                        &self.pool,
                        claimed.agent_execution_id,
                        Some(&decision.lineage.id),
                        Some(&decision.generation.id),
                        decision
                            .generation
                            .rehydrated_from_checkpoint_artifact_id
                            .as_deref(),
                        Some(&decision.generation.invocation_owner_key),
                        disposition.as_deref(),
                        decision.session_reset_reason.as_deref(),
                    )
                    .await?;
                    artifact_contracts::update_source_generation_claim_session(
                        &self.pool,
                        &claimed.artifact_claim_key,
                        Some(&decision.generation.id),
                    )
                    .await?;

                    let mut facts =
                        domain::agent::AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, now);
                    facts.session_reuse_reason =
                        Some(session_reuse_reason_for_policy_decision(decision));
                    agent_execution_runtime_facts::upsert(&self.pool, &facts).await?;
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

                if preclaimed_start.is_none() {
                    let agent_exec = domain::agent::AgentExecution {
                        id: agent_exec_id,
                        stage_execution_id,
                        agent_id: agent_id.clone(),
                        provider: provider.clone(),
                        model: model.clone(),
                        status: domain::agent::AgentStatus::Running,
                        started_at: now,
                        completed_at: None,
                        owner_execution_lineage_id: Some(owner_execution_lineage_id.clone()),
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
                        actual_xcode_runtime_observation_json: None,
                        mcp_session_startup_latency_ms: None,
                        owner_kind: Some(owner_kind.clone()),
                        // MF-PRE-ENABLE-003: For mediation-owned executions, owner_id
                        // must come from the payload — never fall back to synthetic stage ID.
                        owner_id: if is_mediation_owned {
                            owner_id.clone()
                        } else {
                            owner_id
                                .clone()
                                .or_else(|| stage_execution_id.map(|id| id.to_string()))
                        },
                        lead_mediation_record_id: mediation_record_id.clone(),
                        origin_stage_execution_id: origin_stage_execution_id.clone(),
                        // P017 R4 / API-002: cost & transcript filled in by
                        // update_attempt_attribution_tx after the provider
                        // returns; insertion-time row holds None.
                        total_cost_cents: None,
                        input_tokens: None,
                        output_tokens: None,
                        cached_input_tokens: None,
                        transcript_artifact_id: None,
                        actual_toolchain_mapping_diagnostics_json: None,
                    };
                    agent_executions::insert(&self.pool, &agent_exec).await?;
                }

                // Reconcile pre-claimed agent execution with freshly evaluated session policy.
                // When InvokeAgent items are claimed via claim_next_invoke_agent_with_start,
                // the agent execution row is created without session policy data. Update it now.
                if let (Some(claimed), Some(decision)) =
                    (preclaimed_start.as_ref(), policy_decision.as_ref())
                {
                    let disposition = serde_json::to_value(&decision.disposition)
                        .ok()
                        .and_then(|value| value.as_str().map(String::from));
                    agent_executions::update_session_policy(
                        &self.pool,
                        claimed.agent_execution_id,
                        Some(&decision.lineage.id),
                        Some(&decision.generation.id),
                        decision
                            .generation
                            .rehydrated_from_checkpoint_artifact_id
                            .as_deref(),
                        Some(&decision.generation.invocation_owner_key),
                        disposition.as_deref(),
                        decision.session_reset_reason.as_deref(),
                    )
                    .await?;
                    artifact_contracts::update_source_generation_claim_session(
                        &self.pool,
                        &claimed.artifact_claim_key,
                        Some(&decision.generation.id),
                    )
                    .await?;

                    let mut facts =
                        domain::agent::AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, now);
                    facts.session_reuse_reason =
                        Some(session_reuse_reason_for_policy_decision(decision));
                    agent_execution_runtime_facts::upsert(&self.pool, &facts).await?;
                }

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
                    let Some(stage_execution_id) = stage_execution_id else {
                        if let Some(med_id) = mediation_record_id.as_deref() {
                            let mut tx = db::pool::begin_immediate_with_retry(
                                &self.pool,
                                "mediation.mcp_resolution_blocked",
                            )
                            .await?;
                            let _ = db::repos::lead_conflict_mediations::update_status_tx(
                                &mut tx,
                                med_id,
                                "terminal_unverifiable",
                                Some("mcp_resolution_blocked"),
                                Some("clone_or_manual_fallback"),
                                completed_at,
                            )
                            .await?;
                            tx.commit().await?;
                        }
                        projections::rebuild_all_for_run(&self.pool, run_id).await?;
                        return Ok(());
                    };
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
                    let stage_execution_id = stage_execution_id.ok_or_else(|| {
                        anyhow::anyhow!("Release agent execution requires stage_execution_id")
                    })?;
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

                let stage_attempt_number = if let Some(stage_execution_id) = stage_execution_id {
                    stages::find_by_id(&self.pool, stage_execution_id)
                        .await?
                        .map(|stage| stage.attempt_number)
                        .unwrap_or(1)
                } else {
                    1
                };
                let mut discovery_override_status =
                    if legacy_broad_discovery_policy.allows_broad_discovery() {
                        "workflow_opt_in".to_string()
                    } else {
                        "not_requested".to_string()
                    };
                if !legacy_broad_discovery_policy.allows_broad_discovery()
                    && stage_execution_id.is_some()
                {
                    let stage_execution_id = stage_execution_id.expect("checked is_some");
                    let mut tx = db::pool::begin_immediate_with_retry(
                        &self.pool,
                        "executor.consume_legacy_discovery_override",
                    )
                    .await?;
                    if let Some(override_row) =
                        legacy_discovery_overrides::consume_pending_for_stage_tx(
                            &mut tx,
                            run_id,
                            &stage_id,
                            stage_execution_id,
                            stage_attempt_number,
                        )
                        .await?
                    {
                        legacy_broad_discovery_policy = override_row.requested_policy;
                        discovery_override_status = "consumed".to_string();
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            stage_execution_id = %stage_execution_id,
                            attempt = stage_attempt_number,
                            override_id = %override_row.override_id,
                            "P053 legacy broad discovery override consumed for this prompt"
                        );
                    }
                    tx.commit().await?;
                }

                let execution_prompt = if policy_decision.is_some() || !declared_outputs.is_empty()
                {
                    prompt_with_runtime_invocation_contract(
                        prompt.clone(),
                        RuntimeInvocationContractInput {
                            run_id: run_id.to_string(),
                            stage_id: stage_id.clone(),
                            stage_execution_id: stage_execution_id
                                .map(|id| id.to_string())
                                .unwrap_or_else(|| owner_execution_lineage_id.clone()),
                            agent_execution_id: agent_exec_id.to_string(),
                            work_item_id: item.id.clone(),
                            session_generation_id: policy_decision
                                .as_ref()
                                .map(|decision| decision.generation.id.clone()),
                            session_reuse_disposition: policy_decision.as_ref().and_then(
                                |decision| {
                                    serde_json::to_value(&decision.disposition)
                                        .ok()
                                        .and_then(|value| value.as_str().map(String::from))
                                },
                            ),
                            declared_outputs: &declared_outputs,
                        },
                    )
                } else {
                    prompt.clone()
                };
                let estimated_prompt_tokens =
                    std::cmp::max(1_i64, (execution_prompt.chars().count() as i64) / 4);
                let req = acp::ExecutionRequest {
                    agent_execution_id: Some(agent_exec_id),
                    run_id,
                    stage_execution_id: stage_execution_id.map(|id| id.to_string()),
                    stage_id: stage_id.clone(),
                    attempt_number: u32::try_from(stage_attempt_number).unwrap_or(1),
                    agent_id: agent_id.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    effort,
                    workspace_root: run.workspace_root.clone(),
                    prompt: execution_prompt,
                    worktree_root: run.worktree_root.clone(),
                    worktree_write_enabled,
                    worktree_strategy,
                    expected_output_paths,
                    expected_outputs: expected_outputs.clone(),
                    keep_session_alive: policy_decision.is_some() && !xcode_shim_required,
                    reuse_existing_session: policy_decision
                        .as_ref()
                        .map(|decision| decision.should_reuse_live_session)
                        .unwrap_or(false)
                        && !xcode_shim_required,
                    session_generation_id: policy_decision
                        .as_ref()
                        .map(|decision| decision.generation.id.clone()),
                    provider_session_id: policy_decision
                        .as_ref()
                        .and_then(|decision| decision.generation.provider_session_id.clone()),
                    mcp_servers: mcp_resolution.payloads,
                    chainworks_meta_root: run.chainworks_meta_root.clone(),
                    legacy_broad_discovery_policy: legacy_broad_discovery_policy.clone(),
                    xcode_shim_injection_signal,
                    requires_xcode_host_execution,
                    // P017: owner-aware execution identity from payload.
                    // MF-PRE-ENABLE-003: For mediation-owned executions, owner_id and
                    // origin_stage_execution_id must not fall back to synthetic stage IDs.
                    owner_kind: owner_kind.clone(),
                    owner_id: if is_mediation_owned {
                        owner_id.clone()
                    } else {
                        owner_id
                            .clone()
                            .or_else(|| stage_execution_id.map(|id| id.to_string()))
                    },
                    origin_stage_id: origin_stage_id.clone().or_else(|| Some(stage_id.clone())),
                    origin_stage_execution_id: if is_mediation_owned {
                        origin_stage_execution_id.clone()
                    } else {
                        origin_stage_execution_id
                            .clone()
                            .or_else(|| stage_execution_id.map(|id| id.to_string()))
                    },
                    mediation_record_id: mediation_record_id.clone(),
                    toolchain_home: None,
                    toolchain_go_scope_enabled: false,
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

                let mut result = match self.acp.execute(req.clone()).await {
                    Ok(result) => result,
                    Err(error) => {
                        let completed_at = chrono::Utc::now();
                        if let Some(claimed) = preclaimed_start.as_ref() {
                            if self
                                .auto_requeue_active_prompt_close(
                                    &item,
                                    claimed,
                                    policy_decision.as_ref(),
                                    run_id,
                                    &stage_id,
                                    &agent_id,
                                    &provider,
                                    &error,
                                    completed_at,
                                )
                                .await?
                            {
                                return Err(WorkItemRequeued {
                                    work_item_id: item.id.clone(),
                                    reason: "active_prompt_transport_closed",
                                }
                                .into());
                            }
                        }
                        let runtime_facts =
                            runtime_facts_for_acp_error(agent_exec_id, &error, completed_at);
                        if let Err(update_error) = agent_executions::update_completed(
                            &self.pool,
                            agent_exec_id,
                            AgentStatus::Failed,
                            completed_at,
                        )
                        .await
                        {
                            warn!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                agent_execution_id = %agent_exec_id,
                                error = %update_error,
                                "Failed to mark ACP startup failure execution as failed"
                            );
                        }
                        if let Err(update_error) =
                            agent_execution_runtime_facts::upsert(&self.pool, &runtime_facts).await
                        {
                            warn!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                agent_execution_id = %agent_exec_id,
                                error = %update_error,
                                "Failed to persist ACP startup failure runtime facts"
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
                        return Err(error);
                    }
                };
                match agent_executions::find_by_id(&self.pool, agent_exec_id).await? {
                    Some(current_execution) if current_execution.status == AgentStatus::Running => {
                    }
                    Some(current_execution) => {
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            agent_id = %agent_id,
                            agent_execution_id = %agent_exec_id,
                            current_status = %current_execution.status,
                            "Discarding late ACP result for agent execution that is no longer running"
                        );
                        return Ok(());
                    }
                    None => {
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            agent_id = %agent_id,
                            agent_execution_id = %agent_exec_id,
                            "Discarding late ACP result for missing agent execution"
                        );
                        return Ok(());
                    }
                }

                // P017 B2-004: Check mediation staleness for mediation-owned executions.
                // If the mediation has been superseded or canceled while the agent ran,
                // record the output as ignored_late_output and skip artifact persistence.
                let mediation_stale = if owner_kind == "lead_conflict_mediation" {
                    if let Some(ref med_id) = mediation_record_id {
                        match db::repos::lead_conflict_mediations::find_by_id(&self.pool, med_id)
                            .await
                        {
                            Ok(Some(med)) => med.status.is_terminal(),
                            Ok(None) => {
                                warn!(
                                    mediation_id = %med_id,
                                    "Mediation record not found; treating output as stale"
                                );
                                true
                            }
                            Err(e) => {
                                // MC-003: Fail closed on DB error — if we can't verify
                                // mediation status, treat output as stale to prevent
                                // persisting against a terminal or superseded mediation.
                                warn!(
                                    mediation_id = %med_id,
                                    error = %e,
                                    "Failed to check mediation staleness; treating as stale (fail-closed)"
                                );
                                true
                            }
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if mediation_stale {
                    info!(
                        run_id = %run_id,
                        agent_id = %agent_id,
                        mediation_record_id = ?mediation_record_id,
                        "Mediation-owned execution output is stale; recording as ignored_late_output"
                    );
                    let completed_at = chrono::Utc::now();
                    let mut facts =
                        AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, completed_at);
                    facts.output_settlement = AgentOutputSettlement::IgnoredLateOutputs;
                    facts.ignored_late_output_count = 1;
                    facts.late_output_count = 1;
                    facts.valid_required_outputs = false;
                    agent_execution_runtime_facts::upsert(&self.pool, &facts).await?;
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Failed,
                        completed_at,
                    )
                    .await?;
                    // OPS-002 (P017 R4): emit mediation_late_output_ignored_total
                    // per ignored late output so dashboards can detect
                    // unexpected provider lag against superseded/canceled
                    // mediations.
                    if let Some(ref med_id) = mediation_record_id {
                        let mut metric_tx = self.pool.begin().await?;
                        let _ =
                            db::repos::workflow_conflicts::record_mediation_late_output_ignored_tx(
                                &mut metric_tx,
                                &run_id.to_string(),
                                None,
                                med_id,
                                "mediation_terminal_or_missing",
                                completed_at,
                            )
                            .await;
                        let _ = metric_tx.commit().await;
                    }
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    return Ok(());
                }

                // P017 B2-004: Check mediation staleness for mediation-owned executions.
                // If the mediation has been superseded or canceled while the agent ran,
                // record the output as ignored_late_output and skip artifact persistence.
                let mediation_stale = if owner_kind == "lead_conflict_mediation" {
                    if let Some(ref med_id) = mediation_record_id {
                        match db::repos::lead_conflict_mediations::find_by_id(&self.pool, med_id)
                            .await
                        {
                            Ok(Some(med)) => med.status.is_terminal(),
                            Ok(None) => {
                                warn!(
                                    mediation_id = %med_id,
                                    "Mediation record not found; treating output as stale"
                                );
                                true
                            }
                            Err(e) => {
                                // MC-003: Fail closed on DB error — if we can't verify
                                // mediation status, treat output as stale to prevent
                                // persisting against a terminal or superseded mediation.
                                warn!(
                                    mediation_id = %med_id,
                                    error = %e,
                                    "Failed to check mediation staleness; treating as stale (fail-closed)"
                                );
                                true
                            }
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if mediation_stale {
                    info!(
                        run_id = %run_id,
                        agent_id = %agent_id,
                        mediation_record_id = ?mediation_record_id,
                        "Mediation-owned execution output is stale; recording as ignored_late_output"
                    );
                    let completed_at = chrono::Utc::now();
                    let mut facts =
                        AgentExecutionRuntimeFacts::defaults_for(agent_exec_id, completed_at);
                    facts.output_settlement = AgentOutputSettlement::IgnoredLateOutputs;
                    facts.ignored_late_output_count = 1;
                    facts.late_output_count = 1;
                    facts.valid_required_outputs = false;
                    agent_execution_runtime_facts::upsert(&self.pool, &facts).await?;
                    agent_executions::update_completed(
                        &self.pool,
                        agent_exec_id,
                        AgentStatus::Failed,
                        completed_at,
                    )
                    .await?;
                    // OPS-002 (P017 R4): emit mediation_late_output_ignored_total
                    // per ignored late output so dashboards can detect
                    // unexpected provider lag against superseded/canceled
                    // mediations.
                    if let Some(ref med_id) = mediation_record_id {
                        let mut metric_tx = self.pool.begin().await?;
                        let _ =
                            db::repos::workflow_conflicts::record_mediation_late_output_ignored_tx(
                                &mut metric_tx,
                                &run_id.to_string(),
                                None,
                                med_id,
                                "mediation_terminal_or_missing",
                                completed_at,
                            )
                            .await;
                        let _ = metric_tx.commit().await;
                    }
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    return Ok(());
                }

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

                let mut acp_control_plane_manifest_latency_ms: Option<u64> = None;
                let mut acp_git_changed_files_latency_ms: Option<u64> = None;
                let mut acp_git_manifest_status: Option<String> = None;
                let mut acp_exact_output_acceptance_latency_ms: Option<u64> = None;
                let mut acp_resume_discovery_warning: Option<String> = None;
                let mut declared_output_settlement = if !declared_outputs.is_empty() {
                    let manifest_started = Instant::now();
                    match generate_changed_files_manifest_if_declared(
                        &declared_outputs,
                        Some(effective_working_directory.as_str()),
                        worktree_write_enabled,
                    )
                    .await
                    {
                        Ok(status) => {
                            acp_git_manifest_status = status.map(|status| {
                                serde_json::to_value(status)
                                    .ok()
                                    .and_then(|value| value.as_str().map(str::to_string))
                                    .unwrap_or_else(|| "unknown".to_string())
                            });
                        }
                        Err(error) => {
                            acp_git_manifest_status = Some("command_failed".to_string());
                            acp_resume_discovery_warning =
                                Some("git_manifest_generation_failed".to_string());
                            error!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                error = %error,
                                "Failed to generate changed-files manifest"
                            );
                        }
                    }
                    let manifest_latency_ms = manifest_started.elapsed().as_millis() as u64;
                    acp_control_plane_manifest_latency_ms = Some(manifest_latency_ms);
                    acp_git_changed_files_latency_ms = Some(manifest_latency_ms);
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        agent_id = %agent_id,
                        acp_control_plane_manifest_latency_ms = manifest_latency_ms,
                        acp_git_changed_files_latency_ms = manifest_latency_ms,
                        acp_git_manifest_status = acp_git_manifest_status.as_deref().unwrap_or("unknown"),
                        "P053 control-plane manifest generation measured"
                    );
                    let exact_output_acceptance_started = Instant::now();
                    let settlement = settle_agent_outputs_from_discovery_decisions(
                        &declared_outputs,
                        &expected_outputs,
                        &result.discovered_artifacts,
                        &result.pre_prompt_expected_outputs,
                    )?;
                    let found_count = settlement
                        .decisions
                        .iter()
                        .filter(|decision| decision.status == OutputDiscoveryStatus::Accepted)
                        .count();
                    let missing_count = settlement
                        .decisions
                        .iter()
                        .filter(|decision| decision.status == OutputDiscoveryStatus::Missing)
                        .count();
                    let stale_count = settlement
                        .decisions
                        .iter()
                        .filter(|decision| {
                            decision.reason == OutputDiscoveryReason::StaleExpectedOutput
                        })
                        .count();
                    let rejected_count = settlement
                        .decisions
                        .iter()
                        .filter(|decision| decision.status == OutputDiscoveryStatus::Rejected)
                        .count();
                    let exact_output_acceptance_latency_ms =
                        exact_output_acceptance_started.elapsed().as_millis() as u64;
                    acp_exact_output_acceptance_latency_ms =
                        Some(exact_output_acceptance_latency_ms);
                    info!(
                        run_id = %run_id,
                        stage_id = %stage_id,
                        agent_id = %agent_id,
                        acp_exact_output_acceptance_latency_ms = exact_output_acceptance_latency_ms,
                        acp_expected_outputs_found_count = found_count,
                        acp_expected_outputs_missing_count = missing_count,
                        acp_expected_outputs_stale_count = stale_count,
                        acp_expected_outputs_rejected_count = rejected_count,
                        acp_exact_output_aggregate_bytes = settlement.accepted_aggregate_bytes,
                        acp_exact_output_aggregate_cap_hit = settlement.aggregate_cap_hit,
                        acp_reconciliation_pending = false,
                        "P053 exact-output acceptance measured"
                    );
                    if let Some(idempotency_key) = settlement.idempotency_key.as_deref() {
                        debug!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            agent_id = %agent_id,
                            discovery_settlement_idempotency_key = %idempotency_key,
                            "P053 discovery settlement completed"
                        );
                    }
                    Some(settlement)
                } else {
                    None
                };

                let mut output_contract_repair_turn_count = 0_i64;
                if let Some(settlement) = declared_output_settlement.as_ref() {
                    let captured = build_captured_outputs_from_discovery_decisions(
                        &declared_outputs,
                        &settlement.decisions,
                        &settlement.accepted_payloads,
                    );
                    let validation = validate_task_outputs(&captured);
                    if validation_summary_requires_output_contract_repair(&validation) {
                        if let Some(session_generation_id) =
                            result.session_generation_id.clone().or_else(|| {
                                policy_decision
                                    .as_ref()
                                    .map(|decision| decision.generation.id.clone())
                            })
                        {
                            let mut repair_req = req.clone();
                            repair_req.prompt =
                                output_contract_repair_prompt(&validation, &declared_outputs);
                            repair_req.reuse_existing_session = true;
                            repair_req.keep_session_alive = true;
                            repair_req.session_generation_id = Some(session_generation_id.clone());
                            repair_req.provider_session_id = result
                                .provider_session_id
                                .clone()
                                .or_else(|| repair_req.provider_session_id.clone());

                            match self
                                .acp
                                .prompt_session(&session_generation_id, repair_req)
                                .await
                            {
                                Ok(repair_result) => {
                                    output_contract_repair_turn_count += 1;
                                    match settle_agent_outputs_from_discovery_decisions(
                                        &declared_outputs,
                                        &expected_outputs,
                                        &repair_result.discovered_artifacts,
                                        &repair_result.pre_prompt_expected_outputs,
                                    ) {
                                        Ok(repair_settlement) => {
                                            let repair_captured =
                                                build_captured_outputs_from_discovery_decisions(
                                                    &declared_outputs,
                                                    &repair_settlement.decisions,
                                                    &repair_settlement.accepted_payloads,
                                                );
                                            let repair_validation =
                                                validate_task_outputs(&repair_captured);
                                            if repair_validation.failure_class.is_none() {
                                                merge_contract_repair_result(
                                                    &mut result,
                                                    repair_result,
                                                );
                                                declared_output_settlement =
                                                    Some(repair_settlement);
                                                info!(
                                                    run_id = %run_id,
                                                    stage_id = %stage_id,
                                                    agent_id = %agent_id,
                                                    "Output contract repair turn produced valid declared outputs"
                                                );
                                            } else {
                                                warn!(
                                                    run_id = %run_id,
                                                    stage_id = %stage_id,
                                                    agent_id = %agent_id,
                                                    failure = ?repair_validation.failure_summary,
                                                    "Output contract repair turn did not produce valid declared outputs"
                                                );
                                            }
                                        }
                                        Err(error) => warn!(
                                            run_id = %run_id,
                                            stage_id = %stage_id,
                                            agent_id = %agent_id,
                                            error = %error,
                                            "Output contract repair settlement failed"
                                        ),
                                    }
                                }
                                Err(error) => warn!(
                                    run_id = %run_id,
                                    stage_id = %stage_id,
                                    agent_id = %agent_id,
                                    error = %error,
                                    "Output contract repair turn failed"
                                ),
                            }
                        } else {
                            warn!(
                                run_id = %run_id,
                                stage_id = %stage_id,
                                agent_id = %agent_id,
                                "Output contract repair skipped because no live session generation is available"
                            );
                        }
                    }
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
                    let session_turn_count =
                        decision.generation.turn_count + 1 + output_contract_repair_turn_count;
                    sessions::update_generation_usage(
                        &self.pool,
                        &decision.generation.id,
                        provider_session_id,
                        session_turn_count,
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
                if is_mediation_owned {
                    // P017 R5 / REL-002: completion + attribution +
                    // transcript persistence are now atomic in a single
                    // transaction. Either the entire mediation
                    // completion lands (with cost, transcript, and
                    // attempt attribution intact) or none of it does
                    // and the orchestrator can re-drive the work item.
                    //
                    // The transcript artifact file is written BEFORE the tx
                    // (filesystem write is not transactional), but the
                    // artifact row insert + the agent_execution status update
                    // + the attribution write are bundled into one tx so a
                    // partial database state is impossible.
                    let mediation_transcript_artifact = self
                        .build_transcript_artifact_if_present(
                            &run,
                            &stage_id,
                            &agent_id,
                            &provider,
                            model.clone(),
                            agent_exec_id,
                            completed_at,
                            result.transcript_text.as_deref(),
                        )
                        .await
                        .ok()
                        .flatten();
                    let mediation_transcript_artifact_id = mediation_transcript_artifact
                        .as_ref()
                        .map(|artifact| artifact.id.to_string());
                    let usage_cost_cents = result
                        .usage
                        .as_ref()
                        .and_then(|u| u.cost_cents)
                        .or(result.cost_cents);
                    let usage_input_tokens = result.usage.as_ref().and_then(|u| u.input_tokens);
                    let usage_output_tokens = result.usage.as_ref().and_then(|u| u.output_tokens);
                    let usage_cached_input_tokens =
                        result.usage.as_ref().and_then(|u| u.cached_input_tokens);
                    let mut completion_tx = db::pool::begin_immediate_with_retry(
                        &self.pool,
                        "mediation.complete_with_attribution",
                    )
                    .await?;
                    if let Some(artifact) = mediation_transcript_artifact.as_ref() {
                        artifacts::insert_tx(&mut completion_tx, artifact).await?;
                    }
                    agent_executions::update_completed_tx(
                        &mut completion_tx,
                        agent_exec_id,
                        result.status.clone(),
                        completed_at,
                    )
                    .await?;
                    agent_executions::update_attempt_attribution_tx(
                        &mut completion_tx,
                        agent_exec_id,
                        usage_cost_cents,
                        usage_input_tokens,
                        usage_output_tokens,
                        usage_cached_input_tokens,
                        mediation_transcript_artifact_id.as_deref(),
                    )
                    .await?;
                    completion_tx.commit().await?;
                    if let Some(artifact) = mediation_transcript_artifact.as_ref() {
                        let _ = self
                            .events
                            .send(domain::events::DomainEvent::ArtifactCreated {
                                run_id: run.id,
                                artifact_id: artifact.id,
                            });
                    }
                    if let Some(med_id) = mediation_record_id.as_deref() {
                        let mut tx = db::pool::begin_immediate_with_retry(
                            &self.pool,
                            "mediation.agent_execution_completed",
                        )
                        .await?;
                        if let Some(mediation) =
                            db::repos::lead_conflict_mediations::find_by_id_tx(&mut tx, med_id)
                                .await?
                        {
                            let conflict = db::repos::workflow_conflicts::find_conflict_by_id_tx(
                                &mut tx,
                                &mediation.conflict_id,
                            )
                            .await?;
                            if result.status == AgentStatus::Completed {
                                let validation_summary =
                                    declared_output_settlement.as_ref().map(|settlement| {
                                        let captured =
                                            build_captured_outputs_from_discovery_decisions(
                                                &declared_outputs,
                                                &settlement.decisions,
                                                &settlement.accepted_payloads,
                                            );
                                        validate_task_outputs(&captured)
                                    });
                                let validation_failed = declared_outputs.is_empty()
                                    || validation_summary
                                        .as_ref()
                                        .is_none_or(|summary| summary.failure_class.is_some());

                                if validation_failed {
                                    let validation_errors_json = serde_json::to_string(
                                        &serde_json::json!({
                                            "error": if declared_outputs.is_empty() {
                                                "lead_resolution_contract_missing"
                                            } else {
                                                "lead_resolution_contract_validation_failed"
                                            },
                                            "summary": validation_summary
                                                .as_ref()
                                                .and_then(|summary| summary.failure_summary.clone()),
                                            "output_results": validation_summary
                                                .as_ref()
                                                .map(|summary| &summary.output_results),
                                        }),
                                    )?;
                                    let _ =
                                        db::repos::lead_conflict_mediations::update_after_lead_output_tx(
                                            &mut tx,
                                            med_id,
                                            "terminal_unverifiable",
                                            Some("lead_output_validation_failed"),
                                            Some("clone_or_manual_fallback"),
                                            None,
                                            None,
                                            None,
                                            Some("Lead output failed LeadResolutionContract validation"),
                                            Some(&validation_errors_json),
                                            None,
                                            completed_at,
                                        )
                                        .await?;
                                    if let Some(conflict) = conflict {
                                        db::repos::workflow_conflicts::transition_conflict_status_tx(
                                            &mut tx,
                                            &conflict.conflict_id,
                                            domain::workflow_conflict::WorkflowConflictStatus::TerminalUnverifiable,
                                            completed_at,
                                            Some(serde_json::json!({
                                                "resolution_kind": "lead_mediation_output_validation_failed",
                                                "action_class": "clone_or_manual_fallback",
                                                "mediation_record_id": med_id,
                                            })),
                                            Some("lead_output_validation_failed".to_string()),
                                            None,
                                        )
                                        .await?;
                                    }
                                } else {
                                    let confirmation_id = mediation
                                        .confirmation_subject_id
                                        .clone()
                                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                    if mediation.confirmation_subject_id.is_none() {
                                        db::repos::lead_mediation_confirmations::insert_tx(
                                            &mut tx,
                                            &domain::mediation::LeadMediationConfirmation {
                                                id: confirmation_id.clone(),
                                                mediation_record_id: med_id.to_string(),
                                                run_id: mediation.run_id.clone(),
                                                conflict_id: mediation.conflict_id.clone(),
                                                conflict_fingerprint: mediation
                                                    .conflict_fingerprint
                                                    .clone(),
                                                status: domain::mediation::MediationConfirmationStatus::Pending,
                                                suggested_action: Some(
                                                    "confirm_lead_resolution".to_string(),
                                                ),
                                                requested_at: completed_at,
                                                deadline_at: Some(
                                                    completed_at + chrono::Duration::minutes(30),
                                                ),
                                                readback_ref: Some(format!(
                                                    "workflow_conflict.current.lead_mediation:{}",
                                                    med_id
                                                )),
                                                idempotency_scope_key: Some(format!(
                                                    "{}:{}:{}",
                                                    mediation.run_id,
                                                    mediation.conflict_fingerprint,
                                                    med_id
                                                )),
                                                resolved_at: None,
                                                resolved_by_principal_id: None,
                                                resolution_decision: None,
                                                resolution_comment: None,
                                            },
                                        )
                                        .await?;
                                    }
                                    let _ =
                                        db::repos::lead_conflict_mediations::update_after_lead_output_tx(
                                            &mut tx,
                                            med_id,
                                            "operator_confirmation_required",
                                            Some("lead_output_validated"),
                                            None,
                                            Some("confirm_lead_resolution"),
                                            None,
                                            None,
                                            Some("Lead output validated; awaiting operator confirmation"),
                                            None,
                                            Some(&confirmation_id),
                                            completed_at,
                                        )
                                        .await?;
                                    if let Some(conflict) = conflict {
                                        db::repos::workflow_conflicts::update_mediation_pointer_tx(
                                            &mut tx,
                                            &conflict.conflict_id,
                                            &mediation.lead_agent_id,
                                            med_id,
                                            domain::workflow_conflict::WorkflowConflictStatus::OperatorConfirmationRequired,
                                            completed_at,
                                        )
                                        .await?;
                                    }
                                }
                            } else {
                                let _ = db::repos::lead_conflict_mediations::update_after_lead_output_tx(
                                    &mut tx,
                                    med_id,
                                    "terminal_unverifiable",
                                    Some("agent_failed"),
                                    Some("clone_or_manual_fallback"),
                                    None,
                                    None,
                                    None,
                                    Some("Lead mediation agent failed"),
                                    None,
                                    None,
                                    completed_at,
                                )
                                .await?;
                                if let Some(conflict) = conflict {
                                    db::repos::workflow_conflicts::transition_conflict_status_tx(
                                        &mut tx,
                                        &conflict.conflict_id,
                                        domain::workflow_conflict::WorkflowConflictStatus::TerminalUnverifiable,
                                        completed_at,
                                        Some(serde_json::json!({
                                            "resolution_kind": "lead_mediation_agent_failed",
                                            "action_class": "clone_or_manual_fallback",
                                            "mediation_record_id": med_id,
                                        })),
                                        Some("lead_mediation_agent_failed".to_string()),
                                        None,
                                    )
                                    .await?;
                                }
                            }

                            // OPS-001 (P017 R2 audit): emit one
                            // `lead_mediation_attempt_total` per mediation-owned
                            // execution completion, labeled by per-attempt result.
                            // Attempt number is the durable count of mediation-owned
                            // executions for this mediation observed before this row
                            // was committed (so this completion is attempt N+1).
                            let attempt_number = sqlx::query_scalar::<_, i64>(
                                "SELECT COUNT(*) FROM agent_executions
                                 WHERE owner_kind = 'lead_conflict_mediation'
                                   AND lead_mediation_record_id = ?",
                            )
                            .bind(med_id)
                            .fetch_one(&mut *tx)
                            .await
                            .unwrap_or(1);
                            let attempt_result = match result.status {
                                AgentStatus::Completed => {
                                    // Distinguishing happy path vs validation failure
                                    // requires re-reading the just-written status.
                                    match db::repos::lead_conflict_mediations::find_by_id_tx(
                                        &mut tx, med_id,
                                    )
                                    .await
                                    {
                                        Ok(Some(med)) => match med.settlement_result.as_deref() {
                                            Some("lead_output_validation_failed") => {
                                                "lead_output_validation_failed"
                                            }
                                            Some("lead_output_validated") => {
                                                "validated_awaiting_confirmation"
                                            }
                                            _ => "other",
                                        },
                                        _ => "other",
                                    }
                                }
                                AgentStatus::Failed => "agent_failed",
                                AgentStatus::Cancelled => "cancelled",
                                _ => "other",
                            };
                            let _ =
                                db::repos::workflow_conflicts::record_lead_mediation_attempt_tx(
                                    &mut tx,
                                    &mediation.run_id,
                                    Some(&mediation.conflict_id),
                                    med_id,
                                    &mediation.lead_agent_id,
                                    attempt_result,
                                    attempt_number,
                                    completed_at,
                                )
                                .await;
                        }
                        tx.commit().await?;
                    }
                    projections::rebuild_all_for_run(&self.pool, run_id).await?;
                    return Ok(());
                }
                let stage_execution_id = stage_execution_id.ok_or_else(|| {
                    anyhow::anyhow!("Stage-owned execution requires stage_execution_id")
                })?;
                let mut persisted_paths = std::collections::HashSet::new();
                let mut persisted_artifacts = Vec::new();
                let artifact_claim_key = preclaimed_start
                    .as_ref()
                    .map(|claimed| claimed.artifact_claim_key.clone())
                    .unwrap_or_else(|| ArtifactSourceGenerationClaimKey {
                        run_id,
                        owner_kind: domain::mediation::OwnerKind::StageExecution,
                        owner_id: stage_execution_id.to_string(),
                        stage_execution_id: Some(stage_execution_id),
                        agent_execution_id: agent_exec_id,
                        source_work_item_id: item.id.clone(),
                    });
                // Prefer the policy decision's generation id when available,
                // because reconciliation updated the DB claim to match it.
                // The claimed.session_generation_id may be stale (a random UUID
                // from pre-claim time that doesn't match the real session).
                let source_session_generation_id = policy_decision
                    .as_ref()
                    .map(|decision| decision.generation.id.as_str())
                    .or_else(|| {
                        preclaimed_start
                            .as_ref()
                            .and_then(|claimed| claimed.session_generation_id.as_deref())
                    });
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
                let _transcript_exists = transcript_artifact.is_some();
                if let Some(artifact) = transcript_artifact {
                    persisted_artifacts.push(artifact);
                }
                let mut declared_artifacts = self.prepare_declared_output_artifacts(
                    &declared_outputs,
                    declared_output_settlement
                        .as_ref()
                        .map(|settlement| settlement.decisions.as_slice()),
                    run_id,
                    &stage_id,
                    &agent_id,
                    &provider,
                    model.clone(),
                    completed_at,
                    &mut persisted_paths,
                )?;
                // P017 R5 / API-003: stamp the direct execution-attempt FK
                // on every declared output artifact so MCP/GraphQL
                // `execution_attempts.artifacts` can attribute outputs
                // per attempt without falling back to `agent_id`
                // correlation. Required for cross-retry isolation under
                // mediation-owned executions; harmless for stage-owned.
                for artifact in declared_artifacts.iter_mut() {
                    artifact.agent_execution_id = Some(agent_exec_id.to_string());
                }
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

                let supplemental_meta_root_artifact_paths =
                    bounded_meta_root_artifact_paths(run.chainworks_meta_root.as_deref());
                let supplemental_meta_root_discovery =
                    (!supplemental_meta_root_artifact_paths.root_path.is_empty()
                        || !supplemental_meta_root_artifact_paths.warnings.is_empty())
                    .then_some(supplemental_meta_root_artifact_paths.clone());
                for path in result
                    .artifact_paths
                    .iter()
                    .chain(supplemental_meta_root_artifact_paths.artifact_paths.iter())
                {
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

                let captured_declared_outputs = declared_output_settlement
                    .as_ref()
                    .map(|settlement| {
                        build_captured_outputs_from_discovery_decisions(
                            &declared_outputs,
                            &settlement.decisions,
                            &settlement.accepted_payloads,
                        )
                    })
                    .unwrap_or_default();
                let (
                    acp_cap_validation_sample_size,
                    acp_cap_validation_p90_output_bytes,
                    acp_cap_validation_p90_aggregate_bytes,
                ) = load_p053_cap_validation_metrics(&run.workspace_root);
                let discovery_diagnostics = declared_output_settlement.as_ref().map(|settlement| {
                    discovery_diagnostics_for_execution_result(
                        agent_exec_id,
                        &settlement.decisions,
                        &result.pre_prompt_expected_outputs,
                        supplemental_meta_root_discovery.clone(),
                        ExecutionDiscoveryMetrics {
                            acp_pre_initialize_local_latency_ms: result
                                .acp_pre_initialize_local_latency_ms,
                            acp_initialize_latency_ms: result.acp_initialize_latency_ms,
                            acp_session_new_latency_ms: result.acp_session_new_latency_ms,
                            acp_prompt_duration_ms: result.acp_prompt_duration_ms,
                            acp_pre_prompt_metadata_latency_ms: result
                                .acp_pre_prompt_metadata_latency_ms,
                            acp_pre_prompt_metadata_timeout: result.acp_pre_prompt_metadata_timeout,
                            acp_pre_prompt_metadata_digest_bytes: result
                                .acp_pre_prompt_metadata_digest_bytes,
                            acp_expected_output_spec_count: expected_outputs.len(),
                            acp_control_plane_manifest_latency_ms,
                            acp_git_changed_files_latency_ms,
                            acp_git_manifest_status: acp_git_manifest_status.clone(),
                            acp_exact_output_acceptance_latency_ms,
                            acp_exact_output_acceptance_timeout: false,
                            acp_exact_output_aggregate_bytes: settlement.accepted_aggregate_bytes,
                            acp_exact_output_aggregate_cap_hit: settlement.aggregate_cap_hit,
                            acp_legacy_broad_discovery_policy: legacy_broad_discovery_policy_name(
                                legacy_broad_discovery_policy.clone(),
                            ),
                            acp_legacy_broad_discovery_used: legacy_broad_discovery_policy
                                .allows_broad_discovery(),
                            acp_discovery_override_status: discovery_override_status.clone(),
                            acp_legacy_broad_discovery_truncation_reason:
                                legacy_broad_discovery_truncation_reason(
                                    result.legacy_broad_discovery_snapshot.as_ref(),
                                ),
                            acp_resume_discovery_warning: acp_resume_discovery_warning.clone(),
                            acp_cap_validation_sample_size,
                            acp_cap_validation_p90_output_bytes,
                            acp_cap_validation_p90_aggregate_bytes,
                        },
                        completed_at,
                    )
                });
                let import_result = self
                    .import_declared_contract_outputs(
                        &declared_outputs,
                        &captured_declared_outputs,
                        &persisted_artifacts,
                        &declared_artifacts,
                        stage_execution_id,
                        agent_exec_id,
                        &item.id,
                        &artifact_claim_key,
                        source_session_generation_id,
                        result.status.clone(),
                        observed_failure_classification_for_execution_result(
                            &result.status,
                            result.transcript_text.as_deref(),
                        ),
                        result.close_diagnostic.as_ref(),
                        discovery_diagnostics.as_ref(),
                        &stage_degraded_output_policy,
                        completed_at,
                    )
                    .await?;
                let validation_summary = import_result.validation_summary;
                let final_agent_status = import_result.final_agent_status;
                let _degraded_outputs_satisfy_stage = import_result.degraded_outputs_satisfy_stage;

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
                            run.chainworks_meta_root.as_deref(),
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
        let mut delivery_config = self.load_delivery_configuration(&run).await?;
        if let Some(run_target_branch) = run
            .target_branch
            .as_deref()
            .map(str::trim)
            .filter(|branch| !branch.is_empty())
        {
            if delivery_config.target_branch != run_target_branch {
                info!(
                    run_id = %run_id,
                    run_target_branch = %run_target_branch,
                    delivery_target_branch = %delivery_config.target_branch,
                    "Release target branch resolved from provisioned run worktree"
                );
            }
            delivery_config.target_branch = run_target_branch.to_string();
        }
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

    fn prepare_declared_output_artifacts(
        &self,
        declared_outputs: &[DeclaredOutput],
        discovery_decisions: Option<&[OutputDiscoveryDecision]>,
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
            if declared_output_has_accepted_discovery_decision(
                discovery_decisions,
                &declared.output_name,
                domain::discovery::ExpectedOutputRole::Machine,
            ) {
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
            }

            if let (Some(companion_name), Some(companion_path), Some(schema)) = (
                declared.companion_output_name.as_deref(),
                declared.companion_path.as_deref(),
                declared.schema.as_ref(),
            ) {
                if declared_output_has_accepted_discovery_decision(
                    discovery_decisions,
                    companion_name,
                    domain::discovery::ExpectedOutputRole::Companion,
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
        }
        Ok(artifacts_out)
    }

    async fn import_declared_contract_outputs(
        &self,
        declared_outputs: &[DeclaredOutput],
        captured_outputs: &[CapturedOutput],
        persisted_artifacts: &[Artifact],
        declared_artifacts_to_insert: &[Artifact],
        stage_execution_id: domain::ids::StageExecutionId,
        agent_exec_id: domain::ids::AgentExecutionId,
        work_item_id: &str,
        artifact_claim_key: &ArtifactSourceGenerationClaimKey,
        session_generation_id: Option<&str>,
        result_status: AgentStatus,
        observed_failure_classification: Option<RuntimeFailureClassification>,
        close_diagnostic: Option<&acp::AcpCloseDiagnostic>,
        discovery_diagnostics: Option<&AgentExecutionDiscoveryDiagnostics>,
        stage_degraded_output_policy: &workflow::plan::DegradedOutputPolicy,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<DeclaredContractImportResult> {
        let validation_summary = if declared_outputs.is_empty() {
            None
        } else {
            Some(validate_task_outputs(captured_outputs))
        };
        let validation_failed = validation_summary
            .as_ref()
            .and_then(|summary| summary.failure_class.as_ref())
            .is_some();
        let mut runtime_facts = runtime_facts_for_execution_result(
            agent_exec_id,
            result_status.clone(),
            validation_summary.as_ref(),
            observed_failure_classification,
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
        if let Some(discovery_diagnostics) = discovery_diagnostics {
            agent_execution_discovery_diagnostics::upsert_tx(&mut tx, discovery_diagnostics)
                .await?;
        }
        if runtime_facts.failure_kind == Some(AgentFailureKind::ProviderQuota) {
            let ledger = agent_retry_budget_ledger::upsert_quota_failure_for_owner_tx(
                &mut tx,
                artifact_claim_key.run_id,
                artifact_claim_key.owner_kind.clone(),
                artifact_claim_key.owner_id.clone(),
                artifact_claim_key.stage_execution_id,
                agent_exec_id,
                runtime_facts.retry_after,
            )
            .await?;
            if artifact_claim_key.owner_kind == domain::mediation::OwnerKind::LeadConflictMediation
                && ledger.normal_budget_consumed
            {
                workflow_conflicts::record_mediation_retry_budget_exhausted_tx(
                    &mut tx,
                    &artifact_claim_key.run_id.to_string(),
                    &artifact_claim_key.owner_id,
                    None,
                    "provider_quota",
                    chrono::Utc::now(),
                )
                .await?;
            }
            runtime_facts.quota_ledger_id = Some(ledger.id);
        }
        agent_execution_runtime_facts::upsert_tx(&mut tx, &runtime_facts).await?;
        artifact_contracts::close_source_generation_claim_tx(&mut tx, artifact_claim_key).await?;
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
                agent_execution_id: None,
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
        let artifact = self
            .build_transcript_artifact_if_present(
                run,
                stage_id,
                agent_id,
                provider,
                model,
                agent_execution_id,
                created_at,
                transcript_text,
            )
            .await?;
        if let Some(artifact) = artifact.as_ref() {
            artifacts::insert(&self.pool, artifact).await?;
            let _ = self
                .events
                .send(domain::events::DomainEvent::ArtifactCreated {
                    run_id: run.id,
                    artifact_id: artifact.id,
                });
        }
        Ok(artifact)
    }

    async fn build_transcript_artifact_if_present(
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
            // P017 R5 / API-003: stamp the direct execution-attempt FK
            // so MCP/GraphQL `execution_attempts.artifacts` can attribute
            // this transcript to the exact attempt that produced it
            // (cross-retry isolation).
            agent_execution_id: Some(agent_execution_id.to_string()),
        };
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
            None,
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
        source_stage_execution_id: Option<domain::ids::StageExecutionId>,
        source_agent_execution_id: Option<domain::ids::AgentExecutionId>,
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
            agent_execution_id: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;
        if domain::artifact_contracts::known_contract_id(contract_id) {
            artifact_contracts::upsert_generation_and_rebuild(
                &self.pool,
                domain::artifact_contracts::ActiveArtifactGenerationInput {
                    run_id,
                    artifact_id: artifact.id,
                    contract_id: contract_id.to_string(),
                    canonical_path: path.to_string(),
                    raw_path: path.to_string(),
                    raw_status: raw_status_from_artifact_path(path),
                    generation_id: artifact.id.to_string(),
                    source_agent_execution_id: source_agent_execution_id.map(|id| id.to_string()),
                    source_stage_execution_id: source_stage_execution_id.map(|id| id.to_string()),
                    source_session_generation_id: None,
                    source_work_item_id: None,
                    supersedes_generation_id: None,
                    output_settlement: domain::agent::AgentOutputSettlement::None,
                    partial: false,
                    warnings: Vec::new(),
                },
            )
            .await?;
        }
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
            agent_execution_id: None,
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
            agent_execution_id: None,
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
            agent_execution_id: None,
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
            agent_execution_id: None,
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
    let search_dirs = [artifact_root.to_string(), run_dir];

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

struct RuntimeInvocationContractInput<'a> {
    run_id: String,
    stage_id: String,
    stage_execution_id: String,
    agent_execution_id: String,
    work_item_id: String,
    session_generation_id: Option<String>,
    session_reuse_disposition: Option<String>,
    declared_outputs: &'a [DeclaredOutput],
}

fn prompt_with_runtime_invocation_contract(
    mut prompt: String,
    input: RuntimeInvocationContractInput<'_>,
) -> String {
    prompt.push_str("\n\n### Runtime Invocation Contract\n");
    prompt.push_str(
        "This block is authoritative for the current turn. The provider session may be reused, \
         but prior session memory must not override these runtime identifiers or output paths.\n",
    );
    prompt.push_str(&format!("- Run id: `{}`\n", input.run_id));
    prompt.push_str(&format!("- Stage id: `{}`\n", input.stage_id));
    prompt.push_str(&format!(
        "- Stage execution id: `{}`\n",
        input.stage_execution_id
    ));
    prompt.push_str(&format!(
        "- Agent execution id: `{}`\n",
        input.agent_execution_id
    ));
    prompt.push_str(&format!("- Work item id: `{}`\n", input.work_item_id));
    if let Some(session_generation_id) = input.session_generation_id.as_deref() {
        prompt.push_str(&format!(
            "- Session generation id: `{session_generation_id}`\n"
        ));
    }
    if let Some(disposition) = input.session_reuse_disposition.as_deref() {
        prompt.push_str(&format!("- Session reuse disposition: `{disposition}`\n"));
    }

    if input.declared_outputs.is_empty() {
        prompt.push_str("- Declared required outputs: none.\n");
        return prompt;
    }

    prompt.push_str("\nRequired outputs for this turn:\n");
    for output in input.declared_outputs {
        prompt.push_str(&format!(
            "- `{}` -> `{}`\n",
            output.output_name, output.target_path
        ));
        append_status_allowed_values_for_declared_output(&mut prompt, output);
        if let (Some(name), Some(path)) = (
            output.companion_output_name.as_deref(),
            output.companion_path.as_deref(),
        ) {
            prompt.push_str(&format!("- `{name}` companion -> `{path}`\n"));
        }
    }
    prompt.push_str(
        "\nOutputs from prior stage executions, prior work items, or prior prompt turns are stale \
         unless they are explicitly accepted by the current output contract. Return a fresh \
         `CHAINWORKS_OUTPUT` object for this invocation, using the exact canonical paths above as \
         keys. You must not finish this turn without `CHAINWORKS_OUTPUT` when required outputs are \
         listed.\n",
    );
    append_docs_noop_contract_guidance(&mut prompt, input.declared_outputs);
    prompt
}

fn append_status_allowed_values_for_declared_output(prompt: &mut String, output: &DeclaredOutput) {
    let Some(schema) = output.schema.as_ref() else {
        return;
    };
    if !schema.required_fields.iter().any(|field| field == "status") {
        return;
    }
    let Some(allowed_values) = contract_status_allowed_values(&schema.contract_id) else {
        return;
    };
    prompt.push_str(&format!(
        "  Allowed values for `status`: {}.\n",
        allowed_values
            .iter()
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

fn append_docs_noop_contract_guidance(prompt: &mut String, declared_outputs: &[DeclaredOutput]) {
    let has_docs_report = declared_outputs
        .iter()
        .any(|output| output.output_name == "docs_report");
    let has_docs_delta = declared_outputs
        .iter()
        .any(|output| output.output_name == "docs_delta");
    if !(has_docs_report || has_docs_delta) {
        return;
    }

    prompt.push_str(
        "\nDocumentation no-op is a valid structured result. If documentation is already aligned, \
         still emit the required outputs instead of omitting them:\n",
    );
    if has_docs_report {
        prompt.push_str(
            "<<<CHAINWORKS_OUTPUT:docs_report>>>{\"status\":\"not_needed\",\"changed_docs\":[],\"missing_docs\":[],\"followups\":[]}<<<END_CHAINWORKS_OUTPUT>>>\n",
        );
    }
    if has_docs_delta {
        prompt.push_str(
            "<<<CHAINWORKS_OUTPUT:docs_delta>>>{\"files\":[],\"summary\":\"No documentation changes required.\"}<<<END_CHAINWORKS_OUTPUT>>>\n",
        );
    }
}

fn validation_summary_requires_output_contract_repair(summary: &TaskValidationSummary) -> bool {
    matches!(
        summary.failure_class,
        Some(
            domain::validation::ValidationFailureClass::NoOutputProduced
                | domain::validation::ValidationFailureClass::EmptyOutput
                | domain::validation::ValidationFailureClass::OutputContractMismatch
        )
    )
}

fn output_contract_repair_prompt(
    validation: &TaskValidationSummary,
    declared_outputs: &[DeclaredOutput],
) -> String {
    let mut prompt = String::new();
    prompt.push_str("### Output Contract Repair\n");
    prompt.push_str(
        "The previous response did not satisfy the required output contract. Do not redo unrelated implementation work. Return only corrected `CHAINWORKS_OUTPUT` blocks for the outputs listed below.\n",
    );
    if let Some(summary) = validation.failure_summary.as_deref() {
        prompt.push_str(&format!("- Validation failure: {summary}\n"));
    }
    for result in &validation.output_results {
        if result.status == domain::validation::ValidationStatus::Passed {
            continue;
        }
        prompt.push_str(&format!("- `{}` failed", result.output_name));
        if let Some(contract_id) = result.contract_id.as_deref() {
            prompt.push_str(&format!(" contract `{contract_id}`"));
        }
        if !result.missing_fields.is_empty() {
            prompt.push_str(&format!(
                "; missing fields: {}",
                result.missing_fields.join(", ")
            ));
        }
        if let Some(error) = result.validation_error.as_deref() {
            prompt.push_str(&format!("; error: {error}"));
        }
        prompt.push('\n');
    }
    prompt.push_str("\nRequired corrected output envelopes:\n");
    for output in declared_outputs {
        prompt.push_str(&format!("<<<CHAINWORKS_OUTPUT:{}>>>\n", output.output_name));
        append_status_allowed_values_for_declared_output(&mut prompt, output);
        prompt.push_str("{ /* valid JSON matching the declared contract */ }\n");
        prompt.push_str("<<<END_CHAINWORKS_OUTPUT>>>\n");
    }
    append_docs_noop_contract_guidance(&mut prompt, declared_outputs);
    prompt
}

fn merge_contract_repair_result(initial: &mut acp::ExecutionResult, repair: acp::ExecutionResult) {
    initial.status = repair.status;
    initial.artifact_paths.extend(repair.artifact_paths);
    initial.discovered_artifacts = repair.discovered_artifacts;
    initial.pre_prompt_expected_outputs = repair.pre_prompt_expected_outputs;
    initial.cost_cents =
        Some(initial.cost_cents.unwrap_or_default() + repair.cost_cents.unwrap_or_default());
    initial.usage = repair.usage.or_else(|| initial.usage.clone());
    initial.provider_session_id = repair
        .provider_session_id
        .or(initial.provider_session_id.clone());
    initial.reused_existing_session = repair.reused_existing_session;
    initial.session_generation_id = repair
        .session_generation_id
        .or(initial.session_generation_id.clone());
    initial.mcp_observation = repair
        .mcp_observation
        .or_else(|| initial.mcp_observation.clone());
    if !repair.actual_mcp_extensions.is_empty() {
        initial.actual_mcp_extensions = repair.actual_mcp_extensions;
    }
    if !repair.actual_mcp_runtime_ids.is_empty() {
        initial.actual_mcp_runtime_ids = repair.actual_mcp_runtime_ids;
    }
    initial.mcp_session_startup_latency_ms = repair
        .mcp_session_startup_latency_ms
        .or(initial.mcp_session_startup_latency_ms);
    initial
        .xcode_shim_warning_events
        .extend(repair.xcode_shim_warning_events);
    initial.close_diagnostic = repair.close_diagnostic.or(initial.close_diagnostic.take());
    initial.acp_prompt_duration_ms = repair
        .acp_prompt_duration_ms
        .or(initial.acp_prompt_duration_ms);
    initial.acp_pre_prompt_metadata_latency_ms = repair
        .acp_pre_prompt_metadata_latency_ms
        .or(initial.acp_pre_prompt_metadata_latency_ms);
    initial.acp_pre_prompt_metadata_timeout |= repair.acp_pre_prompt_metadata_timeout;
    initial.acp_pre_prompt_metadata_digest_bytes += repair.acp_pre_prompt_metadata_digest_bytes;
    initial.legacy_broad_discovery_snapshot = repair
        .legacy_broad_discovery_snapshot
        .or_else(|| initial.legacy_broad_discovery_snapshot.clone());
    initial.transcript_text = match (initial.transcript_text.take(), repair.transcript_text) {
        (Some(initial_text), Some(repair_text)) => Some(format!(
            "{initial_text}\n\n--- output contract repair turn ---\n{repair_text}"
        )),
        (None, Some(repair_text)) => Some(repair_text),
        (Some(initial_text), None) => Some(initial_text),
        (None, None) => None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_invocation_contract_makes_reused_turn_self_contained() {
        let run_id = RunId::new();
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        let declared_outputs = vec![DeclaredOutput {
            output_name: "proposal_current".to_string(),
            target_path: "/workspace/.chainworks/runs/run-1/proposals/current.md".to_string(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        }];

        let prompt = prompt_with_runtime_invocation_contract(
            "Base stable prompt".to_string(),
            RuntimeInvocationContractInput {
                run_id: run_id.to_string(),
                stage_id: "state_5_proposal_refined".to_string(),
                stage_execution_id: stage_execution_id.to_string(),
                agent_execution_id: agent_execution_id.to_string(),
                work_item_id: "p058-invoke:stage:0".to_string(),
                session_generation_id: Some("generation-1".to_string()),
                session_reuse_disposition: Some("reused".to_string()),
                declared_outputs: &declared_outputs,
            },
        );

        assert!(prompt.contains("Base stable prompt"));
        assert!(prompt.contains("### Runtime Invocation Contract"));
        assert!(prompt.contains("provider session may be reused"));
        assert!(prompt.contains(&stage_execution_id.to_string()));
        assert!(prompt.contains("p058-invoke:stage:0"));
        assert!(prompt.contains("proposal_current"));
        assert!(prompt.contains("/workspace/.chainworks/runs/run-1/proposals/current.md"));
        assert!(prompt.contains("stale"));
        assert!(prompt.contains("CHAINWORKS_OUTPUT"));
        assert!(prompt.contains("must not finish this turn without"));
    }

    #[test]
    fn runtime_invocation_contract_spells_out_docs_noop_outputs() {
        let declared_outputs = vec![
            DeclaredOutput {
                output_name: "docs_report".to_string(),
                target_path: "/workspace/.chainworks/docs/report.json".to_string(),
                schema: None,
                reuse_policy: None,
                companion_output_name: None,
                companion_path: None,
            },
            DeclaredOutput {
                output_name: "docs_delta".to_string(),
                target_path: "/workspace/.chainworks/docs/changed-files.json".to_string(),
                schema: None,
                reuse_policy: None,
                companion_output_name: None,
                companion_path: None,
            },
        ];

        let prompt = prompt_with_runtime_invocation_contract(
            "Review docs".to_string(),
            RuntimeInvocationContractInput {
                run_id: RunId::new().to_string(),
                stage_id: "state_9_implementation_reviewed".to_string(),
                stage_execution_id: domain::ids::StageExecutionId::new().to_string(),
                agent_execution_id: domain::ids::AgentExecutionId::new().to_string(),
                work_item_id: "work-docs".to_string(),
                session_generation_id: None,
                session_reuse_disposition: None,
                declared_outputs: &declared_outputs,
            },
        );

        assert!(prompt.contains("\"status\":\"not_needed\""));
        assert!(prompt.contains("<<<CHAINWORKS_OUTPUT:docs_report>>>"));
        assert!(prompt.contains("<<<CHAINWORKS_OUTPUT:docs_delta>>>"));
        assert!(prompt.contains("\"files\":[]"));
    }

    #[test]
    fn output_contract_repair_prompt_names_missing_outputs_and_exact_envelopes() {
        let declared_outputs = vec![DeclaredOutput {
            output_name: "implementation_review_summary".to_string(),
            target_path: "/workspace/.chainworks/review/implementation-summary.json".to_string(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        }];
        let validation = TaskValidationSummary {
            output_results: vec![domain::validation::OutputValidationResult {
                output_name: "implementation_review_summary".to_string(),
                contract_id: Some("implementation_review_summary_v1".to_string()),
                status: domain::validation::ValidationStatus::Failed,
                missing_fields: vec!["status".to_string()],
                validation_error: Some("required output was not produced".to_string()),
                raw_payload_size: 0,
            }],
            contract_metadata: vec![],
            raw_output_exists: false,
            failure_class: Some(domain::validation::ValidationFailureClass::NoOutputProduced),
            failure_summary: Some(
                "implementation_review_summary: required output was not produced".to_string(),
            ),
        };

        let prompt = output_contract_repair_prompt(&validation, &declared_outputs);

        assert!(validation_summary_requires_output_contract_repair(
            &validation
        ));
        assert!(prompt.contains("implementation_review_summary"));
        assert!(prompt.contains("required output was not produced"));
        assert!(prompt.contains("<<<CHAINWORKS_OUTPUT:implementation_review_summary>>>"));
        assert!(prompt.contains("Do not redo unrelated implementation work"));
    }

    #[test]
    fn runtime_invocation_contract_does_not_mutate_session_fingerprint_prompt() {
        let base_prompt = "Base stable prompt".to_string();
        let declared_outputs = Vec::new();
        let prompt = prompt_with_runtime_invocation_contract(
            base_prompt.clone(),
            RuntimeInvocationContractInput {
                run_id: "run-1".to_string(),
                stage_id: "state_5".to_string(),
                stage_execution_id: "stage-exec-1".to_string(),
                agent_execution_id: "agent-exec-1".to_string(),
                work_item_id: "p058-invoke:stage-exec-1:0".to_string(),
                session_generation_id: Some("generation-1".to_string()),
                session_reuse_disposition: Some("reused".to_string()),
                declared_outputs: &declared_outputs,
            },
        );

        let stable_fingerprint = binding_fingerprint(&BindingFingerprintInput {
            agent_id: "proposal_writer",
            provider: "codex",
            model: Some("gpt-5.4"),
            effort: None,
            prompt: &base_prompt,
            working_directory: "/workspace",
            workspace_mode: "write_enabled",
            worktree_write_enabled: true,
            worktree_strategy: Some("meta_only"),
            inputs: &[],
            outputs: &["proposal_current".to_string()],
            backend_profile: Some("codex_writer_high"),
            permission_profile: Some("PROPOSAL_WRITE"),
            mcp_servers: &[],
            xcode_broker_contract_hash: None,
            xcode_broker_required: false,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            skill_snapshot_hash: None,
            skill_ref: Some("proposal_writer_core"),
            skill_role: None,
            output_contract: None,
            max_turns: None,
            temperature: None,
        });
        let volatile_fingerprint = binding_fingerprint(&BindingFingerprintInput {
            prompt: &prompt,
            ..BindingFingerprintInput {
                agent_id: "proposal_writer",
                provider: "codex",
                model: Some("gpt-5.4"),
                effort: None,
                prompt: &base_prompt,
                working_directory: "/workspace",
                workspace_mode: "write_enabled",
                worktree_write_enabled: true,
                worktree_strategy: Some("meta_only"),
                inputs: &[],
                outputs: &["proposal_current".to_string()],
                backend_profile: Some("codex_writer_high"),
                permission_profile: Some("PROPOSAL_WRITE"),
                mcp_servers: &[],
                xcode_broker_contract_hash: None,
                xcode_broker_required: false,
                xcode_shim_injection_signal: false,
                requires_xcode_host_execution: false,
                skill_snapshot_hash: None,
                skill_ref: Some("proposal_writer_core"),
                skill_role: None,
                output_contract: None,
                max_turns: None,
                temperature: None,
            }
        });

        assert_ne!(
            stable_fingerprint, volatile_fingerprint,
            "including runtime ids in the session fingerprint would force fresh sessions"
        );
    }

    #[test]
    fn p051_runtime_guard_suppresses_interactive_xcode_mcp_for_review_invocations() {
        assert!(suppress_interactive_review_xcode_mcp_for_invocation(
            "proposal_implementation_auditor",
            Some("codex_audit_high"),
            Some("RO_VERIFY")
        ));
        assert!(suppress_interactive_review_xcode_mcp_for_invocation(
            "prepush_code_reviewer",
            Some("claude_prepush_medium"),
            Some("RO_PREPUSH_VERIFY")
        ));
        assert!(suppress_interactive_review_xcode_mcp_for_invocation(
            "proposal_writer",
            Some("codex_writer_high"),
            Some("PROPOSAL_WRITE")
        ));
        assert!(!suppress_interactive_review_xcode_mcp_for_invocation(
            "code_writer",
            Some("codex_writer_high"),
            Some("WRITE")
        ));
    }

    #[test]
    fn proposal_053_bounded_meta_root_artifact_paths_are_supplemental_only() {
        let tmp = tempfile::tempdir().unwrap();
        let meta_root = tmp.path().join("run-meta");
        let target_dir = meta_root.join("target");
        let logs_dir = meta_root.join("logs");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::create_dir_all(&logs_dir).unwrap();
        let log_path = logs_dir.join("agent.log");
        let build_path = target_dir.join("build.log");
        std::fs::write(&log_path, b"operator-visible log").unwrap();
        std::fs::write(&build_path, b"generated build output").unwrap();

        let paths = bounded_meta_root_artifact_paths(Some(meta_root.to_str().unwrap()));

        assert_eq!(paths.artifact_paths.len(), 1);
        assert!(paths.artifact_paths[0].ends_with("logs/agent.log"));
    }

    #[test]
    fn proposal_053_engine_settlement_uses_discovery_filesystem_fake_for_exact_path() {
        let target_path = "/workspace/run/proposal_review.json";
        let root_path = "/workspace/run";
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: target_path.to_string(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(&[declared], root_path, None, None, false);
        specs[0].authorized_roots = vec![domain::discovery::AuthorizedRoot {
            root_class: OutputRootClass::Worktree,
            root_path: root_path.to_string(),
        }];
        let fake = domain::discovery::FakeDiscoveryFilesystem::new()
            .with_canonical_path(target_path, PathBuf::from(target_path))
            .with_canonical_path(root_path, PathBuf::from(root_path))
            .with_path_metadata(
                target_path,
                domain::discovery::DiscoveryPathMetadata {
                    kind: domain::discovery::DiscoveryPathKind::RegularFile,
                    size_bytes: 34,
                },
            );
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"summary":"new","status":"green"}"#.to_vec(),
            source_path: Some(target_path.to_string()),
            source_kind: acp::DiscoveredArtifactSourceKind::ExactPath,
        }];

        let settlement = build_declared_output_discovery_settlement_with_filesystem(
            &specs,
            &discovered,
            &[],
            &fake,
        );

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Accepted
        );
        assert_eq!(
            settlement.decisions[0].canonical_path.as_deref(),
            Some(target_path)
        );
    }

    #[test]
    fn proposal_053_engine_stale_detection_uses_discovery_filesystem_fake() {
        let target_path = "/workspace/run/proposal_review.json";
        let root_path = "/workspace/run";
        let bytes = br#"{"summary":"stale","status":"green"}"#.to_vec();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: target_path.to_string(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(&[declared], root_path, None, None, false);
        specs[0].authorized_roots = vec![domain::discovery::AuthorizedRoot {
            root_class: OutputRootClass::Worktree,
            root_path: root_path.to_string(),
        }];
        let metadata_context = domain::discovery::PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "prompt-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };
        let mut baseline = domain::discovery::PrePromptExpectedOutputMetadata::absent(
            &specs[0],
            &metadata_context,
        );
        baseline.baseline_status = ExpectedPathBaselineStatus::RegularContentCaptured;
        baseline.existed = true;
        baseline.file_type = "regular".to_string();
        baseline.size_bytes = Some(bytes.len() as u64);
        baseline.content_digest = Some(sha256_digest(&bytes));
        let fake = domain::discovery::FakeDiscoveryFilesystem::new()
            .with_canonical_path(target_path, PathBuf::from(target_path))
            .with_canonical_path(root_path, PathBuf::from(root_path))
            .with_path_metadata(
                target_path,
                domain::discovery::DiscoveryPathMetadata {
                    kind: domain::discovery::DiscoveryPathKind::RegularFile,
                    size_bytes: bytes.len() as u64,
                },
            )
            .with_file_bytes(target_path, bytes);

        let settlement = build_declared_output_discovery_settlement_with_filesystem(
            &specs,
            &[],
            &[baseline],
            &fake,
        );

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Missing
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::StaleExpectedOutput
        );
    }

    #[test]
    fn proposal_053_bounded_meta_root_uses_discovery_filesystem_fake() {
        let fake = domain::discovery::FakeDiscoveryFilesystem::new()
            .with_bounded_meta_root_discovery(
                "/workspace/.chainworks/runs/run-1",
                domain::discovery::BoundedMetaRootDiscovery {
                    root_path: "/workspace/.chainworks/runs/run-1".to_string(),
                    artifact_paths: vec!["logs/agent.log".to_string()],
                    files_visited: 1,
                    total_bytes: 128,
                    latency_ms: None,
                    truncated_by_file_cap: false,
                    truncated_by_file_size: false,
                    truncated_by_total_bytes: false,
                    warnings: Vec::new(),
                },
            );

        let discovery = bounded_meta_root_artifact_paths_with_filesystem(
            Some("/workspace/.chainworks/runs/run-1"),
            &fake,
        );

        assert_eq!(discovery.artifact_paths, vec!["logs/agent.log"]);
    }

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
            Some(classify_observation(
                crate::failure_classifier::RuntimeFailureObservation::ProviderQuota {
                    retry_after: None,
                },
            )),
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
    fn provider_quota_observation_overrides_no_output_validation_failure() {
        let retry_after = chrono::DateTime::parse_from_rfc3339("2026-04-26T19:45:06Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let validation = TaskValidationSummary {
            output_results: vec![],
            contract_metadata: vec![],
            raw_output_exists: false,
            failure_class: Some(domain::validation::ValidationFailureClass::NoOutputProduced),
            failure_summary: Some("required output was not produced".into()),
        };

        let facts = runtime_facts_for_execution_result(
            domain::ids::AgentExecutionId::new(),
            AgentStatus::Failed,
            Some(&validation),
            Some(classify_observation(
                crate::failure_classifier::RuntimeFailureObservation::ProviderQuota {
                    retry_after: Some(retry_after),
                },
            )),
            chrono::Utc::now(),
            None,
        );

        assert_eq!(facts.failure_kind, Some(AgentFailureKind::ProviderQuota));
        assert_eq!(
            facts.operator_action_hint,
            Some(OperatorActionHint::WaitUntilRetryAfter)
        );
        assert_eq!(facts.retry_after, Some(retry_after));
        assert_eq!(
            facts.output_settlement,
            AgentOutputSettlement::MissingRequiredOutputs
        );
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
            reuse_policy: None,
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some(companion_path.to_string_lossy().into_owned()),
        };
        let discovered = vec![
            acp::DiscoveredArtifact {
                name: "proposal_review".to_string(),
                content: br#"{"status":"green"}"#.to_vec(),
                source_path: None,
                source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
            },
            acp::DiscoveredArtifact {
                name: "proposal_review_raw".to_string(),
                content: b"# Review\n".to_vec(),
                source_path: None,
                source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
            },
        ];
        let specs = build_expected_output_specs(
            &[declared.clone()],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        settle_agent_outputs_from_discovery_decisions(&[declared], &specs, &discovered, &[])
            .expect("accepted envelope-derived outputs should be materialized to canonical paths");

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
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let discovered = vec![acp::DiscoveredArtifact {
            name: machine_path.to_string_lossy().into_owned(),
            content: br#"{"seemingly_complete":true}"#.to_vec(),
            source_path: None,
            source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
        }];
        let specs = build_expected_output_specs(
            &[declared.clone()],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        settle_agent_outputs_from_discovery_decisions(&[declared], &specs, &discovered, &[])
            .expect(
                "accepted path-keyed JSON envelope outputs should materialize to canonical paths",
            );

        assert_eq!(
            std::fs::read_to_string(machine_path).unwrap(),
            r#"{"seemingly_complete":true}"#
        );
    }

    #[test]
    fn proposal_053_settlement_boundary_records_idempotency_key() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("proposal_review.json");
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let specs = build_expected_output_specs(
            &[declared.clone()],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"status":"green"}"#.to_vec(),
            source_path: None,
            source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
        }];
        let pre_prompt_metadata = vec![PrePromptExpectedOutputMetadata {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            canonical_path: None,
            root_class: OutputRootClass::Workspace,
            existed: false,
            file_type: "absent".to_string(),
            size_bytes: None,
            content_digest: None,
            mtime_ns: None,
            baseline_status: ExpectedPathBaselineStatus::Absent,
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 2,
            session_generation_id: "session-gen-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-gen-1".to_string(),
        }];

        let settlement = settle_agent_outputs_from_discovery_decisions(
            &[declared],
            &specs,
            &discovered,
            &pre_prompt_metadata,
        )
        .expect("named settlement boundary should accept provider payload");

        assert_eq!(
            settlement.idempotency_key.as_deref(),
            Some("agent-exec-1:discovery-gen-1")
        );
        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Accepted
        );
        assert_eq!(
            std::fs::read_to_string(machine_path).unwrap(),
            r#"{"status":"green"}"#
        );
    }

    #[test]
    fn proposal_053_oversized_provider_payload_does_not_validate_stale_target_path() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("proposal_review.json");
        std::fs::write(&machine_path, br#"{"summary":"stale","status":"green"}"#).unwrap();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(
            &[declared.clone()],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        specs[0].max_bytes = 8;
        specs[0].aggregate_acceptance_cap_bytes = 64;
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"summary":"new","status":"green"}"#.to_vec(),
            source_path: None,
            source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
        }];

        let settlement = build_declared_output_discovery_settlement(&specs, &discovered, &[]);
        let captured = build_captured_outputs_from_discovery_decisions(
            &[declared],
            &settlement.decisions,
            &settlement.accepted_payloads,
        );
        let validation = validate_task_outputs(&captured);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Rejected
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::ProviderEnvelopeOversized
        );
        assert!(!validation.raw_output_exists);
        assert_eq!(
            validation.failure_class,
            Some(domain::validation::ValidationFailureClass::NoOutputProduced)
        );
    }

    #[test]
    fn proposal_053_oversized_chainworks_output_uses_specific_rejection_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("proposal_review.json");
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(
            &[declared],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        specs[0].max_bytes = 8;
        specs[0].aggregate_acceptance_cap_bytes = 64;
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"summary":"new","status":"green"}"#.to_vec(),
            source_path: None,
            source_kind: acp::DiscoveredArtifactSourceKind::ChainworksOutput,
        }];

        let settlement = build_declared_output_discovery_settlement(&specs, &discovered, &[]);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Rejected
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::ChainworksOutputOversized
        );
    }

    #[test]
    fn proposal_053_aggregate_cap_rejects_later_declared_outputs() {
        let tmp = tempfile::tempdir().unwrap();
        let first_path = tmp.path().join("first.json");
        let second_path = tmp.path().join("second.json");
        let declared = vec![
            DeclaredOutput {
                output_name: "first".to_string(),
                target_path: first_path.to_string_lossy().into_owned(),
                schema: None,
                reuse_policy: None,
                companion_output_name: None,
                companion_path: None,
            },
            DeclaredOutput {
                output_name: "second".to_string(),
                target_path: second_path.to_string_lossy().into_owned(),
                schema: None,
                reuse_policy: None,
                companion_output_name: None,
                companion_path: None,
            },
        ];
        let mut specs =
            build_expected_output_specs(&declared, tmp.path().to_str().unwrap(), None, None, false);
        for spec in &mut specs {
            spec.max_bytes = 64;
            spec.aggregate_acceptance_cap_bytes = 10;
        }
        let discovered = vec![
            acp::DiscoveredArtifact {
                name: "first".to_string(),
                content: b"12345".to_vec(),
                source_path: None,
                source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
            },
            acp::DiscoveredArtifact {
                name: "second".to_string(),
                content: b"123456".to_vec(),
                source_path: None,
                source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
            },
        ];

        let settlement = build_declared_output_discovery_settlement(&specs, &discovered, &[]);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Accepted
        );
        assert_eq!(
            settlement.decisions[1].status,
            OutputDiscoveryStatus::Rejected
        );
        assert_eq!(
            settlement.decisions[1].reason,
            OutputDiscoveryReason::AggregateExactOutputCap
        );
        assert!(settlement
            .accepted_payloads
            .contains_key(&provider_envelope_payload_ref("first")));
        assert!(!settlement
            .accepted_payloads
            .contains_key(&provider_envelope_payload_ref("second")));
    }

    #[test]
    fn proposal_053_allow_unchanged_existing_accepts_declared_reuse_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("proposal_review.json");
        let content = br#"{"summary":"reused","status":"green"}"#;
        std::fs::write(&machine_path, content).unwrap();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: Some(OutputReusePolicy::AllowUnchangedExisting),
            companion_output_name: None,
            companion_path: None,
        };
        let specs = build_expected_output_specs(
            &[declared],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        let metadata_context = domain::discovery::PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "prompt-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };
        let pre_prompt_metadata = vec![
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata(
                &specs[0],
                &metadata_context,
            ),
        ];

        let settlement =
            build_declared_output_discovery_settlement(&specs, &[], &pre_prompt_metadata);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Accepted
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::DeclaredReusePolicy
        );
        assert_eq!(
            settlement.decisions[0].provenance,
            Some(OutputDiscoveryProvenance::DeclaredReusePolicy)
        );
        assert!(settlement
            .accepted_payloads
            .contains_key(&declared_reuse_policy_payload_ref("proposal_review")));
    }

    #[test]
    fn proposal_053_must_produce_does_not_accept_unchanged_existing_output() {
        let tmp = tempfile::tempdir().unwrap();
        let machine_path = tmp.path().join("proposal_review.json");
        std::fs::write(&machine_path, br#"{"summary":"stale","status":"green"}"#).unwrap();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let specs = build_expected_output_specs(
            &[declared],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        let metadata_context = domain::discovery::PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "prompt-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };
        let pre_prompt_metadata = vec![
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata(
                &specs[0],
                &metadata_context,
            ),
        ];

        let settlement =
            build_declared_output_discovery_settlement(&specs, &[], &pre_prompt_metadata);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Missing
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::StaleExpectedOutput
        );
        assert_eq!(
            settlement.decisions[0].baseline_status,
            Some(ExpectedPathBaselineStatus::RegularContentCaptured)
        );
        assert!(settlement.accepted_payloads.is_empty());
    }

    #[test]
    fn proposal_053_exact_path_rejects_unauthorized_root() {
        let tmp = tempfile::tempdir().unwrap();
        let allowed_root = tmp.path().join("allowed");
        let disallowed_root = tmp.path().join("disallowed");
        std::fs::create_dir_all(&allowed_root).unwrap();
        std::fs::create_dir_all(&disallowed_root).unwrap();
        let machine_path = disallowed_root.join("proposal_review.json");
        std::fs::write(&machine_path, br#"{"summary":"new","status":"green"}"#).unwrap();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: machine_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(
            &[declared],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        specs[0].authorized_roots = vec![domain::discovery::AuthorizedRoot {
            root_class: OutputRootClass::Worktree,
            root_path: allowed_root.to_string_lossy().into_owned(),
        }];
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"summary":"new","status":"green"}"#.to_vec(),
            source_path: Some(machine_path.to_string_lossy().into_owned()),
            source_kind: acp::DiscoveredArtifactSourceKind::ExactPath,
        }];

        let settlement = build_declared_output_discovery_settlement(&specs, &discovered, &[]);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Rejected
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::UnauthorizedRoot
        );
        assert!(settlement.accepted_payloads.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn proposal_053_exact_path_rejects_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let allowed_root = tmp.path().join("allowed");
        let outside_root = tmp.path().join("outside");
        std::fs::create_dir_all(&allowed_root).unwrap();
        std::fs::create_dir_all(&outside_root).unwrap();
        let outside_file = outside_root.join("proposal_review.json");
        std::fs::write(&outside_file, br#"{"summary":"new","status":"green"}"#).unwrap();
        let symlink_path = allowed_root.join("proposal_review.json");
        std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: symlink_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let mut specs = build_expected_output_specs(
            &[declared],
            tmp.path().to_str().unwrap(),
            None,
            None,
            false,
        );
        specs[0].authorized_roots = vec![domain::discovery::AuthorizedRoot {
            root_class: OutputRootClass::Worktree,
            root_path: allowed_root.to_string_lossy().into_owned(),
        }];
        let discovered = vec![acp::DiscoveredArtifact {
            name: "proposal_review".to_string(),
            content: br#"{"summary":"new","status":"green"}"#.to_vec(),
            source_path: Some(symlink_path.to_string_lossy().into_owned()),
            source_kind: acp::DiscoveredArtifactSourceKind::ExactPath,
        }];

        let settlement = build_declared_output_discovery_settlement(&specs, &discovered, &[]);

        assert_eq!(
            settlement.decisions[0].status,
            OutputDiscoveryStatus::Rejected
        );
        assert_eq!(
            settlement.decisions[0].reason,
            OutputDiscoveryReason::SymlinkEscape
        );
        assert!(settlement.accepted_payloads.is_empty());
    }

    #[test]
    fn proposal_053_declared_artifact_persistence_requires_accepted_decision() {
        let decisions = vec![OutputDiscoveryDecision {
            output_name: "proposal_review".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/proposal_review.json".to_string(),
            companion_of: None,
            status: OutputDiscoveryStatus::Rejected,
            reason: OutputDiscoveryReason::StaleExpectedOutput,
            provenance: Some(OutputDiscoveryProvenance::ExactPath),
            canonical_path: None,
            root_class: Some(OutputRootClass::ChainworksMetaRoot),
            baseline_status: Some(ExpectedPathBaselineStatus::RegularContentCaptured),
            size_bytes: Some(32),
            content_digest: None,
            max_bytes_applied: Some(10 * 1024 * 1024),
            aggregate_bytes_after_acceptance: None,
            accepted_payload_ref: None,
            accepted_bytes_sha256: None,
            generated_by: None,
            diagnostics: Default::default(),
            decision_at: chrono::Utc::now(),
        }];

        assert!(!declared_output_has_accepted_discovery_decision(
            Some(&decisions),
            "proposal_review",
            domain::discovery::ExpectedOutputRole::Machine
        ));
        assert!(declared_output_has_accepted_discovery_decision(
            None,
            "proposal_review",
            domain::discovery::ExpectedOutputRole::Machine
        ));
    }
}
