use acp::AcpRuntimeManager;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, approvals,
    artifact_contracts, command_journal, ideas, legacy_discovery_overrides, projections,
    retry_operator_instructions, runs, scheduler, sessions, stages, work_items, workflow_conflicts,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::PrincipalClass;
use domain::agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement};
use domain::approval::ApprovalDecision;
use domain::commands::{CallerContext, Command};
use domain::discovery::{LegacyBroadDiscoveryPolicy, LegacyDiscoveryOverrideInput};
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::provider::InvokeAgentCapacityConfig;
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use domain::workflow_conflict::{
    CandidateTransitionEvaluation, CandidateTransitionResult, WorkflowConflictStatus,
    WorkflowTransitionCursorRecord,
};

use crate::cancellation;
use crate::event_bus::EventSender;
use crate::preflight::{
    DeliveryPreflightResult, missing_delivery_configuration_preflight, run_delivery_preflight,
};
use crate::work_queue::WorkQueue;

pub struct CommandHandler {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
    acp: Option<Arc<AcpRuntimeManager>>,
    capacity_config: Arc<InvokeAgentCapacityConfig>,
    retry_stage_failure_injection: Option<Arc<dyn Fn(&str) -> Result<()> + Send + Sync>>,
}

pub enum CommandResult {
    RunStarted {
        run_id: RunId,
    },
    StartRunBlockedByDeliveryPreflight(StartRunBlockedByDeliveryPreflight),
    StageApproved {
        approval_id: ApprovalId,
    },
    StageRejected {
        approval_id: ApprovalId,
    },
    StageRetryScheduled {
        run_id: RunId,
        stage_id: String,
        legacy_discovery_override_id: Option<String>,
        /// P065: binding id when operator instruction was attached.
        retry_instruction_binding_id: Option<String>,
    },
    WorkflowConflictTransitionSelected {
        run_id: RunId,
        conflict_id: String,
        selected_transition_id: String,
        selected_next_state_id: String,
        retry_instruction_binding_id: Option<String>,
    },
    LegacyDiscoveryOverrideCreated {
        override_id: String,
    },
    RunCancelled {
        run_id: RunId,
    },
    SessionReset {
        run_id: RunId,
        stage_id: String,
    },
    StewardAnalysisQueued,
    ArtifactContractOverrideCreated {
        override_id: String,
    },
    /// P017 Phase B: Mediation confirmation resolved.
    LeadMediationConfirmationResolved {
        run_id: RunId,
        mediation_record_id: String,
        confirmation_subject_id: String,
        journal_id: String,
    },
    /// P017 Phase B: Mediation confirmation is no longer actionable.
    /// DEF-002: Typed result for stale, terminal, canceled, or superseded items
    /// instead of a generic error. Callers can distinguish this from real errors.
    LeadMediationConfirmationStaleOrTerminal {
        confirmation_subject_id: String,
        reason: String,
        journal_id: String,
    },
}

pub struct StartRunBlockedByDeliveryPreflight {
    pub delivery_preflight: DeliveryPreflightResult,
}

/// P029: Wrapper that pairs the command result with the journal audit ID.
/// `CommandHandler::handle` returns this instead of bare `CommandResult`.
pub struct Commanded {
    pub result: CommandResult,
    pub journal_id: String,
}

struct CommandJournalEntry {
    id: String,
    command_type: &'static str,
    payload_json: String,
    run_id: Option<String>,
    created_at: DateTime<Utc>,
    caller_surface: Option<String>,
    caller_principal_id: Option<String>,
    caller_principal_class: Option<String>,
    caller_tool: Option<String>,
    request_id: Option<String>,
}

struct PhaseBDogfoodMetricSnapshot {
    completion_rate: f64,
    sample_size: i64,
    guidance_sufficient_count: i64,
    evidence_source: String,
}

impl CommandJournalEntry {
    fn new(cmd: &Command, caller: &CallerContext) -> Self {
        let command_type = match cmd {
            Command::StartRun(_) => "StartRun",
            Command::ApproveStage(_) => "ApproveStage",
            Command::RejectStage(_) => "RejectStage",
            Command::RetryStage(_) => "RetryStage",
            Command::ResolveWorkflowConflictTransition(_) => "ResolveWorkflowConflictTransition",
            Command::OverrideLegacyDiscoveryPolicy(_) => "OverrideLegacyDiscoveryPolicy",
            Command::MainSyncRequest(_) => "MainSyncRequest",
            Command::MainSyncRetry(_) => "MainSyncRetry",
            Command::MainSyncSetRunOverride(_) => "MainSyncSetRunOverride",
            Command::MainSyncRepairState(_) => "MainSyncRepairState",
            Command::MainSyncRecordRecoveryDecision(_) => "MainSyncRecordRecoveryDecision",
            Command::KnowledgeCapsuleIgnore(_) => "KnowledgeCapsuleIgnore",
            Command::CancelRun(_) => "CancelRun",
            Command::ResetSession(_) => "ResetSession",
            Command::RunStewardAnalysis(_) => "RunStewardAnalysis",
            Command::OverrideArtifactContract(_) => "OverrideArtifactContract",
            Command::ResolveLeadMediationConfirmation(_) => "ResolveLeadMediationConfirmation",
            Command::ResolveApproval(_) => "ResolveApproval",
        };
        let raw = serde_json::to_string(cmd).unwrap_or_default();
        let payload_json = crate::command_journal_redact::redact_for_journal(cmd, &raw);
        let run_id = match cmd {
            Command::StartRun(_) => None,
            Command::ApproveStage(c) => Some(c.run_id.to_string()),
            Command::RejectStage(c) => Some(c.run_id.to_string()),
            Command::RetryStage(c) => Some(c.run_id.to_string()),
            Command::ResolveWorkflowConflictTransition(c) => Some(c.run_id.to_string()),
            Command::OverrideLegacyDiscoveryPolicy(c) => Some(c.run_id.to_string()),
            Command::MainSyncRequest(c) => Some(c.run_id.to_string()),
            Command::MainSyncRetry(c) => Some(c.run_id.to_string()),
            Command::MainSyncSetRunOverride(c) => Some(c.run_id.to_string()),
            Command::MainSyncRepairState(c) => Some(c.run_id.to_string()),
            Command::MainSyncRecordRecoveryDecision(c) => Some(c.run_id.to_string()),
            Command::KnowledgeCapsuleIgnore(c) => Some(c.run_id.to_string()),
            Command::CancelRun(c) => Some(c.run_id.to_string()),
            Command::ResetSession(c) => Some(c.run_id.to_string()),
            Command::RunStewardAnalysis(_) => None,
            Command::OverrideArtifactContract(c) => Some(c.run_id.to_string()),
            Command::ResolveLeadMediationConfirmation(c) => Some(c.run_id.to_string()),
            Command::ResolveApproval(c) => Some(c.run_id.to_string()),
        };
        let principal_class = caller.principal_class.to_string();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            command_type,
            payload_json,
            run_id,
            created_at: Utc::now(),
            caller_surface: Some(caller.surface.to_string()),
            caller_principal_id: Some(caller.principal_id.clone()),
            caller_principal_class: Some(principal_class),
            caller_tool: Some(caller.caller_tool.clone()),
            request_id: caller.request_id.clone(),
        }
    }

    fn is_recorded_in_command_transaction(&self) -> bool {
        matches!(
            self.command_type,
            "StartRun"
                | "ApproveStage"
                | "RejectStage"
                | "RetryStage"
                | "ResolveWorkflowConflictTransition"
                | "OverrideLegacyDiscoveryPolicy"
                | "CancelRun"
                | "ResetSession"
                | "ResolveLeadMediationConfirmation"
                | "ResolveApproval"
        )
    }
}

fn plan_requires_delivery_configuration(plan: &workflow::plan::RunPlan) -> bool {
    plan.states.values().any(|state| {
        state
            .tasks
            .iter()
            .chain(state.post_approval_tasks.iter())
            .any(|task| is_release_agent(&task.agent.agent_id))
    })
}

fn is_release_agent(agent_id: &str) -> bool {
    matches!(
        agent_id,
        "commit_and_push_to_github" | "build_archive_and_push_connect"
    )
}

fn find_source_invoke_work_item<'a>(
    work_items: &'a [WorkItem],
    stage_execution_id: &str,
    agent_id: &str,
    agent_execution_id: &str,
) -> Option<&'a WorkItem> {
    work_items
        .iter()
        .filter(|item| item.kind == WorkItemKind::InvokeAgent)
        .filter_map(|item| {
            let payload = serde_json::from_str::<serde_json::Value>(&item.payload_json).ok()?;
            let claimed_agent_execution_id = payload
                .pointer("/p058_claimed/agent_execution_id")
                .and_then(|value| value.as_str());
            let payload_stage_execution_id = payload
                .get("stage_execution_id")
                .and_then(|value| value.as_str());
            let payload_agent_id = payload.get("agent_id").and_then(|value| value.as_str());
            let matches = claimed_agent_execution_id == Some(agent_execution_id)
                || (payload_stage_execution_id == Some(stage_execution_id)
                    && payload_agent_id == Some(agent_id));
            matches.then_some(item)
        })
        .max_by_key(|item| item.created_at)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetedRetryProviderFallback {
    from_backend_profile_id: Option<String>,
    from_provider: String,
    backend_profile_id: String,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    max_turns: Option<i64>,
}

fn targeted_retry_provider_fallback(
    run: &Run,
    agent_id: &str,
    retry_payload: &serde_json::Value,
    runtime_facts: Option<&AgentExecutionRuntimeFacts>,
) -> Option<TargetedRetryProviderFallback> {
    let from_provider = retry_payload.get("provider")?.as_str()?.to_string();
    if !matches!(
        from_provider.as_str(),
        "gemini" | "gemini_acp" | "claude" | "claude_acp" | "codex" | "codex_acp"
    ) {
        return None;
    }
    let output_contract = retry_payload
        .get("output_contract")
        .and_then(serde_json::Value::as_str);
    let is_proposal_review = output_contract == Some("proposal_review_v1");
    let is_proposal_review_aggregation = agent_id == "lead_orchestrator"
        && retry_payload
            .get("task_outputs")
            .and_then(serde_json::Value::as_array)
            .map(|outputs| {
                outputs
                    .iter()
                    .any(|value| value.as_str() == Some("proposal_review_summary"))
            })
            .unwrap_or(false);
    let is_proposal_authoring = agent_id == "proposal_writer"
        && retry_payload
            .get("task_outputs")
            .and_then(serde_json::Value::as_array)
            .map(|outputs| {
                outputs
                    .iter()
                    .any(|value| value.as_str() == Some("proposal_current"))
            })
            .unwrap_or(false);
    let is_docs_guardian = agent_id == "docs_guardian" && output_contract == Some("docs_report_v1");
    if !is_proposal_review
        && !is_proposal_review_aggregation
        && !is_proposal_authoring
        && !is_docs_guardian
    {
        return None;
    }
    let source_failed_without_required_outputs = runtime_facts
        .map(|facts| {
            matches!(
                facts.failure_kind,
                Some(AgentFailureKind::ProviderQuota)
                    | Some(AgentFailureKind::MissingRequiredOutputs)
            ) || facts.output_settlement == AgentOutputSettlement::MissingRequiredOutputs
        })
        .unwrap_or(true);
    let source_had_transient_runtime_failure = runtime_facts
        .map(|facts| {
            matches!(
                facts.failure_kind,
                Some(
                    AgentFailureKind::ProviderTimeout
                        | AgentFailureKind::TransportClosed
                        | AgentFailureKind::TransportEpipe
                        | AgentFailureKind::TransportProtocolError
                )
            )
        })
        .unwrap_or(false);
    if matches!(
        from_provider.as_str(),
        "claude" | "claude_acp" | "codex" | "codex_acp"
    ) && !source_failed_without_required_outputs
        && !source_had_transient_runtime_failure
    {
        return None;
    }

    let catalog: serde_json::Value =
        serde_json::from_str(run.catalog_snapshot_json.as_deref()?).ok()?;
    let profiles = catalog.get("backend_profiles")?.as_object()?;
    let fallback_id = targeted_retry_fallback_profile_id(
        agent_id,
        &from_provider,
        is_proposal_review_aggregation,
        is_proposal_authoring,
        is_docs_guardian,
        profiles,
    )?;
    let profile = profiles.get(fallback_id)?.as_object()?;
    let provider = profile.get("provider")?.as_str()?.to_string();
    if provider == from_provider {
        return None;
    }

    Some(TargetedRetryProviderFallback {
        from_backend_profile_id: retry_payload
            .get("backend_profile_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        from_provider,
        backend_profile_id: fallback_id.to_string(),
        provider,
        model: profile
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        effort: profile
            .get("effort")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        max_turns: profile.get("max_turns").and_then(serde_json::Value::as_i64),
    })
}

fn targeted_retry_fallback_profile_id<'a>(
    agent_id: &str,
    from_provider: &str,
    is_proposal_review_aggregation: bool,
    is_proposal_authoring: bool,
    is_docs_guardian: bool,
    profiles: &'a serde_json::Map<String, serde_json::Value>,
) -> Option<&'a str> {
    if is_proposal_review_aggregation {
        return ["codex_writer_high", "codex_architect_high"]
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    if is_proposal_authoring {
        let candidates: &[&str] = if matches!(from_provider, "codex" | "codex_acp") {
            &["claude_writer_high", "claude_product_high"]
        } else {
            &["codex_writer_high", "codex_architect_high"]
        };
        return candidates
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    if is_docs_guardian {
        let candidates: &[&str] = if matches!(from_provider, "gemini" | "gemini_acp") {
            &[
                "claude_docs_medium",
                "claude_design_medium",
                "codex_architect_high",
            ]
        } else {
            &[
                "gemini_docs_flash",
                "claude_docs_medium",
                "codex_architect_high",
            ]
        };
        return candidates
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    if matches!(from_provider, "claude" | "claude_acp") {
        return ["codex_architect_high", "codex_writer_high"]
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    if matches!(from_provider, "codex" | "codex_acp") {
        return ["claude_product_high", "claude_design_medium"]
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    let design_reviewer =
        agent_id.contains("ux") || agent_id.contains("ui") || agent_id.contains("macos");
    let candidates: &[&str] = if design_reviewer {
        &[
            "claude_design_medium",
            "claude_product_high",
            "codex_architect_high",
        ]
    } else {
        &["claude_product_high", "codex_architect_high"]
    };
    candidates
        .iter()
        .copied()
        .find(|candidate| profiles.contains_key(*candidate))
}

/// OPS-002 (P017 R4): classify the workflow-compile error so the
/// `phase_c_validation_outcome_total` fail-path metric carries a
/// bounded `failure_kind` label.
///
/// The classifier matches on the typed prefix the compile error emits
/// (`lead_missing`, `lead_ambiguous`, `lead_backend_profile_missing`,
/// `lead_permission_profile_missing`, `lead_resolution_contract_missing`)
/// and falls back to `other_compile_failure` so cardinality stays bounded.
fn classify_phase_c_failure_kind(error_message: &str) -> String {
    for kind in [
        "lead_missing",
        "lead_ambiguous",
        "lead_backend_profile_missing",
        "lead_permission_profile_missing",
        "lead_resolution_contract_missing",
    ] {
        if error_message.contains(kind) {
            return kind.to_string();
        }
    }
    "other_compile_failure".to_string()
}

fn frozen_legacy_broad_discovery_policy(run: &Run) -> Result<LegacyBroadDiscoveryPolicy> {
    let Some(snapshot_json) = run.workflow_snapshot_json.as_deref() else {
        return Ok(LegacyBroadDiscoveryPolicy::Disabled);
    };
    let workflow: workflow::definition::WorkflowFile = serde_json::from_str(snapshot_json)
        .map_err(|e| anyhow!("parse workflow_snapshot_json for legacy discovery policy: {e}"))?;
    Ok(
        match workflow
            .discovery
            .and_then(|discovery| discovery.legacy_broad_discovery_policy)
            .unwrap_or(workflow::definition::LegacyBroadDiscoveryPolicyDef::Disabled)
        {
            workflow::definition::LegacyBroadDiscoveryPolicyDef::Disabled => {
                LegacyBroadDiscoveryPolicy::Disabled
            }
            workflow::definition::LegacyBroadDiscoveryPolicyDef::WorkflowOptIn => {
                LegacyBroadDiscoveryPolicy::WorkflowOptIn
            }
        },
    )
}

fn validate_operator_selected_candidate(candidate: &CandidateTransitionEvaluation) -> Result<()> {
    match candidate.result {
        CandidateTransitionResult::Matched => Ok(()),
        CandidateTransitionResult::NotMatched
            if candidate
                .sanitized_diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.contains("Loop budget exhausted")) =>
        {
            Ok(())
        }
        CandidateTransitionResult::NotMatched => {
            anyhow::bail!(
                "operator conflict resolution may select only loop-budget-exhausted not_matched candidates"
            )
        }
        _ => anyhow::bail!(
            "operator conflict resolution may select only matched candidates or loop-budget-exhausted not_matched candidates"
        ),
    }
}

impl CommandHandler {
    /// Read-only access to the pool for pre-flight lookups (e.g. MCP server
    /// deriving mediation_record_id before building a command).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn new(pool: SqlitePool, events: EventSender, work_queue: WorkQueue) -> Self {
        Self::new_with_capacity(
            pool,
            events,
            work_queue,
            InvokeAgentCapacityConfig::default(),
        )
    }

    pub fn new_with_capacity(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            events,
            work_queue,
            acp: None,
            capacity_config: Arc::new(capacity_config),
            retry_stage_failure_injection: None,
        }
    }

    pub fn new_with_acp(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        acp: Arc<AcpRuntimeManager>,
    ) -> Self {
        Self::new_with_acp_and_capacity(
            pool,
            events,
            work_queue,
            acp,
            InvokeAgentCapacityConfig::default(),
        )
    }

    pub fn new_with_acp_and_capacity(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        acp: Arc<AcpRuntimeManager>,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            events,
            work_queue,
            acp: Some(acp),
            capacity_config: Arc::new(capacity_config),
            retry_stage_failure_injection: None,
        }
    }

    pub fn with_retry_stage_failure_injection(
        mut self,
        injection: Arc<dyn Fn(&str) -> Result<()> + Send + Sync>,
    ) -> Self {
        self.retry_stage_failure_injection = Some(injection);
        self
    }

    fn maybe_inject_retry_stage_failure(&self, step: &str) -> Result<()> {
        if let Some(injection) = &self.retry_stage_failure_injection {
            injection(step)?;
        }
        Ok(())
    }

    pub async fn handle(&self, cmd: Command, caller: CallerContext) -> Result<Commanded> {
        if matches!(&cmd, Command::OverrideArtifactContract(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: OverrideArtifactContract requires operator principal");
        }
        if matches!(
            &cmd,
            Command::RetryStage(c) if c.legacy_discovery_override_policy.is_some()
        ) && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!(
                "forbidden: RetryStage legacy_discovery_override_policy requires operator principal"
            );
        }
        // P065: operator_instruction requires operator principal
        if matches!(
            &cmd,
            Command::RetryStage(c) if c.operator_instruction.is_some()
        ) && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: RetryStage operator_instruction requires operator principal");
        }
        if matches!(&cmd, Command::ResolveWorkflowConflictTransition(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!(
                "forbidden: ResolveWorkflowConflictTransition requires operator principal"
            );
        }
        if matches!(&cmd, Command::OverrideLegacyDiscoveryPolicy(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: OverrideLegacyDiscoveryPolicy requires operator principal");
        }
        if matches!(
            &cmd,
            Command::MainSyncRequest(_)
                | Command::MainSyncRetry(_)
                | Command::MainSyncSetRunOverride(_)
                | Command::MainSyncRepairState(_)
                | Command::MainSyncRecordRecoveryDecision(_)
                | Command::KnowledgeCapsuleIgnore(_)
        ) && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: Proposal 064 commands require operator principal");
        }
        if matches!(&cmd, Command::ResolveApproval(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: ResolveApproval requires operator principal");
        }

        // ── Command journal: record before execution (proposal §6.4) ────────
        let journal = CommandJournalEntry::new(&cmd, &caller);
        if !journal.is_recorded_in_command_transaction() {
            // INSERT is mandatory — fail closed (P029 §P2-005)
            command_journal::record(
                &self.pool,
                &journal.id,
                journal.command_type,
                &journal.payload_json,
                journal.run_id.as_deref(),
                journal.created_at,
                journal.caller_surface.as_deref(),
                journal.caller_principal_id.as_deref(),
                journal.caller_principal_class.as_deref(),
                journal.caller_tool.as_deref(),
                journal.request_id.as_deref(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        }

        let result = self.execute_command(cmd, &journal, &caller).await;

        // Completion/failure are best-effort — log errors but don't fail the command
        if !journal.is_recorded_in_command_transaction() {
            let completed_at = Utc::now();
            match &result {
                Ok(_) => {
                    if let Err(e) =
                        command_journal::complete_entry(&self.pool, &journal.id, completed_at).await
                    {
                        tracing::error!(journal_id = %journal.id, error = %e, "Failed to close journal entry");
                    }
                }
                Err(e) => {
                    if let Err(e2) = command_journal::fail_entry(
                        &self.pool,
                        &journal.id,
                        completed_at,
                        &e.to_string(),
                    )
                    .await
                    {
                        tracing::error!(journal_id = %journal.id, error = %e2, "Failed to record journal failure");
                    }
                }
            }
        }

        result.map(|r| Commanded {
            result: r,
            journal_id: journal.id.clone(),
        })
    }

    async fn execute_command(
        &self,
        cmd: Command,
        journal: &CommandJournalEntry,
        caller: &CallerContext,
    ) -> Result<CommandResult> {
        let journal_id = journal.id.as_str();
        match cmd {
            Command::StartRun(c) => {
                let now = Utc::now();
                let run_id = RunId::new();
                // Compile the plan early to fail fast on invalid YAML before
                // persisting anything.
                let plan = match workflow::compiler::compile(
                    &c.workflow_yaml_path,
                    &c.agent_catalog_yaml_path,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        let message = error.to_string();
                        // OPS-002 (P017 R4): emit phase_c_validation_outcome_total
                        // for the FAIL-CLOSED compile path. Run id is None
                        // because the run row never gets inserted.
                        let failure_kind = classify_phase_c_failure_kind(&message);
                        let _ = db::repos::workflow_conflicts::record_phase_c_validation_failure(
                            &self.pool,
                            &failure_kind,
                            Some(c.workflow_yaml_path.as_str()),
                            Some(c.agent_catalog_yaml_path.as_str()),
                            now,
                        )
                        .await;
                        self.record_failed_command_transaction(
                            journal,
                            "command.StartRun",
                            &message,
                        )
                        .await?;
                        return Err(error);
                    }
                };

                let delivery_preflight_json =
                    if let Some(delivery_configuration_json) = &c.delivery_configuration_json {
                        let delivery_config: domain::run::DeliveryConfiguration =
                            match serde_json::from_str(delivery_configuration_json) {
                                Ok(config) => config,
                                Err(error) => {
                                    let message = error.to_string();
                                    self.record_failed_command_transaction(
                                        journal,
                                        "command.StartRun",
                                        &message,
                                    )
                                    .await?;
                                    return Err(error.into());
                                }
                            };
                        let preflight = run_delivery_preflight(&delivery_config);
                        if !preflight.passed {
                            self.record_completed_command_transaction(journal, "command.StartRun")
                                .await?;
                            return Ok(CommandResult::StartRunBlockedByDeliveryPreflight(
                                StartRunBlockedByDeliveryPreflight {
                                    delivery_preflight: preflight,
                                },
                            ));
                        }
                        match serde_json::to_string(&preflight) {
                            Ok(json) => Some(json),
                            Err(error) => {
                                let message = error.to_string();
                                self.record_failed_command_transaction(
                                    journal,
                                    "command.StartRun",
                                    &message,
                                )
                                .await?;
                                return Err(error.into());
                            }
                        }
                    } else if plan_requires_delivery_configuration(&plan) {
                        self.record_completed_command_transaction(journal, "command.StartRun")
                            .await?;
                        return Ok(CommandResult::StartRunBlockedByDeliveryPreflight(
                            StartRunBlockedByDeliveryPreflight {
                                delivery_preflight: missing_delivery_configuration_preflight(),
                            },
                        ));
                    } else {
                        None
                    };
                let phase_b_dogfood_snapshot =
                    phase_b_dogfood_exit_metric_snapshot(&c.workspace_root);
                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.StartRun").await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
                let idea = if let Some(idea) = ideas::find_by_id_tx(&mut tx, c.idea_id).await? {
                    idea
                } else {
                    let error = anyhow!("Idea {} not found", c.idea_id);
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.StartRun", tx_started);
                    return Err(error);
                };
                let project_key = idea
                    .project_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("untagged")
                    .to_string();
                let validated_review_routing_json = match resolve_start_run_review_routing_json(
                    c.review_routing_json.as_deref(),
                    &idea.body,
                    Some(caller.principal_id.as_str()),
                    now,
                ) {
                    Ok(json) => Some(json),
                    Err(error) => {
                        let message = format!("review_routing_json: {error}");
                        command_journal::fail_entry_tx(&mut tx, &journal.id, Utc::now(), &message)
                            .await?;
                        tx.commit().await?;
                        db::pool::log_write_transaction("command.StartRun", tx_started);
                        return Err(anyhow!(message));
                    }
                };

                let run = Run {
                    id: run_id,
                    idea_id: c.idea_id,
                    status: RunStatus::Pending,
                    workflow_id: c.workflow_id,
                    workflow_title: c.workflow_title,
                    workspace_root: c.workspace_root,
                    artifact_root: c.artifact_root,
                    started_at: now,
                    completed_at: None,
                    cancellation_requested_at: None,
                    cancellation_settled_at: None,
                    cancellation_settlement_log: None,
                    current_state: Some(plan.initial_state),
                    workflow_yaml_path: Some(c.workflow_yaml_path.clone()),
                    agent_catalog_yaml_path: Some(c.agent_catalog_yaml_path.clone()),
                    // Worktree fields — provisioned later by the orchestrator
                    // when the first write-enabled implementation state is entered.
                    worktree_root: None,
                    base_branch: None,
                    base_revision: None,
                    target_branch: None,
                    delivery_configuration_json: c.delivery_configuration_json.clone(),
                    delivery_preflight_json,
                    workflow_family: plan.workflow_family.clone(),
                    project_key: Some(project_key),
                    risk_class: plan.risk_class.clone(),
                    stack: plan.stack.clone(),
                    workflow_snapshot_hash: Some(plan.workflow_snapshot_hash.clone()),
                    catalog_snapshot_hash: Some(plan.catalog_snapshot_hash.clone()),
                    workflow_snapshot_json: Some(plan.workflow_snapshot_json.clone()),
                    catalog_snapshot_json: Some(plan.catalog_snapshot_json.clone()),
                    drift_detected_at: None,
                    drift_details_json: None,
                    // P050: Per-run workspace isolation. All YAML artifact paths
                    // resolve through this meta root instead of shared .chainworks/.
                    chainworks_meta_root: Some(format!(".chainworks/runs/{}", run_id)),
                    // P060: Frozen review routing options.
                    review_routing_json: validated_review_routing_json,
                };
                runs::insert_tx(&mut tx, &run).await?;
                // OPS-001 (P017 R2 audit): the workflow compiler ran
                // Phase C lead-validation as part of `compile()`. Reaching
                // this point means it passed; record the outcome so the
                // metric has at least one production caller per run start.
                db::repos::workflow_conflicts::record_phase_c_validation_outcome_tx(
                    &mut tx, run_id, "pass", "compile", now,
                )
                .await?;
                // OPS-002 (P017 R4): emit phase_c_lead_inventory_external_catalog_total
                // per-run with the inventory result observed at compile time.
                // For the bundled-only catalog path (the local operator's
                // current evidence inventory says zero active externals),
                // this is `inventory_result=zero_active_externals` +
                // `enforcement_decision=waive_warning_window` per the
                // attested evidence at
                // docs/reference/workflow-conflict-evidence/phase-c-external-catalog-enforcement-inventory.json.
                db::repos::workflow_conflicts::record_phase_c_lead_inventory_external_catalog_tx(
                    &mut tx,
                    Some(&run_id.to_string()),
                    "zero_active_externals",
                    "waive_warning_window",
                    Some(c.agent_catalog_yaml_path.as_str()),
                    now,
                )
                .await?;
                // P017 R6 / OPS-001: keep the Phase B dogfood exit evidence
                // visible in the same runtime metric stream as other P017
                // operational metrics. These are snapshot emissions from the
                // signed dogfood exit record, not live mediation counters.
                if let Some(snapshot) = phase_b_dogfood_snapshot.as_ref() {
                    db::repos::workflow_conflicts::record_phase_b_dogfood_mediation_completion_rate_tx(
                        &mut tx,
                        Some(&run_id.to_string()),
                        run.workflow_id.as_str(),
                        "all_phase_b_dogfood_conflicts",
                        snapshot.completion_rate,
                        snapshot.sample_size,
                        snapshot.evidence_source.as_str(),
                        now,
                    )
                    .await?;
                    db::repos::workflow_conflicts::record_phase_b_dogfood_operator_guidance_sufficient_tx(
                        &mut tx,
                        Some(&run_id.to_string()),
                        "lead_mediation_guidance",
                        "sufficient",
                        snapshot.guidance_sufficient_count,
                        snapshot.evidence_source.as_str(),
                        now,
                    )
                    .await?;
                }
                // Activate the idea when its first run starts.
                db::repos::ideas::update_status_tx(
                    &mut tx,
                    c.idea_id,
                    domain::idea::IdeaStatus::Active,
                )
                .await?;
                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({ "run_id": run_id.to_string() })
                            .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(run_id),
                        stage_id: None,
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.StartRun",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.StartRun", tx_started);
                info!(run_id = %run_id, "Run started");
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                let _ = self.events.send(DomainEvent::RunStarted {
                    run_id,
                    idea_id: run.idea_id,
                });
                Ok(CommandResult::RunStarted { run_id })
            }

            Command::OverrideArtifactContract(c) => {
                let override_id = db::repos::artifact_contracts::create_override_and_rebuild(
                    &self.pool,
                    domain::artifact_contracts::ArtifactContractOverrideInput {
                        run_id: c.run_id,
                        contract_id: c.contract_id,
                        override_type: c.override_type,
                        from_status: c.from_status,
                        to_status: c.to_status,
                        reason: c.reason,
                        owner: "operator".to_string(),
                        source_artifacts: c.source_artifacts,
                        expires_at_stage: c.expires_at_stage,
                        journal_id: journal_id.to_string(),
                    },
                )
                .await?;
                Ok(CommandResult::ArtifactContractOverrideCreated { override_id })
            }

            Command::MainSyncRequest(_) => Err(anyhow!(
                "not implemented: MainSyncRequest is frozen in Phase 0 only"
            )),

            Command::MainSyncRetry(_) => Err(anyhow!(
                "not implemented: MainSyncRetry is frozen in Phase 0 only"
            )),

            Command::MainSyncSetRunOverride(_) => Err(anyhow!(
                "not implemented: MainSyncSetRunOverride is frozen in Phase 0 only"
            )),

            Command::MainSyncRepairState(_) => Err(anyhow!(
                "not implemented: MainSyncRepairState is frozen in Phase 0 only"
            )),

            Command::MainSyncRecordRecoveryDecision(_) => Err(anyhow!(
                "not implemented: MainSyncRecordRecoveryDecision is frozen in Phase 0 only"
            )),

            Command::KnowledgeCapsuleIgnore(_) => Err(anyhow!(
                "not implemented: KnowledgeCapsuleIgnore is frozen in Phase 0 only"
            )),

            Command::ApproveStage(c) => {
                let now = Utc::now();
                let has_post_tasks = self
                    .check_has_post_approval_tasks(c.run_id, &c.stage_id)
                    .await;
                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.ApproveStage")
                        .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
                let pending = approvals::list_by_run_tx(&mut tx, c.run_id).await?;
                let approval = if let Some(approval) = pending.into_iter().find(|a| {
                    a.stage_id == c.stage_id
                        && matches!(
                            a.decision,
                            ApprovalDecision::Pending | ApprovalDecision::Requested
                        )
                }) {
                    approval
                } else {
                    let error = anyhow!("No pending approval for stage {}", c.stage_id);
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.ApproveStage", tx_started);
                    return Err(error);
                };

                approvals::resolve_tx(
                    &mut tx,
                    approval.id,
                    ApprovalDecision::Granted,
                    now,
                    c.comment,
                )
                .await?;

                let mut stage_status_event = None;
                let run_stages = stages::list_by_run_tx(&mut tx, c.run_id).await?;
                if let Some(stage) = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id && s.status == StageStatus::WaitingApproval)
                {
                    if stage.stage_type.as_deref() == Some("manual_gate") {
                        // P044 §3d: If post-approval tasks exist, set stage to Running
                        // so the orchestrator can enqueue them. Otherwise settle as Completed.
                        if has_post_tasks {
                            stages::update_status_tx(&mut tx, stage.id, StageStatus::Running)
                                .await?;
                            stage_status_event = Some((stage.id, StageStatus::Running));
                        } else {
                            stages::settle_tx(
                                &mut tx,
                                stage.id,
                                StageSettlementKind::Completed,
                                now,
                            )
                            .await?;
                            stage_status_event = Some((stage.id, StageStatus::Completed));
                        }
                    } else {
                        stages::update_status_tx(&mut tx, stage.id, StageStatus::Running).await?;
                        stage_status_event = Some((stage.id, StageStatus::Running));
                    }
                }

                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({ "run_id": c.run_id.to_string() })
                            .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(c.run_id),
                        stage_id: None,
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.ApproveStage",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.ApproveStage", tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                if let Some((stage_execution_id, status)) = stage_status_event {
                    let _ = self.events.send(DomainEvent::StageStatusChanged {
                        run_id: c.run_id,
                        stage_execution_id,
                        status,
                    });
                }
                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision: ApprovalDecision::Granted,
                });
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageApproved {
                    approval_id: approval.id,
                })
            }

            Command::RejectStage(c) => {
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.RejectStage").await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
                let pending = approvals::list_by_run_tx(&mut tx, c.run_id).await?;
                let approval = if let Some(approval) = pending.into_iter().find(|a| {
                    a.stage_id == c.stage_id
                        && matches!(
                            a.decision,
                            ApprovalDecision::Pending | ApprovalDecision::Requested
                        )
                }) {
                    approval
                } else {
                    let error = anyhow!("No pending approval for stage {}", c.stage_id);
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.RejectStage", tx_started);
                    return Err(error);
                };

                approvals::resolve_tx(
                    &mut tx,
                    approval.id,
                    ApprovalDecision::Rejected,
                    now,
                    c.comment,
                )
                .await?;
                let mut should_enqueue_advance = false;
                let mut stage_status_event = None;

                // Workflow manual gates use rejection as transition evidence so
                // the state machine can route normal loopbacks such as state_6 -> state_5.
                // Non-manual stages retain the existing rejection-as-blocked behavior.
                let run_stages = stages::list_by_run_tx(&mut tx, c.run_id).await?;
                if let Some(stage) = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id && s.status == StageStatus::WaitingApproval)
                {
                    if stage.stage_type.as_deref() == Some("manual_gate") {
                        stages::settle_tx(&mut tx, stage.id, StageSettlementKind::Completed, now)
                            .await?;
                        should_enqueue_advance = true;
                        stage_status_event = Some((stage.id, StageStatus::Completed));
                    } else {
                        stages::update_status_tx(&mut tx, stage.id, StageStatus::Blocked).await?;
                        stage_status_event = Some((stage.id, StageStatus::Blocked));
                    }
                }

                if should_enqueue_advance {
                    work_items::enqueue_tx(
                        &mut tx,
                        &WorkItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            kind: WorkItemKind::AdvanceRun,
                            payload_json: serde_json::json!({ "run_id": c.run_id.to_string() })
                                .to_string(),
                            status: WorkItemStatus::Pending,
                            run_id: Some(c.run_id),
                            stage_id: None,
                            created_at: now,
                            scheduled_at: now,
                            attempt_count: 0,
                            last_error: None,
                        },
                    )
                    .await?;
                }

                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.RejectStage",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.RejectStage", tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                if let Some((stage_execution_id, status)) = stage_status_event {
                    let _ = self.events.send(DomainEvent::StageStatusChanged {
                        run_id: c.run_id,
                        stage_execution_id,
                        status,
                    });
                }
                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision: ApprovalDecision::Rejected,
                });
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageRejected {
                    approval_id: approval.id,
                })
            }

            Command::RetryStage(c) => {
                // P065: validate operator_instruction early (before any DB writes)
                let validated_instruction = if let Some(ref raw) = c.operator_instruction {
                    Some(
                        domain::retry_instruction::validate_operator_instruction(raw)
                            .map_err(|e| anyhow!("operator_instruction validation: {e}"))?,
                    )
                } else {
                    None
                };

                if let Some(agent_execution_id) = c.agent_execution_id {
                    if c.legacy_discovery_override_policy.is_some() {
                        anyhow::bail!(
                            "legacy_discovery_override_policy is only supported for full stage retry"
                        );
                    }
                    return self
                        .retry_agent_execution(
                            c.run_id,
                            &c.stage_id,
                            agent_execution_id,
                            c.consume_quota_budget_now,
                            journal_id,
                            journal,
                            validated_instruction.as_deref(),
                            &caller,
                        )
                        .await;
                }

                let now = Utc::now();
                let retry_tx_started = Instant::now();
                let mut retry_tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.RetryStage").await?;
                command_journal::record_tx(
                    &mut retry_tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
                self.maybe_inject_retry_stage_failure("record_journal")?;

                let run_stages = stages::list_by_run_tx(&mut retry_tx, c.run_id).await?;
                let matching_stages = run_stages
                    .iter()
                    .filter(|s| s.stage_id == c.stage_id)
                    .collect::<Vec<_>>();
                let old_stage = if let Some(old_stage) =
                    matching_stages.iter().copied().max_by_key(|s| s.started_at)
                {
                    old_stage
                } else {
                    let error = anyhow!("Stage {} not found", c.stage_id);
                    command_journal::fail_entry_tx(
                        &mut retry_tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    retry_tx.commit().await?;
                    db::pool::log_write_transaction("command.RetryStage", retry_tx_started);
                    return Err(error);
                };
                let run = runs::find_by_id_tx(&mut retry_tx, c.run_id)
                    .await?
                    .ok_or_else(|| anyhow!("Run {} not found", c.run_id))?;
                let completed_current_stage_on_blocked_run =
                    if old_stage.status == StageStatus::Completed {
                        run.status == RunStatus::Blocked
                            && (run.current_state.as_deref() == Some(c.stage_id.as_str())
                                || old_stage.stage_id == c.stage_id)
                    } else {
                        false
                    };

                if !matches!(old_stage.status, StageStatus::Failed | StageStatus::Blocked)
                    && !completed_current_stage_on_blocked_run
                {
                    let error = anyhow!(
                        "Stage {} latest attempt is {} and cannot be retried yet",
                        c.stage_id,
                        old_stage.status
                    );
                    command_journal::fail_entry_tx(
                        &mut retry_tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    retry_tx.commit().await?;
                    db::pool::log_write_transaction("command.RetryStage", retry_tx_started);
                    return Err(error);
                }

                let next_attempt_number = matching_stages
                    .iter()
                    .map(|s| s.attempt_number)
                    .max()
                    .unwrap_or(old_stage.attempt_number)
                    + 1;
                let new_stage = StageExecution {
                    id: domain::ids::StageExecutionId::new(),
                    run_id: c.run_id,
                    stage_id: old_stage.stage_id.clone(),
                    label: old_stage.label.clone(),
                    status: StageStatus::Pending,
                    iteration: old_stage.iteration,
                    attempt_number: next_attempt_number,
                    settlement_kind: None,
                    started_at: now,
                    completed_at: None,
                    owner_agent: old_stage.owner_agent.clone(),
                    provider: old_stage.provider.clone(),
                    model: old_stage.model.clone(),
                    stage_type: old_stage.stage_type.clone(),
                    validation_failure_json: None,
                    evidence_packet_json: None,
                    recovery_snapshot_json: None,
                    retry_reason: Some("operator_retry".into()),
                };
                let legacy_discovery_override_input = if let Some(requested_policy) =
                    c.legacy_discovery_override_policy
                {
                    let reason = c.legacy_discovery_override_reason.clone().ok_or_else(|| {
                            anyhow!(
                                "legacy_discovery_override_reason is required with legacy_discovery_override_policy"
                            )
                        })?;
                    Some(LegacyDiscoveryOverrideInput {
                        run_id: c.run_id,
                        stage_id: c.stage_id.clone(),
                        workflow_id: run.workflow_id.clone(),
                        target_stage_execution_id: new_stage.id,
                        target_attempt_number: next_attempt_number,
                        actor_id: caller.principal_id.clone(),
                        reason,
                        requested_policy,
                        from_policy: frozen_legacy_broad_discovery_policy(&run)?,
                        approval_source: caller.caller_tool.clone(),
                        journal_id: journal_id.to_string(),
                    })
                } else {
                    None
                };
                let retry_advance_work_item_id = new_stage.id.to_string();
                let retry_invoke_work_item_id = format!("p058-invoke:{}:0", new_stage.id);
                apply_quota_retry_budget_for_stage_tx(
                    &mut retry_tx,
                    c.run_id,
                    old_stage.id,
                    c.consume_quota_budget_now,
                    journal_id,
                )
                .await?;
                self.maybe_inject_retry_stage_failure("apply_quota_budget")?;
                agent_executions::cancel_running_by_stage_tx(&mut retry_tx, old_stage.id, now)
                    .await?;
                self.maybe_inject_retry_stage_failure("cancel_agent_executions")?;
                work_items::cancel_pending_or_running_by_stage_tx(
                    &mut retry_tx,
                    c.run_id,
                    &c.stage_id,
                    now,
                    "superseded_by_retry",
                )
                .await?;
                self.maybe_inject_retry_stage_failure("cancel_work_items")?;
                stages::settle_tx(
                    &mut retry_tx,
                    old_stage.id,
                    StageSettlementKind::Skipped,
                    now,
                )
                .await?;
                self.maybe_inject_retry_stage_failure("settle_old_stage")?;
                stages::insert_tx(&mut retry_tx, &new_stage).await?;
                self.maybe_inject_retry_stage_failure("insert_new_stage")?;
                sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
                    .bind(RunStatus::Running.to_string())
                    .bind(c.stage_id.clone())
                    .bind(c.run_id.to_string())
                    .execute(&mut *retry_tx)
                    .await?;
                self.maybe_inject_retry_stage_failure("update_run_for_retry")?;
                supersede_current_workflow_conflict_for_stage_retry_tx(
                    &mut retry_tx,
                    c.run_id,
                    &c.stage_id,
                    now,
                    journal_id,
                )
                .await?;
                self.maybe_inject_retry_stage_failure("supersede_workflow_conflict")?;
                let legacy_discovery_override_id = if let Some(input) =
                    legacy_discovery_override_input.as_ref()
                {
                    let override_record = legacy_discovery_overrides::create_for_pending_retry_tx(
                        &mut retry_tx,
                        input,
                    )
                    .await?;
                    // OPS-001 (P017 R2 audit): an operator-attested
                    // legacy/external catalog override is the canonical
                    // external-catalog warning decision point. Emit one
                    // metric event per override so rollout dashboards can
                    // track override volume + decision class.
                    let _ = db::repos::workflow_conflicts::record_external_catalog_warning_tx(
                        &mut retry_tx,
                        &c.run_id.to_string(),
                        "P017_PHASE_C_EXTERNAL_CATALOG_UNDISCOVERED",
                        "enabled",
                        "legacy_discovery_override",
                        now,
                    )
                    .await;
                    Some(override_record.override_id)
                } else {
                    None
                };
                // P065: create parent binding for operator instruction (full-stage retry).
                // Child delivery rows are deferred to the orchestrator's fanout.
                let retry_instruction_binding_id = if let Some(ref instruction_text) =
                    validated_instruction
                {
                    let binding =
                            retry_operator_instructions::create_for_retry_attempt_tx(
                                &mut retry_tx,
                                &domain::retry_instruction::RetryInstructionBindingInput {
                                    journal_id: journal_id.to_string(),
                                    run_id: c.run_id,
                                    stage_id: c.stage_id.clone(),
                                    source_stage_execution_id: old_stage.id,
                                    retry_stage_execution_id: new_stage.id,
                                    retry_attempt_number: next_attempt_number,
                                    target_agent_execution_id: None,
                                    scope_kind: domain::retry_instruction::RetryInstructionScopeKind::FullStageRetry,
                                    instruction_text: instruction_text.clone(),
                                    created_by_principal_id: caller.principal_id.clone(),
                                    created_by_principal_class: caller.principal_class.to_string(),
                                },
                            )
                            .await?;
                    Some(binding.binding_id)
                } else {
                    None
                };
                self.maybe_inject_retry_stage_failure("create_retry_instruction_binding")?;

                artifact_contracts::mark_active_claims_superseded_pending_retry_for_stage_tx(
                    &mut retry_tx,
                    c.run_id,
                    &old_stage.id.to_string(),
                    &retry_invoke_work_item_id,
                    journal_id,
                )
                .await?;
                self.maybe_inject_retry_stage_failure("supersede_artifact_claims")?;
                work_items::enqueue_tx(
                    &mut retry_tx,
                    &WorkItem {
                        id: retry_advance_work_item_id,
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({
                            "run_id": c.run_id.to_string(),
                            "stage_id": c.stage_id.clone()
                        })
                        .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(c.run_id),
                        stage_id: Some(c.stage_id.clone()),
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                self.maybe_inject_retry_stage_failure("enqueue_retry_wake")?;
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut retry_tx,
                    &self.capacity_config,
                    now,
                    "command.RetryStage",
                    0,
                )
                .await?;
                self.maybe_inject_retry_stage_failure("refresh_scheduler")?;
                command_journal::complete_entry_tx(&mut retry_tx, &journal.id, Utc::now()).await?;
                self.maybe_inject_retry_stage_failure("complete_journal")?;
                retry_tx.commit().await?;
                db::pool::log_write_transaction("command.RetryStage", retry_tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);

                // Refresh projections so reads reflect the retry.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageRetryScheduled {
                    run_id: c.run_id,
                    stage_id: c.stage_id,
                    legacy_discovery_override_id,
                    retry_instruction_binding_id,
                })
            }

            Command::ResolveWorkflowConflictTransition(c) => {
                if c.resolution_reason.trim().is_empty() {
                    anyhow::bail!("resolution_reason is required");
                }
                let validated_instruction = if let Some(ref raw) = c.operator_instruction {
                    Some(
                        domain::retry_instruction::validate_operator_instruction(raw)
                            .map_err(|e| anyhow!("operator_instruction validation: {e}"))?,
                    )
                } else {
                    None
                };

                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = db::pool::begin_immediate_with_retry(
                    &self.pool,
                    "command.ResolveWorkflowConflictTransition",
                )
                .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

                let run = runs::find_by_id_tx(&mut tx, c.run_id)
                    .await?
                    .ok_or_else(|| anyhow!("Run {} not found", c.run_id))?;
                let conflict =
                    workflow_conflicts::get_current_blocking_conflict_tx(&mut tx, c.run_id)
                        .await?
                        .ok_or_else(|| {
                            anyhow!("Run {} has no current blocking workflow conflict", c.run_id)
                        })?;
                if conflict.conflict_id != c.conflict_id {
                    anyhow::bail!(
                        "Conflict {} is not the current blocking conflict for run {}",
                        c.conflict_id,
                        c.run_id
                    );
                }
                if !conflict.status.is_current_blocking() {
                    anyhow::bail!("Conflict {} is not currently blocking", c.conflict_id);
                }
                if run.current_state.as_deref() != Some(conflict.current_state_id.as_str()) {
                    anyhow::bail!(
                        "Run {} current_state does not match conflict state {}",
                        c.run_id,
                        conflict.current_state_id
                    );
                }

                let selected_candidate = conflict
                    .candidate_transitions
                    .iter()
                    .find(|candidate| candidate.transition_id == c.selected_transition_id)
                    .ok_or_else(|| {
                        anyhow!(
                            "Transition {} is not a candidate for conflict {}",
                            c.selected_transition_id,
                            c.conflict_id
                        )
                    })?;
                validate_operator_selected_candidate(selected_candidate)?;
                let selected_next_state_id = selected_candidate.to_state_id.clone();

                let resolved_conflict = workflow_conflicts::transition_conflict_status_tx(
                    &mut tx,
                    &conflict.conflict_id,
                    WorkflowConflictStatus::Resolved,
                    now,
                    Some(serde_json::json!({
                        "resolution_kind": "operator_selected_candidate_transition",
                        "selected_transition_id": c.selected_transition_id,
                        "selected_next_state_id": selected_next_state_id,
                        "selected_candidate_result": selected_candidate.result.to_string(),
                        "resolution_reason": c.resolution_reason,
                        "caller_principal_id": caller.principal_id,
                        "caller_tool": caller.caller_tool,
                    })),
                    None,
                    None,
                )
                .await?;
                workflow_conflicts::record_recovery_action_chosen_tx(
                    &mut tx,
                    &resolved_conflict,
                    "operator_selected_candidate_transition",
                    &caller.surface.to_string(),
                    "accepted",
                    now,
                )
                .await?;

                let run_stages = stages::list_by_run_tx(&mut tx, c.run_id).await?;
                let latest_target_stage = run_stages
                    .iter()
                    .filter(|stage| stage.stage_id == selected_next_state_id)
                    .max_by_key(|stage| (stage.iteration, stage.attempt_number, stage.started_at));
                let mut enqueued_stage_id = None;
                let mut retry_stage_execution_id = None;
                let mut source_stage_execution_id = None;
                let mut retry_attempt_number = None;
                if let Some(previous) = latest_target_stage {
                    if matches!(
                        previous.status,
                        StageStatus::Completed
                            | StageStatus::Failed
                            | StageStatus::Blocked
                            | StageStatus::Skipped
                    ) {
                        let next_stage = StageExecution {
                            id: domain::ids::StageExecutionId::new(),
                            run_id: c.run_id,
                            stage_id: previous.stage_id.clone(),
                            label: previous.label.clone(),
                            status: StageStatus::Pending,
                            iteration: previous.iteration + 1,
                            attempt_number: 1,
                            settlement_kind: None,
                            started_at: now,
                            completed_at: None,
                            owner_agent: previous.owner_agent.clone(),
                            provider: previous.provider.clone(),
                            model: previous.model.clone(),
                            stage_type: previous.stage_type.clone(),
                            validation_failure_json: None,
                            evidence_packet_json: None,
                            recovery_snapshot_json: None,
                            retry_reason: Some("operator_conflict_resolution".into()),
                        };
                        enqueued_stage_id = Some(next_stage.stage_id.clone());
                        retry_stage_execution_id = Some(next_stage.id);
                        source_stage_execution_id = Some(previous.id);
                        retry_attempt_number = Some(next_stage.attempt_number);
                        stages::insert_tx(&mut tx, &next_stage).await?;
                    }
                }
                let retry_instruction_binding_id = if let Some(ref instruction_text) =
                    validated_instruction
                {
                    let retry_stage_execution_id = retry_stage_execution_id.ok_or_else(|| {
                            anyhow!(
                                "operator_instruction requires a newly created retry stage for selected workflow transition"
                            )
                        })?;
                    let source_stage_execution_id = source_stage_execution_id.ok_or_else(|| {
                            anyhow!(
                                "operator_instruction requires a source stage for selected workflow transition"
                            )
                        })?;
                    let retry_attempt_number = retry_attempt_number.ok_or_else(|| {
                            anyhow!(
                                "operator_instruction requires retry attempt metadata for selected workflow transition"
                            )
                        })?;
                    let binding = retry_operator_instructions::create_for_retry_attempt_tx(
                            &mut tx,
                            &domain::retry_instruction::RetryInstructionBindingInput {
                                journal_id: journal_id.to_string(),
                                run_id: c.run_id,
                                stage_id: selected_next_state_id.clone(),
                                source_stage_execution_id,
                                retry_stage_execution_id,
                                retry_attempt_number,
                                target_agent_execution_id: None,
                                scope_kind:
                                    domain::retry_instruction::RetryInstructionScopeKind::FullStageRetry,
                                instruction_text: instruction_text.clone(),
                                created_by_principal_id: caller.principal_id.clone(),
                                created_by_principal_class: caller.principal_class.to_string(),
                            },
                        )
                        .await?;
                    Some(binding.binding_id)
                } else {
                    None
                };

                sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
                    .bind(RunStatus::Running.to_string())
                    .bind(&selected_next_state_id)
                    .bind(c.run_id.to_string())
                    .execute(&mut *tx)
                    .await?;
                workflow_conflicts::upsert_transition_cursor_tx(
                    &mut tx,
                    &WorkflowTransitionCursorRecord {
                        schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
                        run_id: c.run_id.to_string(),
                        current_state_id: conflict.current_state_id.clone(),
                        cursor_status: "operator_transition_selected".to_string(),
                        resume_policy: "continue_from_selected_transition".to_string(),
                        selected_transition_id: Some(c.selected_transition_id.clone()),
                        selected_next_state_id: Some(selected_next_state_id.clone()),
                        conflict_id: Some(conflict.conflict_id.clone()),
                        conflict_fingerprint: Some(conflict.conflict_fingerprint.clone()),
                        candidate_transition_hash: Some(conflict.candidate_transition_hash.clone()),
                        terminal_failure_reason: None,
                        updated_at: now,
                    },
                )
                .await?;
                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: format!(
                            "operator-transition:{}:{}",
                            c.conflict_id,
                            uuid::Uuid::new_v4()
                        ),
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({
                            "run_id": c.run_id.to_string(),
                            "reason": "operator_conflict_resolution",
                            "conflict_id": c.conflict_id.clone(),
                            "selected_transition_id": c.selected_transition_id.clone(),
                            "to": selected_next_state_id.clone(),
                        })
                        .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(c.run_id),
                        stage_id: enqueued_stage_id
                            .or_else(|| Some(selected_next_state_id.clone())),
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.ResolveWorkflowConflictTransition",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction(
                    "command.ResolveWorkflowConflictTransition",
                    tx_started,
                );
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::WorkflowConflictTransitionSelected {
                    run_id: c.run_id,
                    conflict_id: conflict.conflict_id,
                    selected_transition_id: c.selected_transition_id,
                    selected_next_state_id,
                    retry_instruction_binding_id,
                })
            }

            Command::OverrideLegacyDiscoveryPolicy(c) => {
                let run = runs::find_by_id(&self.pool, c.run_id)
                    .await?
                    .ok_or_else(|| anyhow!("Run {} not found", c.run_id))?;
                let target_stage = stages::find_by_id(&self.pool, c.target_stage_execution_id)
                    .await?
                    .ok_or_else(|| {
                        anyhow!(
                            "legacy discovery override target stage execution {} not found",
                            c.target_stage_execution_id
                        )
                    })?;
                if target_stage.run_id != c.run_id || target_stage.stage_id != c.stage_id {
                    return Err(anyhow!(
                        "legacy discovery override target stage execution {} does not match run {} stage {}",
                        c.target_stage_execution_id,
                        c.run_id,
                        c.stage_id
                    ));
                }
                if target_stage.attempt_number != c.target_attempt_number {
                    return Err(anyhow!(
                        "legacy discovery override target attempt mismatch: requested {}, found {}",
                        c.target_attempt_number,
                        target_stage.attempt_number
                    ));
                }
                if target_stage.status != StageStatus::Pending {
                    return Err(anyhow!(
                        "legacy discovery override target stage execution {} already started or settled with status {}",
                        c.target_stage_execution_id,
                        target_stage.status
                    ));
                }

                let input = LegacyDiscoveryOverrideInput {
                    run_id: c.run_id,
                    stage_id: c.stage_id.clone(),
                    workflow_id: run.workflow_id.clone(),
                    target_stage_execution_id: c.target_stage_execution_id,
                    target_attempt_number: c.target_attempt_number,
                    actor_id: caller.principal_id.clone(),
                    reason: c.legacy_discovery_override_reason,
                    requested_policy: c.legacy_discovery_override_policy,
                    from_policy: frozen_legacy_broad_discovery_policy(&run)?,
                    approval_source: caller.caller_tool.clone(),
                    journal_id: journal_id.to_string(),
                };
                let tx_started = Instant::now();
                let mut tx = db::pool::begin_immediate_with_retry(
                    &self.pool,
                    "command.OverrideLegacyDiscoveryPolicy",
                )
                .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow!("command journal insert failed: {e}"))?;
                let created =
                    legacy_discovery_overrides::create_for_pending_retry_tx(&mut tx, &input)
                        .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction(
                    "command.OverrideLegacyDiscoveryPolicy",
                    tx_started,
                );

                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::LegacyDiscoveryOverrideCreated {
                    override_id: created.override_id,
                })
            }

            Command::CancelRun(c) => {
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.CancelRun").await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

                let run = if let Some(run) = runs::find_by_id_tx(&mut tx, c.run_id).await? {
                    run
                } else {
                    let error = anyhow!("Run {} not found", c.run_id);
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.CancelRun", tx_started);
                    return Err(error);
                };

                if run.status.is_terminal() {
                    let error = anyhow!("Run {} is already in terminal state", c.run_id);
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.CancelRun", tx_started);
                    return Err(error);
                }

                let settlement = cancellation::begin_settlement_tx(
                    &mut tx,
                    c.run_id,
                    now,
                    &self.capacity_config,
                    "command.CancelRun",
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.CancelRun", tx_started);
                self.work_queue
                    .publish_scheduler_notification(settlement.scheduler_refresh);

                // Worktree cleanup on cancel (Proposal 007).
                if let Some(ref wt) = run.worktree_root {
                    if let Err(e) =
                        crate::worktree::WorktreeProvisioner::cleanup(wt, &run.workspace_root).await
                    {
                        tracing::warn!(
                            run_id = %c.run_id,
                            worktree = %wt,
                            error = %e,
                            "Worktree cleanup on cancel failed"
                        );
                    }
                }

                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id: c.run_id,
                    status: RunStatus::Cancelling,
                });

                cancellation::spawn_finalize_settlement(
                    self.pool.clone(),
                    self.events.clone(),
                    self.acp.clone(),
                    c.run_id,
                );

                Ok(CommandResult::RunCancelled { run_id: c.run_id })
            }

            Command::RunStewardAnalysis(c) => {
                let artifact_base = c
                    .artifact_base
                    .or_else(|| std::env::var("CHAINWORKS_META_ROOT").ok())
                    .unwrap_or_else(|| ".chainworks".into());
                self.work_queue
                    .enqueue(
                        WorkItemKind::StewardAnalysis,
                        None,
                        None,
                        serde_json::json!({
                            "reason": c.reason,
                            "artifact_base": artifact_base,
                        }),
                    )
                    .await?;
                Ok(CommandResult::StewardAnalysisQueued)
            }

            Command::ResolveLeadMediationConfirmation(c) => {
                // BLK-006: Guard against resolution when mediation is disabled
                if !crate::mediation::feature_flag::is_phase_b_mediation_enabled() {
                    return Err(anyhow!("Phase B mediation is disabled"));
                }

                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = db::pool::begin_immediate_with_retry(
                    &self.pool,
                    "command.ResolveLeadMediationConfirmation",
                )
                .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

                // Validate the confirmation exists and is pending
                let confirmation = db::repos::lead_mediation_confirmations::find_by_id_tx(
                    &mut tx,
                    &c.confirmation_subject_id,
                )
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "Mediation confirmation {} not found",
                        c.confirmation_subject_id
                    )
                })?;

                // BLK-005: Validate run_id matches the confirmation's run
                if confirmation.run_id != c.run_id.to_string() {
                    let error = anyhow!(
                        "Confirmation run_id mismatch: confirmation belongs to a different run"
                    );
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction(
                        "command.ResolveLeadMediationConfirmation",
                        tx_started,
                    );
                    return Err(error);
                }

                // MF-PRE-ENABLE-005: Validate idempotency_key against stored scope key.
                if let Some(ref stored_key) = confirmation.idempotency_scope_key {
                    if *stored_key != c.idempotency_key {
                        let error = anyhow!(
                            "Idempotency key mismatch for confirmation {}",
                            c.confirmation_subject_id,
                        );
                        command_journal::fail_entry_tx(
                            &mut tx,
                            &journal.id,
                            Utc::now(),
                            &error.to_string(),
                        )
                        .await?;
                        tx.commit().await?;
                        db::pool::log_write_transaction(
                            "command.ResolveLeadMediationConfirmation",
                            tx_started,
                        );
                        return Err(error);
                    }
                }

                if confirmation.status != domain::mediation::MediationConfirmationStatus::Pending {
                    // MF-PRE-ENABLE-005: If already resolved with the same idempotency key,
                    // return cached success instead of an error (idempotent retry).
                    if confirmation.status
                        == domain::mediation::MediationConfirmationStatus::Resolved
                    {
                        let mediation_record_id = &confirmation.mediation_record_id;
                        command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now())
                            .await?;
                        tx.commit().await?;
                        db::pool::log_write_transaction(
                            "command.ResolveLeadMediationConfirmation",
                            tx_started,
                        );
                        return Ok(CommandResult::LeadMediationConfirmationResolved {
                            run_id: c.run_id,
                            mediation_record_id: mediation_record_id.clone(),
                            confirmation_subject_id: c.confirmation_subject_id,
                            journal_id: journal_id.to_string(),
                        });
                    }
                    // DEF-002: Return typed stale_or_terminal result instead of
                    // generic error so MCP callers can distinguish this outcome.
                    let reason = format!(
                        "confirmation status is '{}' (not pending)",
                        confirmation.status,
                    );
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &format!("stale_or_terminal: {}", reason),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction(
                        "command.ResolveLeadMediationConfirmation",
                        tx_started,
                    );
                    return Ok(CommandResult::LeadMediationConfirmationStaleOrTerminal {
                        confirmation_subject_id: c.confirmation_subject_id,
                        reason,
                        journal_id: journal_id.to_string(),
                    });
                }

                // Validate conflict fingerprint matches
                // CL-001: Do not leak stored fingerprint in error messages.
                if confirmation.conflict_fingerprint != c.conflict_fingerprint {
                    tracing::debug!(
                        confirmation_id = %c.confirmation_subject_id,
                        stored_fingerprint = %confirmation.conflict_fingerprint,
                        supplied_fingerprint = %c.conflict_fingerprint,
                        "Conflict fingerprint mismatch detail"
                    );
                    let error = anyhow!("Conflict fingerprint mismatch (stale_or_superseded)");
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction(
                        "command.ResolveLeadMediationConfirmation",
                        tx_started,
                    );
                    return Err(error);
                }

                // Validate mediation record linkage — derive mediation_record_id
                // from the confirmation record instead of trusting the caller
                let mediation_record_id = &confirmation.mediation_record_id;

                // Resolve the confirmation — MC-001: check rows_affected
                // to detect concurrent resolution (CAS guard on status='pending').
                let resolve_rows = db::repos::lead_mediation_confirmations::resolve_tx(
                    &mut tx,
                    &c.confirmation_subject_id,
                    &c.decision.to_string(),
                    c.comment.as_deref(),
                    caller.principal_id.as_str(),
                    now,
                )
                .await?;

                if resolve_rows == 0 {
                    // Confirmation was concurrently resolved, expired, or superseded.
                    // DEF-002: Return typed stale_or_terminal result.
                    let reason = "concurrent resolution (CAS guard blocked update)".to_string();
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &format!("stale_or_terminal: {}", reason),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction(
                        "command.ResolveLeadMediationConfirmation",
                        tx_started,
                    );
                    return Ok(CommandResult::LeadMediationConfirmationStaleOrTerminal {
                        confirmation_subject_id: c.confirmation_subject_id,
                        reason,
                        journal_id: journal_id.to_string(),
                    });
                }

                // BLK-004: Route settlement through MediationSettlementService
                match c.decision {
                    domain::mediation::MediationConfirmationDecision::Confirm => {
                        crate::mediation::settlement::settle_confirmed_tx(
                            &mut tx,
                            mediation_record_id,
                            now,
                        )
                        .await?;
                    }
                    domain::mediation::MediationConfirmationDecision::ManualFallback => {
                        crate::mediation::settlement::settle_rejected_clone_manual_tx(
                            &mut tx,
                            mediation_record_id,
                            now,
                        )
                        .await?;
                    }
                };
                if let Some(conflict) =
                    workflow_conflicts::find_conflict_by_id_tx(&mut tx, &confirmation.conflict_id)
                        .await?
                {
                    let action_class = match c.decision {
                        domain::mediation::MediationConfirmationDecision::Confirm => {
                            "lead_mediation_confirmed"
                        }
                        domain::mediation::MediationConfirmationDecision::ManualFallback => {
                            "manual_fallback"
                        }
                    };
                    workflow_conflicts::record_recovery_action_chosen_tx(
                        &mut tx,
                        &conflict,
                        action_class,
                        &caller.surface.to_string(),
                        "accepted",
                        now,
                    )
                    .await?;
                }

                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction(
                    "command.ResolveLeadMediationConfirmation",
                    tx_started,
                );

                let mediation_record_id_owned = mediation_record_id.clone();

                let _ =
                    self.events
                        .send(domain::events::DomainEvent::MediationConfirmationResolved {
                            run_id: c.run_id,
                            mediation_record_id: mediation_record_id_owned.clone(),
                            confirmation_subject_id: c.confirmation_subject_id.clone(),
                            decision: c.decision.clone(),
                        });

                // P017 B2-006: Enqueue AdvanceRun to re-advance the run after mediation
                // settlement, just as ApproveStage does for stage approvals. This triggers
                // the orchestrator to re-evaluate transitions with the mediation outcome.
                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(c.run_id),
                        None,
                        serde_json::json!({
                            "run_id": c.run_id.to_string(),
                            "trigger": "mediation_confirmation_resolved",
                            "mediation_record_id": mediation_record_id_owned,
                        }),
                    )
                    .await?;

                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::LeadMediationConfirmationResolved {
                    run_id: c.run_id,
                    mediation_record_id: mediation_record_id_owned,
                    confirmation_subject_id: c.confirmation_subject_id,
                    journal_id: journal_id.to_string(),
                })
            }

            Command::ResetSession(c) => {
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.ResetSession")
                        .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

                let mut generation_ids_to_close = Vec::new();

                // Mark the stage as requiring a reset by setting it to Pending.
                let run_stages = stages::list_by_run_tx(&mut tx, c.run_id).await?;
                if let Some(stage) = run_stages.iter().find(|s| s.stage_id == c.stage_id) {
                    let executions = agent_executions::find_by_stage_tx(&mut tx, stage.id).await?;
                    for execution in executions {
                        if let Some(ref generation_id) = execution.session_generation_id {
                            sessions::end_generation_tx(
                                &mut tx,
                                generation_id,
                                domain::session::SessionGenerationStatus::Reset,
                                "operator_reset",
                                now,
                            )
                            .await?;
                            generation_ids_to_close.push(generation_id.clone());
                            if let Some(ref lineage_id) = execution.session_lineage_id {
                                sessions::set_active_generation_tx(&mut tx, lineage_id, None)
                                    .await?;
                                sessions::insert_event_tx(
                                    &mut tx,
                                    &domain::session::SessionEvent {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        lineage_id: lineage_id.to_string(),
                                        generation_id: generation_id.to_string(),
                                        event_type:
                                            domain::session::SessionEventType::OperatorReset,
                                        recorded_at: now,
                                        details_json: Some(
                                            serde_json::json!({ "reason": "operator_reset" })
                                                .to_string(),
                                        ),
                                    },
                                )
                                .await?;
                            }
                        }
                    }
                    stages::update_status_tx(&mut tx, stage.id, StageStatus::Pending).await?;
                }

                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: WorkItemKind::StartupRepair,
                        payload_json: serde_json::json!({
                            "run_id": c.run_id.to_string(),
                            "stage_id": c.stage_id.clone()
                        })
                        .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(c.run_id),
                        stage_id: Some(c.stage_id.clone()),
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.ResetSession",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.ResetSession", tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);

                if let Some(acp) = &self.acp {
                    for generation_id in generation_ids_to_close {
                        let _ = acp.close_session(&generation_id).await;
                    }
                }

                // Refresh projections so reads reflect the reset.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::SessionReset {
                    run_id: c.run_id,
                    stage_id: c.stage_id,
                })
            }

            // ── P072: Converged approval resolution by approval_id ──────
            Command::ResolveApproval(c) => {
                use domain::commands::ApprovalResolutionDecision;

                let now = Utc::now();
                let decision = match c.decision {
                    ApprovalResolutionDecision::Approved => ApprovalDecision::Granted,
                    ApprovalResolutionDecision::Rejected => ApprovalDecision::Rejected,
                };

                let has_post_tasks = if decision == ApprovalDecision::Granted {
                    self.check_has_post_approval_tasks(c.run_id, &c.stage_id)
                        .await
                } else {
                    false
                };

                let tx_started = Instant::now();
                let mut tx =
                    db::pool::begin_immediate_with_retry(&self.pool, "command.ResolveApproval")
                        .await?;
                command_journal::record_tx(
                    &mut tx,
                    &journal.id,
                    journal.command_type,
                    &journal.payload_json,
                    journal.run_id.as_deref(),
                    journal.created_at,
                    journal.caller_surface.as_deref(),
                    journal.caller_principal_id.as_deref(),
                    journal.caller_principal_class.as_deref(),
                    journal.caller_tool.as_deref(),
                    journal.request_id.as_deref(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

                // Verify the approval exists and is still actionable.
                let approval = approvals::find_by_id_tx(&mut tx, c.approval_id).await?;
                let approval = match approval {
                    Some(a)
                        if matches!(
                            a.decision,
                            ApprovalDecision::Pending | ApprovalDecision::Requested
                        ) =>
                    {
                        a
                    }
                    Some(_) => {
                        let error = anyhow!(
                            "Approval {} is not actionable (already resolved)",
                            c.approval_id
                        );
                        command_journal::fail_entry_tx(
                            &mut tx,
                            &journal.id,
                            Utc::now(),
                            &error.to_string(),
                        )
                        .await?;
                        tx.commit().await?;
                        db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                        return Err(error);
                    }
                    None => {
                        let error = anyhow!("Approval {} not found", c.approval_id);
                        command_journal::fail_entry_tx(
                            &mut tx,
                            &journal.id,
                            Utc::now(),
                            &error.to_string(),
                        )
                        .await?;
                        tx.commit().await?;
                        db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                        return Err(error);
                    }
                };
                if approval.run_id != c.run_id || approval.stage_id != c.stage_id {
                    let error = anyhow!(
                        "Approval {} provenance mismatch: command run/stage {}:{} but approval belongs to {}:{}",
                        c.approval_id,
                        c.run_id,
                        c.stage_id,
                        approval.run_id,
                        approval.stage_id
                    );
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &error.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                    return Err(error);
                }
                let authoritative_run_id = approval.run_id;
                let authoritative_stage_id = approval.stage_id.clone();

                approvals::resolve_tx(&mut tx, approval.id, decision.clone(), now, c.rationale)
                    .await?;

                let mut stage_status_event = None;
                let mut should_enqueue_advance = decision == ApprovalDecision::Granted;
                let run_stages = stages::list_by_run_tx(&mut tx, authoritative_run_id).await?;

                if decision == ApprovalDecision::Granted {
                    if let Some(stage) = run_stages.iter().find(|s| {
                        s.stage_id == authoritative_stage_id
                            && s.status == StageStatus::WaitingApproval
                    }) {
                        if stage.stage_type.as_deref() == Some("manual_gate") {
                            if has_post_tasks {
                                stages::update_status_tx(&mut tx, stage.id, StageStatus::Running)
                                    .await?;
                                stage_status_event = Some((stage.id, StageStatus::Running));
                            } else {
                                stages::settle_tx(
                                    &mut tx,
                                    stage.id,
                                    StageSettlementKind::Completed,
                                    now,
                                )
                                .await?;
                                stage_status_event = Some((stage.id, StageStatus::Completed));
                            }
                        } else {
                            stages::update_status_tx(&mut tx, stage.id, StageStatus::Running)
                                .await?;
                            stage_status_event = Some((stage.id, StageStatus::Running));
                        }
                    }
                } else {
                    // Rejection path — mirrors RejectStage logic.
                    if let Some(stage) = run_stages.iter().find(|s| {
                        s.stage_id == authoritative_stage_id
                            && s.status == StageStatus::WaitingApproval
                    }) {
                        if stage.stage_type.as_deref() == Some("manual_gate") {
                            stages::settle_tx(
                                &mut tx,
                                stage.id,
                                StageSettlementKind::Completed,
                                now,
                            )
                            .await?;
                            stage_status_event = Some((stage.id, StageStatus::Completed));
                            should_enqueue_advance = true;
                            if stage.stage_id == "state_11_manual_release" {
                                sqlx::query(
                                    "UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3",
                                )
                                .bind(RunStatus::Running.to_string())
                                .bind("state_10_implementation_refined")
                                .bind(authoritative_run_id.to_string())
                                .execute(&mut *tx)
                                .await?;
                                supersede_current_workflow_conflict_for_manual_release_rejection_tx(
                                    &mut tx,
                                    authoritative_run_id,
                                    &stage.stage_id,
                                    now,
                                    &journal.id,
                                )
                                .await?;
                            }
                        } else {
                            stages::update_status_tx(&mut tx, stage.id, StageStatus::Blocked)
                                .await?;
                            stage_status_event = Some((stage.id, StageStatus::Blocked));
                        }
                    }
                }

                if should_enqueue_advance {
                    work_items::enqueue_tx(
                        &mut tx,
                        &WorkItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            kind: WorkItemKind::AdvanceRun,
                            payload_json: serde_json::json!({
                                "run_id": authoritative_run_id.to_string()
                            })
                            .to_string(),
                            status: WorkItemStatus::Pending,
                            run_id: Some(authoritative_run_id),
                            stage_id: None,
                            created_at: now,
                            scheduled_at: now,
                            attempt_count: 0,
                            last_error: None,
                        },
                    )
                    .await?;
                }
                let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                    &mut tx,
                    &self.capacity_config,
                    now,
                    "command.ResolveApproval",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                if let Some((stage_execution_id, status)) = stage_status_event {
                    let _ = self.events.send(DomainEvent::StageStatusChanged {
                        run_id: authoritative_run_id,
                        stage_execution_id,
                        status,
                    });
                }
                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision,
                });
                projections::rebuild_all_for_run(&self.pool, authoritative_run_id).await?;

                let result = match c.decision {
                    ApprovalResolutionDecision::Approved => CommandResult::StageApproved {
                        approval_id: approval.id,
                    },
                    ApprovalResolutionDecision::Rejected => CommandResult::StageRejected {
                        approval_id: approval.id,
                    },
                };
                Ok(result)
            }
        }
    }

    async fn retry_stage_latest_attempt(
        &self,
        run_id: RunId,
        stage_id: &str,
        consume_quota_budget_now: bool,
        journal_id: &str,
        journal: &CommandJournalEntry,
        retry_reason: &str,
        validated_instruction: Option<&str>,
        caller: &CallerContext,
    ) -> Result<CommandResult> {
        let run_stages = stages::list_by_run(&self.pool, run_id).await?;
        let matching_stages = run_stages
            .iter()
            .filter(|s| s.stage_id == stage_id)
            .collect::<Vec<_>>();
        let old_stage = matching_stages
            .iter()
            .copied()
            .max_by_key(|s| s.started_at)
            .ok_or_else(|| anyhow!("Stage {} not found", stage_id))?;
        let completed_current_stage_on_blocked_run = if old_stage.status == StageStatus::Completed {
            let run = runs::find_by_id(&self.pool, run_id)
                .await?
                .ok_or_else(|| anyhow!("Run {} not found", run_id))?;
            run.status == RunStatus::Blocked
                && (run.current_state.as_deref() == Some(stage_id)
                    || old_stage.stage_id == stage_id)
        } else {
            false
        };

        if !matches!(old_stage.status, StageStatus::Failed | StageStatus::Blocked)
            && !completed_current_stage_on_blocked_run
        {
            return Err(anyhow!(
                "Stage {} latest attempt is {} and cannot be retried yet",
                stage_id,
                old_stage.status
            ));
        }
        let next_attempt_number = matching_stages
            .iter()
            .map(|s| s.attempt_number)
            .max()
            .unwrap_or(old_stage.attempt_number)
            + 1;

        let now = Utc::now();
        let new_stage = StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: old_stage.stage_id.clone(),
            label: old_stage.label.clone(),
            status: StageStatus::Pending,
            iteration: old_stage.iteration,
            attempt_number: next_attempt_number,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: old_stage.owner_agent.clone(),
            provider: old_stage.provider.clone(),
            model: old_stage.model.clone(),
            stage_type: old_stage.stage_type.clone(),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: Some(retry_reason.into()),
        };
        let retry_advance_work_item_id = new_stage.id.to_string();
        let retry_invoke_work_item_id = format!("p058-invoke:{}:0", new_stage.id);
        let retry_tx_started = Instant::now();
        let mut retry_tx =
            db::pool::begin_immediate_with_retry(&self.pool, "command.RetryStage").await?;
        command_journal::record_tx(
            &mut retry_tx,
            &journal.id,
            journal.command_type,
            &journal.payload_json,
            journal.run_id.as_deref(),
            journal.created_at,
            journal.caller_surface.as_deref(),
            journal.caller_principal_id.as_deref(),
            journal.caller_principal_class.as_deref(),
            journal.caller_tool.as_deref(),
            journal.request_id.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        apply_quota_retry_budget_for_stage_tx(
            &mut retry_tx,
            run_id,
            old_stage.id,
            consume_quota_budget_now,
            journal_id,
        )
        .await?;
        stages::settle_tx(
            &mut retry_tx,
            old_stage.id,
            StageSettlementKind::Skipped,
            now,
        )
        .await?;
        stages::insert_tx(&mut retry_tx, &new_stage).await?;
        artifact_contracts::mark_active_claims_superseded_pending_retry_for_stage_tx(
            &mut retry_tx,
            run_id,
            &old_stage.id.to_string(),
            &retry_invoke_work_item_id,
            journal_id,
        )
        .await?;
        sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
            .bind(RunStatus::Running.to_string())
            .bind(stage_id)
            .bind(run_id.to_string())
            .execute(&mut *retry_tx)
            .await?;
        supersede_current_workflow_conflict_for_stage_retry_tx(
            &mut retry_tx,
            run_id,
            stage_id,
            now,
            journal_id,
        )
        .await?;
        // P065: create binding for fallback full-stage retry path
        let retry_instruction_binding_id = if let Some(instruction_text) = validated_instruction {
            let scope_kind = if retry_reason == "operator_retry_stale_targeted_retry" {
                domain::retry_instruction::RetryInstructionScopeKind::TargetedRetryFallbackFullStage
            } else {
                domain::retry_instruction::RetryInstructionScopeKind::FullStageRetry
            };
            let binding = retry_operator_instructions::create_for_retry_attempt_tx(
                &mut retry_tx,
                &domain::retry_instruction::RetryInstructionBindingInput {
                    journal_id: journal_id.to_string(),
                    run_id,
                    stage_id: stage_id.to_string(),
                    source_stage_execution_id: old_stage.id,
                    retry_stage_execution_id: new_stage.id,
                    retry_attempt_number: next_attempt_number,
                    target_agent_execution_id: None,
                    scope_kind,
                    instruction_text: instruction_text.to_string(),
                    created_by_principal_id: caller.principal_id.clone(),
                    created_by_principal_class: caller.principal_class.to_string(),
                },
            )
            .await?;
            Some(binding.binding_id)
        } else {
            None
        };
        work_items::enqueue_tx(
            &mut retry_tx,
            &WorkItem {
                id: retry_advance_work_item_id,
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": stage_id
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some(stage_id.to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await?;
        command_journal::complete_entry_tx(&mut retry_tx, &journal.id, Utc::now()).await?;
        retry_tx.commit().await?;
        db::pool::log_write_transaction("command.RetryStage", retry_tx_started);

        // Refresh projections so reads reflect the retry.
        projections::rebuild_all_for_run(&self.pool, run_id).await?;

        Ok(CommandResult::StageRetryScheduled {
            run_id,
            stage_id: stage_id.to_string(),
            legacy_discovery_override_id: None,
            retry_instruction_binding_id,
        })
    }

    async fn retry_agent_execution(
        &self,
        run_id: RunId,
        stage_id: &str,
        agent_execution_id: domain::ids::AgentExecutionId,
        consume_quota_budget_now: bool,
        journal_id: &str,
        journal: &CommandJournalEntry,
        validated_instruction: Option<&str>,
        caller: &CallerContext,
    ) -> Result<CommandResult> {
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow!("Run {} not found", run_id))?;
        if run.status.is_terminal() {
            return Err(anyhow!("Run {} is already in terminal state", run_id));
        }

        let target_exec = agent_executions::find_by_id(&self.pool, agent_execution_id)
            .await?
            .ok_or_else(|| anyhow!("Agent execution {} not found", agent_execution_id))?;
        let old_stage_execution_id = target_exec.stage_execution_id.ok_or_else(|| {
            anyhow!(
                "Agent execution {} is not stage-owned and cannot be retried as a stage",
                agent_execution_id
            )
        })?;
        let old_stage = stages::find_by_id(&self.pool, old_stage_execution_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Stage execution {} for agent execution {} not found",
                    old_stage_execution_id,
                    agent_execution_id
                )
            })?;
        if old_stage.run_id != run_id || old_stage.stage_id != stage_id {
            return Err(anyhow!(
                "Agent execution {} belongs to run {} stage {}, not run {} stage {}",
                agent_execution_id,
                old_stage.run_id,
                old_stage.stage_id,
                run_id,
                stage_id
            ));
        }

        let run_stages = stages::list_by_run(&self.pool, run_id).await?;
        let matching_stages = run_stages
            .iter()
            .filter(|s| s.stage_id == stage_id)
            .collect::<Vec<_>>();
        let latest_stage = matching_stages
            .iter()
            .copied()
            .max_by_key(|s| s.started_at)
            .ok_or_else(|| anyhow!("Stage {} not found", stage_id))?;
        if latest_stage.id != old_stage.id {
            return Err(anyhow!(
                "Agent execution {} is on stale stage execution {}; latest for {} is {}",
                agent_execution_id,
                old_stage.id,
                stage_id,
                latest_stage.id
            ));
        }

        let completed_current_stage_on_blocked_run = old_stage.status == StageStatus::Completed
            && run.status == RunStatus::Blocked
            && (run.current_state.as_deref() == Some(stage_id) || old_stage.stage_id == stage_id);
        if !matches!(old_stage.status, StageStatus::Failed | StageStatus::Blocked)
            && !completed_current_stage_on_blocked_run
        {
            return Err(anyhow!(
                "Stage {} latest attempt is {} and cannot be targeted-retried yet",
                stage_id,
                old_stage.status
            ));
        }

        let run_work_items = work_items::list_by_run(&self.pool, run_id).await?;
        let source_item = find_source_invoke_work_item(
            &run_work_items,
            &old_stage.id.to_string(),
            &target_exec.agent_id,
            &agent_execution_id.to_string(),
        )
        .ok_or_else(|| {
            anyhow!(
                "InvokeAgent work item for agent execution {} not found",
                agent_execution_id
            )
        })?;
        if matches!(
            source_item.status,
            WorkItemStatus::Pending | WorkItemStatus::Running
        ) {
            return Err(anyhow!(
                "Agent execution {} source work item {} is still {}",
                agent_execution_id,
                source_item.id,
                source_item.status
            ));
        }
        if let (Some(acp), Some(generation_id)) = (
            self.acp.as_ref(),
            target_exec.session_generation_id.as_deref(),
        ) {
            if !acp.has_live_session(generation_id, None).await {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    agent_execution_id = %agent_execution_id,
                    generation_id = %generation_id,
                    source_work_item_id = %source_item.id,
                    source_work_item_status = %source_item.status,
                    "Targeted retry source ACP generation is no longer live; creating a fresh targeted retry from persisted payload"
                );
            }
        }

        let mut retry_payload: serde_json::Value = serde_json::from_str(&source_item.payload_json)
            .map_err(|e| {
                anyhow!(
                    "Source InvokeAgent work item {} has invalid payload: {}",
                    source_item.id,
                    e
                )
            })?;
        let runtime_facts =
            agent_execution_runtime_facts::find_by_execution_id(&self.pool, agent_execution_id)
                .await?;
        let provider_fallback = targeted_retry_provider_fallback(
            &run,
            &target_exec.agent_id,
            &retry_payload,
            runtime_facts.as_ref(),
        );
        let next_attempt_number = matching_stages
            .iter()
            .map(|s| s.attempt_number)
            .max()
            .unwrap_or(old_stage.attempt_number)
            + 1;
        let now = Utc::now();
        let new_stage = StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: old_stage.stage_id.clone(),
            label: old_stage.label.clone(),
            status: StageStatus::Running,
            iteration: old_stage.iteration,
            attempt_number: next_attempt_number,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: old_stage.owner_agent.clone(),
            provider: old_stage.provider.clone(),
            model: old_stage.model.clone(),
            stage_type: old_stage.stage_type.clone(),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: Some(format!("operator_targeted_retry:{}", target_exec.agent_id)),
        };
        let retry_work_item_id = format!(
            "p058-targeted-retry:{}:{}",
            new_stage.id, agent_execution_id
        );
        if let Some(object) = retry_payload.as_object_mut() {
            object.insert("run_id".into(), serde_json::json!(run_id.to_string()));
            object.insert("stage_id".into(), serde_json::json!(stage_id));
            object.insert(
                "stage_execution_id".into(),
                serde_json::json!(new_stage.id.to_string()),
            );
            object.remove("p058_claimed");
            object.insert(
                "targeted_retry".into(),
                serde_json::json!({
                    "journal_id": journal_id,
                    "source_stage_execution_id": old_stage.id.to_string(),
                    "source_agent_execution_id": agent_execution_id.to_string(),
                    "source_work_item_id": source_item.id,
                    "reason": "operator_targeted_retry"
                }),
            );
            if let Some(fallback) = provider_fallback {
                object.insert(
                    "backend_profile_id".into(),
                    serde_json::json!(fallback.backend_profile_id.clone()),
                );
                object.insert(
                    "provider".into(),
                    serde_json::json!(fallback.provider.clone()),
                );
                object.insert("model".into(), serde_json::json!(fallback.model.clone()));
                if let Some(effort) = fallback.effort.clone() {
                    object.insert("effort".into(), serde_json::json!(effort));
                }
                if let Some(max_turns) = fallback.max_turns {
                    object.insert("max_turns".into(), serde_json::json!(max_turns));
                }
                if let Some(targeted_retry) = object
                    .get_mut("targeted_retry")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    targeted_retry.insert(
                        "provider_fallback".into(),
                        serde_json::json!({
                            "reason": "source_provider_failed_without_required_output",
                            "from_backend_profile_id": fallback.from_backend_profile_id,
                            "from_provider": fallback.from_provider,
                            "to_backend_profile_id": fallback.backend_profile_id,
                            "to_provider": fallback.provider,
                        }),
                    );
                }
            }
        } else {
            return Err(anyhow!(
                "Source InvokeAgent work item {} payload is not a JSON object",
                source_item.id
            ));
        }

        let retry_tx_started = Instant::now();
        let mut retry_tx =
            db::pool::begin_immediate_with_retry(&self.pool, "command.RetryAgentExecution").await?;
        command_journal::record_tx(
            &mut retry_tx,
            &journal.id,
            journal.command_type,
            &journal.payload_json,
            journal.run_id.as_deref(),
            journal.created_at,
            journal.caller_surface.as_deref(),
            journal.caller_principal_id.as_deref(),
            journal.caller_principal_class.as_deref(),
            journal.caller_tool.as_deref(),
            journal.request_id.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        apply_quota_retry_budget_for_stage_tx(
            &mut retry_tx,
            run_id,
            old_stage.id,
            consume_quota_budget_now,
            journal_id,
        )
        .await?;
        stages::settle_tx(
            &mut retry_tx,
            old_stage.id,
            StageSettlementKind::Skipped,
            now,
        )
        .await?;
        stages::insert_tx(&mut retry_tx, &new_stage).await?;
        sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
            .bind(RunStatus::Running.to_string())
            .bind(stage_id)
            .bind(run_id.to_string())
            .execute(&mut *retry_tx)
            .await?;
        // P065: create parent binding + child delivery for targeted retry
        let retry_instruction_binding_id = if let Some(instruction_text) = validated_instruction {
            let binding = retry_operator_instructions::create_for_retry_attempt_tx(
                &mut retry_tx,
                &domain::retry_instruction::RetryInstructionBindingInput {
                    journal_id: journal_id.to_string(),
                    run_id,
                    stage_id: stage_id.to_string(),
                    source_stage_execution_id: old_stage.id,
                    retry_stage_execution_id: new_stage.id,
                    retry_attempt_number: next_attempt_number,
                    target_agent_execution_id: Some(agent_execution_id),
                    scope_kind: domain::retry_instruction::RetryInstructionScopeKind::TargetedRetry,
                    instruction_text: instruction_text.to_string(),
                    created_by_principal_id: caller.principal_id.clone(),
                    created_by_principal_class: caller.principal_class.to_string(),
                },
            )
            .await?;
            // For targeted retry, the work item is known now — create child delivery row.
            retry_operator_instructions::create_for_work_item_tx(
                &mut retry_tx,
                &binding.binding_id,
                Some(&retry_work_item_id),
                None,
            )
            .await?;
            // Inject metadata into the payload so executor can find it.
            if let Some(object) = retry_payload.as_object_mut() {
                object.insert(
                    "operator_retry_instruction".into(),
                    serde_json::json!({
                        "binding_id": binding.binding_id,
                        "journal_id": binding.journal_id,
                        "scope_kind": binding.scope_kind.to_string(),
                        "instruction": binding.instruction_text,
                        "instruction_sha256": binding.instruction_sha256,
                    }),
                );
            }
            Some(binding.binding_id)
        } else {
            None
        };
        work_items::enqueue_tx(
            &mut retry_tx,
            &WorkItem {
                id: retry_work_item_id,
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::to_string(&retry_payload)?,
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some(stage_id.to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await?;
        command_journal::complete_entry_tx(&mut retry_tx, &journal.id, Utc::now()).await?;
        retry_tx.commit().await?;
        db::pool::log_write_transaction("command.RetryAgentExecution", retry_tx_started);

        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: old_stage.id,
            status: StageStatus::Skipped,
        });
        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: new_stage.id,
            status: StageStatus::Running,
        });
        let _ = self.events.send(DomainEvent::RunStatusChanged {
            run_id,
            status: RunStatus::Running,
        });

        projections::rebuild_all_for_run(&self.pool, run_id).await?;

        Ok(CommandResult::StageRetryScheduled {
            run_id,
            stage_id: stage_id.to_string(),
            legacy_discovery_override_id: None,
            retry_instruction_binding_id,
        })
    }

    async fn record_completed_command_transaction(
        &self,
        journal: &CommandJournalEntry,
        context: &'static str,
    ) -> Result<()> {
        let tx_started = Instant::now();
        let mut tx = db::pool::begin_immediate_with_retry(&self.pool, context).await?;
        command_journal::record_tx(
            &mut tx,
            &journal.id,
            journal.command_type,
            &journal.payload_json,
            journal.run_id.as_deref(),
            journal.created_at,
            journal.caller_surface.as_deref(),
            journal.caller_principal_id.as_deref(),
            journal.caller_principal_class.as_deref(),
            journal.caller_tool.as_deref(),
            journal.request_id.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
        tx.commit().await?;
        db::pool::log_write_transaction(context, tx_started);
        Ok(())
    }

    async fn record_failed_command_transaction(
        &self,
        journal: &CommandJournalEntry,
        context: &'static str,
        error: &str,
    ) -> Result<()> {
        let tx_started = Instant::now();
        let mut tx = db::pool::begin_immediate_with_retry(&self.pool, context).await?;
        command_journal::record_tx(
            &mut tx,
            &journal.id,
            journal.command_type,
            &journal.payload_json,
            journal.run_id.as_deref(),
            journal.created_at,
            journal.caller_surface.as_deref(),
            journal.caller_principal_id.as_deref(),
            journal.caller_principal_class.as_deref(),
            journal.caller_tool.as_deref(),
            journal.request_id.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        command_journal::fail_entry_tx(&mut tx, &journal.id, Utc::now(), error).await?;
        tx.commit().await?;
        db::pool::log_write_transaction(context, tx_started);
        Ok(())
    }

    /// P044 §3d helper: Check whether the workflow plan for the given run has
    /// `post_approval_tasks` on the state identified by `stage_id`.
    ///
    /// Returns `false` on any error (run not found, missing paths, plan compile
    /// failure, state not found) so that the caller falls back to the existing
    /// "settle as Completed" behaviour.
    async fn check_has_post_approval_tasks(&self, run_id: RunId, stage_id: &str) -> bool {
        let run = match runs::find_by_id(&self.pool, run_id).await {
            Ok(Some(r)) => r,
            _ => {
                warn!(run_id = %run_id, "check_has_post_approval_tasks: run not found");
                return false;
            }
        };

        let workflow_path = match run.workflow_yaml_path.as_deref() {
            Some(p) => p,
            None => return false,
        };
        let catalog_path = match run.agent_catalog_yaml_path.as_deref() {
            Some(p) => p,
            None => return false,
        };

        let plan = match workflow::compiler::compile(workflow_path, catalog_path) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    error = %e,
                    "check_has_post_approval_tasks: failed to compile plan"
                );
                return false;
            }
        };

        match plan.states.get(stage_id) {
            Some(state) => !state.post_approval_tasks.is_empty(),
            None => {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    "check_has_post_approval_tasks: state not found in plan"
                );
                false
            }
        }
    }
}

async fn apply_quota_retry_budget_for_stage_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: RunId,
    stage_execution_id: domain::ids::StageExecutionId,
    consume_quota_budget_now: bool,
    journal_id: &str,
) -> Result<()> {
    let now = Utc::now();
    let ledgers =
        agent_retry_budget_ledger::list_quota_for_stage_tx(tx, run_id, stage_execution_id).await?;
    for ledger in ledgers {
        if ledger.normal_budget_consumed {
            continue;
        }
        match ledger.retry_after {
            Some(retry_after) if retry_after > now => {
                if !consume_quota_budget_now {
                    return Err(anyhow!(
                        "quota retry_after has not elapsed for stage {}; retry after {} or set consume_quota_budget_now=true",
                        stage_execution_id,
                        retry_after.to_rfc3339()
                    ));
                }
                agent_retry_budget_ledger::consume_early_quota_retry_tx(tx, &ledger.id, journal_id)
                    .await?;
            }
            _ => {
                agent_retry_budget_ledger::mark_quota_reset_elapsed_tx(tx, &ledger.id).await?;
            }
        }
    }
    Ok(())
}

fn phase_b_dogfood_exit_metric_snapshot(
    workspace_root: &str,
) -> Option<PhaseBDogfoodMetricSnapshot> {
    let path = Path::new(workspace_root)
        .join("docs/reference/workflow-conflict-evidence/phase-b-dogfood-exit-record.json");
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let gate_results = payload.get("gate_results")?;
    let sample_size = gate_results.get("sample_size")?.as_i64()?;
    let completion_rate = gate_results.get("completion_rate_observed")?.as_f64()?;
    let guidance_rate = gate_results
        .get("operator_guidance_sufficient_rate")?
        .as_f64()?;
    let evidence_source = payload
        .get("record_id")
        .and_then(|value| value.as_str())
        .unwrap_or("phase_b_dogfood_exit_record")
        .to_string();

    Some(PhaseBDogfoodMetricSnapshot {
        completion_rate,
        sample_size,
        guidance_sufficient_count: (guidance_rate * sample_size as f64).round() as i64,
        evidence_source,
    })
}

fn resolve_start_run_review_routing_json(
    explicit_json: Option<&str>,
    idea_body: &str,
    operator_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<String> {
    if let Some(json) = explicit_json {
        let opts: domain::routing::ReviewRoutingOptions =
            serde_json::from_str(json).map_err(|error| anyhow!("{error}"))?;
        validate_review_routing_options(&opts)?;
        return Ok(serde_json::to_string(&opts).unwrap_or_else(|_| json.to_string()));
    }

    let mut opts = domain::routing::ReviewRoutingOptions::default();
    let mut has_hint = false;
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(idea_body) {
        if let Some(mode) = yaml_lookup_string(&yaml, &["idea.review_mode"])
            .or_else(|| yaml_lookup_string(&yaml, &["idea", "review_mode"]))
        {
            opts.mode = mode
                .parse::<domain::routing::ReviewRoutingMode>()
                .map_err(|error| anyhow!("{error}"))?;
            has_hint = true;
        }

        if let Some(override_node) = yaml_lookup(&yaml, &["reviewer_override"]) {
            opts.force_include = yaml_lookup_string_list(override_node, &["force_include"]);
            opts.force_exclude = yaml_lookup_string_list(override_node, &["force_exclude"]);
            opts.override_reason = yaml_lookup_string(override_node, &["reason"]);
            has_hint = true;
        }
    }

    validate_review_routing_options(&opts)?;
    if has_hint
        && (opts.override_reason.is_some()
            || !opts.force_include.is_empty()
            || !opts.force_exclude.is_empty())
    {
        opts.operator_id = operator_id.map(str::to_string);
        opts.created_at = Some(now);
    }

    serde_json::to_string(&opts).map_err(Into::into)
}

fn validate_review_routing_options(opts: &domain::routing::ReviewRoutingOptions) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for agent_id in opts.force_include.iter().chain(opts.force_exclude.iter()) {
        if !seen.insert(agent_id.as_str()) {
            return Err(anyhow!(
                "duplicate agent_id '{agent_id}' in force_include/force_exclude"
            ));
        }
    }
    Ok(())
}

fn yaml_lookup<'a>(value: &'a serde_yaml::Value, path: &[&str]) -> Option<&'a serde_yaml::Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(serde_yaml::Value::String((*key).to_string()))?;
    }
    Some(cursor)
}

fn yaml_lookup_string(value: &serde_yaml::Value, path: &[&str]) -> Option<String> {
    yaml_lookup(value, path)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn yaml_lookup_string_list(value: &serde_yaml::Value, path: &[&str]) -> Vec<String> {
    let Some(value) = yaml_lookup(value, path) else {
        return Vec::new();
    };
    if let Some(sequence) = value.as_sequence() {
        return sequence
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect();
    }
    value
        .as_str()
        .map(|item| {
            item.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

async fn supersede_current_workflow_conflict_for_stage_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    now: DateTime<Utc>,
    journal_id: &str,
) -> Result<()> {
    let Some(conflict) = workflow_conflicts::get_current_blocking_conflict_tx(tx, run_id).await?
    else {
        return Ok(());
    };

    if conflict.current_state_id != stage_id {
        return Ok(());
    }

    workflow_conflicts::transition_conflict_status_tx(
        tx,
        &conflict.conflict_id,
        WorkflowConflictStatus::Superseded,
        now,
        Some(serde_json::json!({
            "resolution_kind": "operator_stage_retry",
            "stage_id": stage_id,
            "journal_id": journal_id,
        })),
        None,
        None,
    )
    .await?;

    workflow_conflicts::upsert_transition_cursor_tx(
        tx,
        &WorkflowTransitionCursorRecord {
            schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: stage_id.to_string(),
            cursor_status: "stage_retry_scheduled".to_string(),
            resume_policy: "continue_from_selected_transition".to_string(),
            selected_transition_id: None,
            selected_next_state_id: Some(stage_id.to_string()),
            conflict_id: Some(conflict.conflict_id),
            conflict_fingerprint: Some(conflict.conflict_fingerprint),
            candidate_transition_hash: Some(conflict.candidate_transition_hash),
            terminal_failure_reason: None,
            updated_at: now,
        },
    )
    .await?;

    Ok(())
}

async fn supersede_current_workflow_conflict_for_manual_release_rejection_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    now: DateTime<Utc>,
    journal_id: &str,
) -> Result<()> {
    let Some(conflict) = workflow_conflicts::get_current_blocking_conflict_tx(tx, run_id).await?
    else {
        return Ok(());
    };

    if conflict.current_state_id != stage_id {
        return Ok(());
    }

    workflow_conflicts::transition_conflict_status_tx(
        tx,
        &conflict.conflict_id,
        WorkflowConflictStatus::Superseded,
        now,
        Some(serde_json::json!({
            "resolution_kind": "manual_release_rejection_loopback",
            "from_stage_id": stage_id,
            "selected_next_state_id": "state_10_implementation_refined",
            "journal_id": journal_id,
        })),
        None,
        None,
    )
    .await?;

    workflow_conflicts::upsert_transition_cursor_tx(
        tx,
        &WorkflowTransitionCursorRecord {
            schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: stage_id.to_string(),
            cursor_status: "manual_release_rejection_loopback".to_string(),
            resume_policy: "continue_from_selected_transition".to_string(),
            selected_transition_id: None,
            selected_next_state_id: Some("state_10_implementation_refined".to_string()),
            conflict_id: Some(conflict.conflict_id),
            conflict_fingerprint: Some(conflict.conflict_fingerprint),
            candidate_transition_hash: Some(conflict.candidate_transition_hash),
            terminal_failure_reason: None,
            updated_at: now,
        },
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p060_idea_body_review_mode_and_reviewer_override_are_canonicalized() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-28T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let idea_body = r#"
idea.review_mode: legacy_fixed
reviewer_override:
  force_include: [proposal_reviewer_security]
  force_exclude: [proposal_reviewer_ui]
  reason: "Security-sensitive internal API; no UI surface."
"#;

        let json = resolve_start_run_review_routing_json(None, idea_body, Some("operator-1"), now)
            .expect("idea-level P060 routing hints should parse");
        let options: domain::routing::ReviewRoutingOptions =
            serde_json::from_str(&json).expect("canonical ReviewRoutingOptions JSON");

        assert_eq!(
            options.mode,
            domain::routing::ReviewRoutingMode::LegacyFixed
        );
        assert_eq!(options.force_include, vec!["proposal_reviewer_security"]);
        assert_eq!(options.force_exclude, vec!["proposal_reviewer_ui"]);
        assert_eq!(
            options.override_reason.as_deref(),
            Some("Security-sensitive internal API; no UI surface.")
        );
        assert_eq!(options.operator_id.as_deref(), Some("operator-1"));
        assert_eq!(options.created_at, Some(now));
    }

    #[test]
    fn p060_explicit_review_routing_json_wins_over_idea_body_hints() {
        let now = Utc::now();
        let explicit = serde_json::json!({
            "mode": "dynamic",
            "force_include": ["proposal_reviewer_api_contract"],
            "override_reason": "Explicit run-start routing"
        })
        .to_string();

        let json = resolve_start_run_review_routing_json(
            Some(&explicit),
            "idea.review_mode: legacy_fixed",
            Some("operator-1"),
            now,
        )
        .expect("explicit routing JSON should canonicalize");
        let options: domain::routing::ReviewRoutingOptions =
            serde_json::from_str(&json).expect("canonical ReviewRoutingOptions JSON");

        assert_eq!(options.mode, domain::routing::ReviewRoutingMode::Dynamic);
        assert_eq!(
            options.force_include,
            vec!["proposal_reviewer_api_contract"]
        );
        assert_eq!(
            options.override_reason.as_deref(),
            Some("Explicit run-start routing")
        );
        assert_eq!(options.operator_id, None);
        assert_eq!(options.created_at, None);
    }

    #[test]
    fn p060_review_routing_duplicate_override_ids_are_rejected() {
        let now = Utc::now();
        let duplicate = serde_json::json!({
            "mode": "dynamic",
            "force_include": ["proposal_reviewer_security"],
            "force_exclude": ["proposal_reviewer_security"]
        })
        .to_string();

        let err = resolve_start_run_review_routing_json(Some(&duplicate), "", None, now)
            .expect_err("duplicate include/exclude IDs should fail validation");
        assert!(err.to_string().contains("duplicate agent_id"));
    }

    #[test]
    fn p017_phase_b_dogfood_metric_snapshot_reads_evidence_record() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let snapshot = phase_b_dogfood_exit_metric_snapshot(&workspace_root.to_string_lossy())
            .expect("P017 Phase B dogfood evidence snapshot should parse");

        assert_eq!(snapshot.sample_size, 10);
        assert!((snapshot.completion_rate - 1.0).abs() < 1e-6);
        assert_eq!(snapshot.guidance_sufficient_count, 10);
        assert_eq!(
            snapshot.evidence_source,
            "p017-phase-b-dogfood-exit-2026-04-26"
        );
    }
}
