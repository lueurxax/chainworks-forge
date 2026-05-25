use acp::AcpRuntimeManager;
use anyhow::{anyhow, Context, Result};
use auth;
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error as ThisError;
use tracing::{info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger,
    approval_mutation_idempotency, approvals, artifact_contracts, audit_log, closeout,
    code_writer_completion_receipts, command_journal, ideas, legacy_discovery_overrides,
    mcp_command_idempotency, projections, retry_operator_instructions,
    retry_stage_execution_authorities, runs, scheduler, sessions, stages, work_items,
    workflow_conflicts,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter};
use domain::agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement};
use domain::approval::ApprovalDecision;
use domain::closeout_readiness_mode::resolve_closeout_readiness_mode;
use domain::commands::{
    ApprovalResolutionDecision, CallerContext, Command, ExtendWorkflowLoopBudgetCmd,
    ProposalGateSettlementAction, SettleProposalGateCmd, WorkflowLoopBudgetExtensionCmd,
};
use domain::discovery::{LegacyBroadDiscoveryPolicy, LegacyDiscoveryOverrideInput};
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::proposal_gate_result::{
    ProposalGateFailureClassification, ProposalGateLineage, ProposalGateResult, ProposalGateStatus,
};
use domain::provider::InvokeAgentCapacityConfig;
use domain::retry_authority::{
    RetryAuthorityEntryKind, RetryAuthorityState, RetryStageExecutionAuthority,
};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use domain::workflow_conflict::{
    CandidateTransitionEvaluation, CandidateTransitionResult, WorkflowConflictStatus,
    WorkflowTransitionCursorRecord,
};
use domain::PrincipalClass;
use sha2::{Digest, Sha256};

use crate::cancellation;
use crate::closeout_fingerprint::{
    build_closeout_fingerprint, resolve_closeout_worktree_truth,
    CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS,
};
use crate::closeout_loop_budget::closeout_loop_budget_remaining;
use crate::event_bus::EventSender;
use crate::preflight::{
    missing_delivery_configuration_preflight, run_delivery_preflight, DeliveryPreflightResult,
};
use crate::side_effects::{retry_preflight_within_tx, run_cancel_preflight_within_tx};
use crate::synthesizers::closeout_readiness::{
    synthesize_implementation_closeout_readiness_for_state9, SynthesizerInputs,
};
use crate::work_queue::WorkQueue;

pub struct CommandHandler {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
    db_writer: Arc<DbWriter>,
    acp: Option<Arc<AcpRuntimeManager>>,
    capacity_config: Arc<InvokeAgentCapacityConfig>,
    retry_stage_failure_injection: Option<Arc<dyn Fn(&str) -> Result<()> + Send + Sync>>,
    /// P081 Phase 3: shared immutable boundary policy injected at daemon startup.
    /// Used to record policy mode and fixture version in audit_log entries written
    /// inside command transactions.
    boundary_policy: Option<Arc<auth::boundary::BoundaryPolicy>>,
}

pub enum CommandResult {
    IdeaCreated {
        idea: domain::idea::Idea,
    },
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
    WorkflowLoopBudgetExtended {
        run_id: RunId,
        counter: String,
        previous_max: u64,
        new_max: u64,
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
    /// P077: gate settled and closeout transaction committed.
    ProposalGateSettled {
        run_id: RunId,
        gate_id: String,
        journal_id: String,
        gate_generation_id: String,
        readiness_generation_id: String,
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

/// Sentinel returned by the inner settlement-transaction idempotency check when a
/// concurrent request already committed the same key between the outer pre-check and
/// the settlement transaction. The caller in `handle` catches this and returns the
/// original `Commanded { journal_id: command_journal_id }` without writing any new
/// journal completion entry. This is intentionally private — callers outside this
/// module must not construct or match on it directly.
#[derive(Debug)]
struct ConcurrentIdempotencyRaceReplay {
    command_journal_id: String,
    was_approved: bool,
    approval_id: ApprovalId,
}

impl std::fmt::Display for ConcurrentIdempotencyRaceReplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "concurrent idempotency race replay for journal {}",
            self.command_journal_id
        )
    }
}

impl std::error::Error for ConcurrentIdempotencyRaceReplay {}

#[derive(Debug, ThisError)]
pub enum ApprovalResolutionConflict {
    /// Same caller + approval_id + action + idempotency_key replay after committed success.
    /// The original result is returned without re-settling.
    #[error("Approval {approval_id} is not actionable (already resolved)")]
    AlreadyResolved {
        approval_id: ApprovalId,
        journal_id: String,
    },
    /// Approval is already terminal but the caller supplied a DIFFERENT idempotency key.
    /// Per P081: APPROVAL_NOT_ACTIONABLE with zero settlement side effects.
    #[error("Approval {approval_id} is terminal; a different idempotency key cannot re-settle it")]
    ApprovalNotActionable {
        approval_id: ApprovalId,
        journal_id: String,
    },
}

impl ApprovalResolutionConflict {
    pub fn journal_id(&self) -> &str {
        match self {
            Self::AlreadyResolved { journal_id, .. } => journal_id,
            Self::ApprovalNotActionable { journal_id, .. } => journal_id,
        }
    }
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
    caller_class: Option<String>, // P081 Phase 2 - derived from CallerContext
    // SEC-P081-M002: derived token_id for audit correlation, in-memory only (not persisted to DB).
    token_id: Option<String>,
    // P081 Phase 3: MCP idempotency key and boundary matrix row_id for command_journal linkage.
    mcp_idempotency_key: Option<String>,
    mcp_idempotency_request_hash: Option<String>,
    boundary_row_id: Option<String>,
}

fn ensure_run_meta_root_exists(run: &Run) -> Result<()> {
    let Some(meta_root) = run
        .chainworks_meta_root
        .as_deref()
        .map(str::trim)
        .filter(|root| !root.is_empty())
    else {
        return Ok(());
    };

    let meta_root = Path::new(meta_root);
    let absolute_meta_root = if meta_root.is_absolute() {
        meta_root.to_path_buf()
    } else {
        Path::new(&run.workspace_root).join(meta_root)
    };

    for child in ["", "artifacts", "context", "state", "summaries"] {
        let path = if child.is_empty() {
            absolute_meta_root.clone()
        } else {
            absolute_meta_root.join(child)
        };
        std::fs::create_dir_all(&path)
            .with_context(|| format!("create run meta-root directory {}", path.display()))?;
    }

    Ok(())
}

const PROPOSAL_GATE_EXECUTOR_VERSION: &str = "proposal-gate-executor.v1";
const PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION: &str = "proposal_gate_receipt.v1";
const PROPOSAL_GATE_EXECUTOR_DEFAULT_TIMEOUT_MS: u64 = 120_000;
const PROPOSAL_GATE_EXECUTOR_MAX_TIMEOUT_MS: u64 = 600_000;
const PROPOSAL_GATE_EXECUTOR_POLL_MS: u64 = 25;
const PROPOSAL_GATE_EXECUTOR_TIMEOUT_EXIT_CODE: i32 = 124;
const PROPOSAL_GATE_EXECUTOR_CAPTURE_BUFFER_SIZE: usize = 16 * 1024;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalGateReceiptV1 {
    schema_version: String,
    status: String,
    gate_id: Option<String>,
    proposal_id: String,
    run_id: String,
    stage_id: String,
    executor_version: String,
    evidence_digest: String,
    stdout_digest: String,
    stderr_digest: String,
    exit_code: i32,
    elapsed_ms: u64,
    current_fingerprint: String,
    diagnostic_reason: Option<String>,
    failure_classification: Option<String>,
    source_artifacts: Option<Vec<String>>,
}

struct ManagedProposalGateProcessResult {
    exit_code: i32,
    timed_out: bool,
    elapsed_ms: u64,
    stdout_digest: String,
    stderr_digest: String,
}

fn build_proposal_gate_result_from_settlement(
    c: &SettleProposalGateCmd,
    journal_id: &str,
    gate_id: &str,
    gate_generation_id: &str,
    elapsed_ms: u64,
) -> Result<ProposalGateResult> {
    let has_receipt = c
        .receipt_json
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    // Route Execute and RecordSettlement to ImportReceipt when a receipt is present.
    // Execute without a receipt must only reach this helper after the bounded
    // managed executor has produced a receipt; never fabricate a Passed result.
    let effective_action = if has_receipt
        && matches!(
            c.action,
            ProposalGateSettlementAction::RecordSettlement | ProposalGateSettlementAction::Execute
        ) {
        ProposalGateSettlementAction::ImportReceipt
    } else {
        c.action.clone()
    };

    match effective_action {
        ProposalGateSettlementAction::Execute => {
            anyhow::bail!(
                "ProposalGateSettlementAction::Execute requires the managed executor receipt path"
            )
        }
        ProposalGateSettlementAction::RecordSettlement => build_managed_proposal_gate_result(
            c,
            journal_id,
            gate_id,
            gate_generation_id,
            ProposalGateStatus::Passed,
            None,
            None,
            elapsed_ms,
        ),
        ProposalGateSettlementAction::Waive => build_managed_proposal_gate_result(
            c,
            journal_id,
            gate_id,
            gate_generation_id,
            ProposalGateStatus::Waived,
            Some(c.reason.clone()),
            None,
            elapsed_ms,
        ),
        ProposalGateSettlementAction::ImportReceipt => {
            build_imported_proposal_gate_result(c, journal_id, gate_id, gate_generation_id)
        }
    }
}

fn execute_managed_proposal_gate_receipt(
    c: &SettleProposalGateCmd,
    gate_id: &str,
    execution_root: impl AsRef<Path>,
) -> Result<String> {
    let execution_root = execution_root.as_ref();
    let gate_script = execution_root.join("scripts").join("test-gate.sh");
    if !gate_script.is_file() {
        anyhow::bail!(
            "managed proposal gate executor could not find {}",
            gate_script.display()
        );
    }

    let started = Instant::now();
    let child = ProcessCommand::new(&gate_script)
        .arg("proposal-077")
        .current_dir(execution_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "managed proposal gate executor failed to launch {}",
                gate_script.display()
            )
        })?;
    let process_result = wait_for_managed_proposal_gate(
        child,
        Duration::from_millis(proposal_gate_executor_timeout_ms(c)),
        started,
    )?;
    let elapsed_ms = process_result.elapsed_ms;
    let exit_code = process_result.exit_code;
    let status = if !process_result.timed_out && exit_code == 0 {
        "passed"
    } else {
        "failed"
    };
    let stdout_digest = process_result.stdout_digest.clone();
    let stderr_digest = process_result.stderr_digest.clone();
    let evidence_digest = proposal_gate_executor_evidence_digest(
        c,
        gate_id,
        status,
        exit_code,
        elapsed_ms,
        &stdout_digest,
        &stderr_digest,
    )?;

    let diagnostic_reason = if status == "passed" {
        None
    } else if process_result.timed_out {
        Some(format!(
            "proposal-077 gate timed out after {} ms",
            proposal_gate_executor_timeout_ms(c)
        ))
    } else {
        Some(format!(
            "proposal-077 gate failed with exit code {exit_code}"
        ))
    };
    let failure_classification = if status == "passed" {
        None
    } else if process_result.timed_out {
        Some(
            ProposalGateFailureClassification::UnclearOrNonCodeOwned
                .as_str()
                .to_string(),
        )
    } else {
        Some(
            ProposalGateFailureClassification::CodeOwnedBudgetRemaining
                .as_str()
                .to_string(),
        )
    };

    let receipt = serde_json::json!({
        "schema_version": PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION,
        "status": status,
        "gate_id": gate_id,
        "proposal_id": c.proposal_id.clone(),
        "run_id": c.run_id.to_string(),
        "stage_id": c.stage_id.clone(),
        "executor_version": PROPOSAL_GATE_EXECUTOR_VERSION,
        "evidence_digest": evidence_digest,
        "stdout_digest": stdout_digest,
        "stderr_digest": stderr_digest,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "current_fingerprint": c.current_fingerprint.clone(),
        "diagnostic_reason": diagnostic_reason,
        "failure_classification": failure_classification,
        "source_artifacts": c.source_artifacts.clone(),
    });
    Ok(receipt.to_string())
}

fn proposal_gate_executor_timeout_ms(c: &SettleProposalGateCmd) -> u64 {
    c.timeout_ms
        .unwrap_or(PROPOSAL_GATE_EXECUTOR_DEFAULT_TIMEOUT_MS)
        .clamp(1, PROPOSAL_GATE_EXECUTOR_MAX_TIMEOUT_MS)
}

fn wait_for_managed_proposal_gate(
    mut child: Child,
    timeout: Duration,
    started: Instant,
) -> Result<ManagedProposalGateProcessResult> {
    let stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("managed proposal gate stdout pipe unavailable"))?;
    let stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("managed proposal gate stderr pipe unavailable"))?;
    let stdout_handle = spawn_digest_reader(stdout_reader);
    let stderr_handle = spawn_digest_reader(stderr_reader);

    let (exit_code, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status.code().unwrap_or(1), false);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break (PROPOSAL_GATE_EXECUTOR_TIMEOUT_EXIT_CODE, true);
        }
        std::thread::sleep(Duration::from_millis(PROPOSAL_GATE_EXECUTOR_POLL_MS));
    };

    let stdout_digest = stdout_handle
        .join()
        .map_err(|_| anyhow!("managed proposal gate stdout digest thread panicked"))??;
    let stderr_digest = stderr_handle
        .join()
        .map_err(|_| anyhow!("managed proposal gate stderr digest thread panicked"))??;

    Ok(ManagedProposalGateProcessResult {
        exit_code,
        timed_out,
        elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        stdout_digest,
        stderr_digest,
    })
}

fn spawn_digest_reader<R>(mut reader: R) -> thread::JoinHandle<Result<String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; PROPOSAL_GATE_EXECUTOR_CAPTURE_BUFFER_SIZE];
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        Ok(format!("sha256:{:x}", hasher.finalize()))
    })
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn proposal_gate_executor_evidence_digest(
    c: &SettleProposalGateCmd,
    gate_id: &str,
    status: &str,
    exit_code: i32,
    elapsed_ms: u64,
    stdout_digest: &str,
    stderr_digest: &str,
) -> Result<String> {
    let payload = serde_json::json!({
        "schema_version": PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION,
        "executor_version": PROPOSAL_GATE_EXECUTOR_VERSION,
        "gate_id": gate_id,
        "proposal_id": c.proposal_id.clone(),
        "run_id": c.run_id.to_string(),
        "stage_id": c.stage_id.clone(),
        "status": status,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "stdout_digest": stdout_digest,
        "stderr_digest": stderr_digest,
        "workflow_digest": c.workflow_digest.clone(),
        "worktree_head": c.worktree_head.clone(),
        "dirty_or_changed_file_digest": c.dirty_or_changed_file_digest.clone(),
        "source_generation_ids": c.source_generation_ids.clone(),
        "source_artifacts": c.source_artifacts.clone(),
        "current_fingerprint": c.current_fingerprint.clone(),
    });
    Ok(sha256_digest(&serde_json::to_vec(&payload)?))
}

fn build_managed_proposal_gate_result(
    c: &SettleProposalGateCmd,
    journal_id: &str,
    gate_id: &str,
    gate_generation_id: &str,
    status: ProposalGateStatus,
    diagnostic_reason: Option<String>,
    failure_classification: Option<ProposalGateFailureClassification>,
    elapsed_ms: u64,
) -> Result<ProposalGateResult> {
    let exit_code = 0;
    Ok(ProposalGateResult {
        gate_id: gate_id.to_string(),
        proposal_id: c.proposal_id.clone(),
        run_id: c.run_id.to_string(),
        stage_id: c.stage_id.clone(),
        status: status.clone(),
        generation_id: gate_generation_id.to_string(),
        diagnostic_reason,
        executor_version: Some(PROPOSAL_GATE_EXECUTOR_VERSION.to_string()),
        evidence_digest: Some(proposal_gate_evidence_digest(
            c,
            journal_id,
            gate_generation_id,
            status.as_str(),
            exit_code,
        )?),
        exit_code: Some(exit_code),
        elapsed_ms: Some(elapsed_ms),
        settled_at: Utc::now(),
        authorization_lineage: Some(proposal_gate_lineage(c, journal_id)),
        failure_classification,
    })
}

fn build_imported_proposal_gate_result(
    c: &SettleProposalGateCmd,
    journal_id: &str,
    gate_id: &str,
    gate_generation_id: &str,
) -> Result<ProposalGateResult> {
    let raw = c
        .receipt_json
        .as_deref()
        .ok_or_else(|| anyhow!("import_receipt requires receipt_json"))?;
    let receipt: ProposalGateReceiptV1 = serde_json::from_str(raw)
        .map_err(|e| anyhow!("invalid proposal_gate_receipt.v1 schema: {e}"))?;

    if receipt.schema_version != PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION {
        anyhow::bail!(
            "invalid proposal gate receipt schema_version '{}': expected '{}'",
            receipt.schema_version,
            PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION
        );
    }
    if receipt.proposal_id != c.proposal_id {
        anyhow::bail!("proposal gate receipt proposal_id does not match command");
    }
    if receipt.run_id != c.run_id.to_string() {
        anyhow::bail!("proposal gate receipt run_id does not match command");
    }
    if receipt.stage_id != c.stage_id {
        anyhow::bail!("proposal gate receipt stage_id does not match command");
    }
    if let Some(receipt_gate_id) = receipt.gate_id.as_deref() {
        if receipt_gate_id != gate_id {
            anyhow::bail!("proposal gate receipt gate_id does not match command");
        }
    }
    if receipt.current_fingerprint != c.current_fingerprint {
        anyhow::bail!(
            "proposal gate receipt current_fingerprint does not match command fingerprint"
        );
    }
    if receipt.executor_version != PROPOSAL_GATE_EXECUTOR_VERSION {
        anyhow::bail!(
            "proposal gate receipt executor_version '{}' is not managed by '{}'",
            receipt.executor_version,
            PROPOSAL_GATE_EXECUTOR_VERSION
        );
    }
    validate_sha256_digest("evidence_digest", &receipt.evidence_digest)?;
    validate_sha256_digest("stdout_digest", &receipt.stdout_digest)?;
    validate_sha256_digest("stderr_digest", &receipt.stderr_digest)?;

    let status: ProposalGateStatus = receipt
        .status
        .parse()
        .map_err(|e| anyhow!("invalid proposal gate receipt status: {e}"))?;
    if matches!(
        status,
        ProposalGateStatus::Passed | ProposalGateStatus::Waived
    ) && receipt.exit_code != 0
    {
        anyhow::bail!(
            "proposal gate receipt status '{}' requires exit_code 0",
            status
        );
    }
    let failure_classification = receipt
        .failure_classification
        .as_deref()
        .map(str::parse::<ProposalGateFailureClassification>)
        .transpose()
        .map_err(|e| anyhow!("invalid proposal gate receipt failure_classification: {e}"))?;

    Ok(ProposalGateResult {
        gate_id: receipt.gate_id.unwrap_or_else(|| gate_id.to_string()),
        proposal_id: receipt.proposal_id,
        run_id: receipt.run_id,
        stage_id: receipt.stage_id,
        status,
        generation_id: gate_generation_id.to_string(),
        diagnostic_reason: receipt.diagnostic_reason,
        executor_version: Some(receipt.executor_version),
        evidence_digest: Some(receipt.evidence_digest),
        exit_code: Some(receipt.exit_code),
        elapsed_ms: Some(receipt.elapsed_ms),
        settled_at: Utc::now(),
        authorization_lineage: Some(proposal_gate_lineage_with_source_artifacts(
            c,
            journal_id,
            receipt
                .source_artifacts
                .unwrap_or_else(|| c.source_artifacts.clone()),
        )),
        failure_classification,
    })
}

fn proposal_gate_lineage(c: &SettleProposalGateCmd, journal_id: &str) -> ProposalGateLineage {
    proposal_gate_lineage_with_source_artifacts(c, journal_id, c.source_artifacts.clone())
}

fn proposal_gate_lineage_with_source_artifacts(
    c: &SettleProposalGateCmd,
    journal_id: &str,
    source_artifacts: Vec<String>,
) -> ProposalGateLineage {
    ProposalGateLineage {
        principal: c.principal.clone(),
        capability: c.capability.clone(),
        journal_id: journal_id.to_string(),
        authority: c.authority.clone(),
        reason: c.reason.clone(),
        source_artifacts,
        run_id: c.run_id.to_string(),
        proposal_id: c.proposal_id.clone(),
        stage_id: c.stage_id.clone(),
        workflow_digest: c.workflow_digest.clone(),
        worktree_head: c.worktree_head.clone(),
        dirty_or_changed_file_digest: c.dirty_or_changed_file_digest.clone(),
        source_generation_ids: c.source_generation_ids.clone(),
        current_fingerprint: c.current_fingerprint.clone(),
    }
}

fn proposal_gate_evidence_digest(
    c: &SettleProposalGateCmd,
    journal_id: &str,
    gate_generation_id: &str,
    status: &str,
    exit_code: i32,
) -> Result<String> {
    let payload = serde_json::json!({
        "schema_version": PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION,
        "executor_version": PROPOSAL_GATE_EXECUTOR_VERSION,
        "journal_id": journal_id,
        "gate_generation_id": gate_generation_id,
        "proposal_id": c.proposal_id,
        "run_id": c.run_id.to_string(),
        "stage_id": c.stage_id,
        "status": status,
        "exit_code": exit_code,
        "workflow_digest": c.workflow_digest,
        "worktree_head": c.worktree_head,
        "dirty_or_changed_file_digest": c.dirty_or_changed_file_digest,
        "source_generation_ids": c.source_generation_ids,
        "source_artifacts": c.source_artifacts,
        "current_fingerprint": c.current_fingerprint,
    });
    let raw = serde_json::to_vec(&payload)?;
    Ok(format!("sha256:{:x}", Sha256::digest(raw)))
}

fn proposal_gate_execution_root(run: &Run) -> PathBuf {
    if let Some(worktree_root) = run
        .worktree_root
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let path = PathBuf::from(worktree_root);
        if path.is_absolute() {
            return path;
        }
        return Path::new(&run.workspace_root).join(path);
    }
    PathBuf::from(&run.workspace_root)
}

fn validate_sha256_digest(field: &str, value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        anyhow::bail!("proposal gate receipt {field} must start with sha256:");
    };
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("proposal gate receipt {field} must be a sha256 digest");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct LoopBudgetExtensionResult {
    counter: String,
    variable_name: String,
    previous_max: u64,
    new_max: u64,
    additional_cycles: u32,
    reason: String,
    target_conflict_id: Option<String>,
    workflow_snapshot_hash: String,
}

fn validate_loop_budget_extension(extension: &WorkflowLoopBudgetExtensionCmd) -> Result<()> {
    if extension.counter.trim().is_empty() {
        anyhow::bail!("loop budget counter is required");
    }
    if extension.additional_cycles == 0 {
        anyhow::bail!("additional_cycles must be greater than zero");
    }
    if extension.additional_cycles > 100 {
        anyhow::bail!("additional_cycles must be <= 100");
    }
    if extension.reason.trim().is_empty() {
        anyhow::bail!("loop budget extension reason is required");
    }
    Ok(())
}

fn workflow_snapshot_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn find_loop_budget_variable(snapshot: &serde_json::Value, counter: &str) -> Result<(String, u64)> {
    fn visit<'a>(value: &'a serde_json::Value, counter: &str) -> Option<&'a str> {
        let object = value.as_object()?;
        if object
            .get("counter")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == counter)
        {
            if let Some(max_expr) = object.get("max").and_then(|value| value.as_str()) {
                return Some(max_expr);
            }
        }
        for child in object.values() {
            if let Some(found) = visit(child, counter) {
                return Some(found);
            }
            if let Some(array) = child.as_array() {
                for item in array {
                    if let Some(found) = visit(item, counter) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    let max_expr = visit(snapshot, counter)
        .ok_or_else(|| anyhow!("loop budget counter {counter} not found in workflow snapshot"))?;
    let variable_name = max_expr
        .strip_prefix("vars.")
        .ok_or_else(|| anyhow!("loop budget counter {counter} max is not vars.*"))?
        .to_string();
    let previous_max = snapshot
        .get("variables")
        .and_then(|value| value.get(&variable_name))
        .and_then(|value| value.as_u64())
        .ok_or_else(|| anyhow!("loop budget variable {variable_name} is missing or not numeric"))?;
    Ok((variable_name, previous_max))
}

async fn extend_workflow_loop_budget_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    extension: &WorkflowLoopBudgetExtensionCmd,
) -> Result<LoopBudgetExtensionResult> {
    validate_loop_budget_extension(extension)?;
    let row = sqlx::query("SELECT workflow_snapshot_json FROM runs WHERE id = ?1")
        .bind(run_id.to_string())
        .fetch_one(&mut **tx)
        .await
        .context("load run workflow snapshot for loop budget extension")?;
    let raw_snapshot: Option<String> = row.get("workflow_snapshot_json");
    let raw_snapshot = raw_snapshot
        .as_deref()
        .filter(|raw| !raw.trim().is_empty())
        .ok_or_else(|| anyhow!("Run {run_id} has no frozen workflow snapshot"))?;
    let mut snapshot: serde_json::Value = serde_json::from_str(raw_snapshot)
        .map_err(|e| anyhow!("parse workflow_snapshot_json for loop budget extension: {e}"))?;
    let (variable_name, previous_max) =
        find_loop_budget_variable(&snapshot, extension.counter.trim())?;
    let new_max = previous_max
        .checked_add(extension.additional_cycles as u64)
        .ok_or_else(|| anyhow!("loop budget extension overflows u64"))?;
    let variables = snapshot
        .get_mut("variables")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| anyhow!("workflow snapshot has no mutable variables object"))?;
    variables.insert(variable_name.clone(), serde_json::json!(new_max));
    let updated_snapshot = serde_json::to_string(&snapshot)?;
    let updated_hash = workflow_snapshot_hash(&updated_snapshot);
    sqlx::query(
        "UPDATE runs SET workflow_snapshot_json = ?1, workflow_snapshot_hash = ?2 WHERE id = ?3",
    )
    .bind(updated_snapshot)
    .bind(&updated_hash)
    .bind(run_id.to_string())
    .execute(&mut **tx)
    .await
    .context("persist extended workflow loop budget")?;
    Ok(LoopBudgetExtensionResult {
        counter: extension.counter.trim().to_string(),
        variable_name,
        previous_max,
        new_max,
        additional_cycles: extension.additional_cycles,
        reason: extension.reason.trim().to_string(),
        target_conflict_id: extension.target_conflict_id.clone(),
        workflow_snapshot_hash: updated_hash,
    })
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
            Command::CreateIdea(_) => "CreateIdea",
            Command::StartRun(_) => "StartRun",
            Command::ApproveStage(_) => "ApproveStage",
            Command::RejectStage(_) => "RejectStage",
            Command::RetryStage(_) => "RetryStage",
            Command::ResolveWorkflowConflictTransition(_) => "ResolveWorkflowConflictTransition",
            Command::ExtendWorkflowLoopBudget(_) => "ExtendWorkflowLoopBudget",
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
            Command::SettleProposalGate(_) => "SettleProposalGate",
        };
        let raw = serde_json::to_string(cmd).unwrap_or_default();
        let payload_json = crate::command_journal_redact::redact_for_journal(cmd, &raw);
        let run_id = match cmd {
            Command::CreateIdea(_) => None,
            Command::StartRun(_) => None,
            Command::ApproveStage(c) => Some(c.run_id.to_string()),
            Command::RejectStage(c) => Some(c.run_id.to_string()),
            Command::RetryStage(c) => Some(c.run_id.to_string()),
            Command::ResolveWorkflowConflictTransition(c) => Some(c.run_id.to_string()),
            Command::ExtendWorkflowLoopBudget(c) => Some(c.run_id.to_string()),
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
            Command::SettleProposalGate(c) => Some(c.run_id.to_string()),
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
            caller_class: caller.caller_class.clone(),
            token_id: caller.token_id.clone(),
            mcp_idempotency_key: caller.mcp_idempotency_key.clone(),
            mcp_idempotency_request_hash: caller.mcp_idempotency_request_hash.clone(),
            boundary_row_id: caller.boundary_row_id.clone(),
        }
    }

    fn is_recorded_in_command_transaction(&self) -> bool {
        matches!(
            self.command_type,
            "CreateIdea"
                | "StartRun"
                | "ApproveStage"
                | "RejectStage"
                | "RetryStage"
                | "ResolveWorkflowConflictTransition"
                | "ExtendWorkflowLoopBudget"
                | "OverrideLegacyDiscoveryPolicy"
                | "CancelRun"
                | "ResetSession"
                | "ResolveLeadMediationConfirmation"
                | "ResolveApproval"
                | "SettleProposalGate"
                | "RunStewardAnalysis"
                | "MainSyncRequest"
                | "MainSyncRetry"
                | "MainSyncSetRunOverride"
                | "MainSyncRepairState"
                | "MainSyncRecordRecoveryDecision"
                | "KnowledgeCapsuleIgnore"
        )
    }
}

async fn record_command_journal_tx(
    tx: &mut Transaction<'_, Sqlite>,
    journal: &CommandJournalEntry,
) -> Result<()> {
    if let Some(idempotency_key) = journal.mcp_idempotency_key.as_deref() {
        let request_hash = journal
            .mcp_idempotency_request_hash
            .as_deref()
            .ok_or_else(|| {
                anyhow!("MCP idempotency request hash missing for command write unit")
            })?;
        let tool_name = journal
            .caller_tool
            .as_deref()
            .unwrap_or(journal.command_type);
        let caller_fingerprint = journal
            .caller_principal_id
            .as_deref()
            .ok_or_else(|| anyhow!("MCP idempotency caller fingerprint missing"))?;
        let claimed = mcp_command_idempotency::insert_pending_tx(
            tx,
            idempotency_key,
            tool_name,
            caller_fingerprint,
            request_hash,
            journal.boundary_row_id.as_deref(),
        )
        .await?;
        if !claimed {
            anyhow::bail!("IDEMPOTENCY_IN_FLIGHT: idempotency key already claimed or committed");
        }
    }

    command_journal::record_tx(
        tx,
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
        journal.caller_class.as_deref(),
        journal.mcp_idempotency_key.as_deref(),
        journal.boundary_row_id.as_deref(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))
}

const MAX_ROLLOUT_CONTRACT_PREFLIGHT_POLICY_JSON_BYTES: usize = 64 * 1024;

fn merge_rollout_contract_preflight_policy(
    base_json: Option<String>,
    raw_policy: Option<&str>,
    journal: &CommandJournalEntry,
    caller: &CallerContext,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let Some(raw_policy) = raw_policy else {
        return Ok(base_json);
    };
    if raw_policy.len() > MAX_ROLLOUT_CONTRACT_PREFLIGHT_POLICY_JSON_BYTES {
        anyhow::bail!(
            "rollout_contract_preflight_policy_json exceeds maximum length of {} bytes",
            MAX_ROLLOUT_CONTRACT_PREFLIGHT_POLICY_JSON_BYTES
        );
    }
    let policy: serde_json::Value = serde_json::from_str(raw_policy)
        .map_err(|e| anyhow!("rollout_contract_preflight_policy_json: {e}"))?;
    let policy_object = policy
        .as_object()
        .ok_or_else(|| anyhow!("rollout_contract_preflight_policy_json must be an object"))?;

    for key in policy_object.keys() {
        if key != "waiver" && key != "enforcement_mode" {
            anyhow::bail!("rollout_contract_preflight_policy_json unknown top-level field: {key}");
        }
    }

    let mut stamped = serde_json::Map::new();
    if let Some(waiver) = policy_object.get("waiver") {
        stamped.insert(
            "waiver".to_string(),
            stamp_rollout_contract_policy_record(
                waiver,
                "waiver",
                &["state", "decision", "reason_code", "expires_at"],
                journal,
                caller,
                now,
            )?,
        );
    }
    if let Some(enforcement_mode) = policy_object.get("enforcement_mode") {
        stamped.insert(
            "enforcement_mode".to_string(),
            stamp_rollout_contract_policy_record(
                enforcement_mode,
                "enforcement_mode",
                &["mode", "reason_code", "expires_at"],
                journal,
                caller,
                now,
            )?,
        );
    }
    if stamped.is_empty() {
        anyhow::bail!("rollout_contract_preflight_policy_json requires waiver or enforcement_mode");
    }

    let mut root = match base_json {
        Some(json) => serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|e| anyhow!("delivery_preflight_json: {e}"))?,
        None => serde_json::json!({
            "passed": true,
            "checks": [],
            "timestamp": now.to_rfc3339()
        }),
    };
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("delivery_preflight_json must be an object"))?;
    root_object.insert(
        "rollout_contract_preflight".to_string(),
        serde_json::Value::Object(stamped),
    );
    Ok(Some(serde_json::to_string(&root)?))
}

fn stamp_rollout_contract_policy_record(
    value: &serde_json::Value,
    context: &str,
    allowed_fields: &[&str],
    journal: &CommandJournalEntry,
    caller: &CallerContext,
    now: DateTime<Utc>,
) -> Result<serde_json::Value> {
    let object = value.as_object().ok_or_else(|| {
        anyhow!("rollout_contract_preflight_policy_json.{context} must be an object")
    })?;
    for key in object.keys() {
        if matches!(
            key.as_str(),
            "authorized" | "principal_id" | "principal_class" | "audit_event_id"
        ) {
            anyhow::bail!(
                "rollout_contract_preflight_policy_json.{context}.{key} is server-stamped and must not be supplied"
            );
        }
        if !allowed_fields.contains(&key.as_str()) {
            anyhow::bail!("rollout_contract_preflight_policy_json.{context} unknown field: {key}");
        }
    }

    let reason_code = required_policy_string(object, context, "reason_code")?;
    let expires_at = required_policy_string(object, context, "expires_at")?;
    match DateTime::parse_from_rfc3339(expires_at) {
        Ok(expires_at) if expires_at.with_timezone(&Utc) > now => {}
        Ok(_) => anyhow::bail!(
            "rollout_contract_preflight_policy_json.{context}.expires_at must be later than scheduling time"
        ),
        Err(_) => anyhow::bail!(
            "rollout_contract_preflight_policy_json.{context}.expires_at must be an ISO-8601 timestamp"
        ),
    }

    if context == "waiver" {
        if required_policy_string(object, context, "state")? != "active" {
            anyhow::bail!("rollout_contract_preflight_policy_json.waiver.state must be active");
        }
        if required_policy_string(object, context, "decision")? != "waive" {
            anyhow::bail!("rollout_contract_preflight_policy_json.waiver.decision must be waive");
        }
    } else {
        match required_policy_string(object, context, "mode")? {
            "enforce" | "permissive" | "disabled" => {}
            other => anyhow::bail!(
                "rollout_contract_preflight_policy_json.enforcement_mode.mode is invalid: {other}"
            ),
        }
    }

    let mut stamped = object.clone();
    stamped.insert("authorized".to_string(), serde_json::json!(true));
    stamped.insert(
        "principal_id".to_string(),
        serde_json::json!(caller.principal_id),
    );
    stamped.insert(
        "principal_class".to_string(),
        serde_json::json!(caller.principal_class.to_string()),
    );
    stamped.insert("audit_event_id".to_string(), serde_json::json!(journal.id));
    stamped.insert("reason_code".to_string(), serde_json::json!(reason_code));
    Ok(serde_json::Value::Object(stamped))
}

fn required_policy_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    context: &str,
    field: &str,
) -> Result<&'a str> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "rollout_contract_preflight_policy_json.{context}.{field} must be a non-empty string"
            )
        })
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

fn retry_requires_effect_reconciliation(
    stage: &StageExecution,
    target_agent_id: Option<&str>,
    has_release_post_approval_tasks: bool,
) -> bool {
    let stage_type_requires_guard = matches!(
        stage.stage_type.as_deref(),
        Some("release" | "side_effect" | "side-effect")
    );
    stage_type_requires_guard
        || has_release_post_approval_tasks
        || is_release_agent(&stage.stage_id)
        || stage.owner_agent.as_deref().is_some_and(is_release_agent)
        || target_agent_id.is_some_and(is_release_agent)
}

fn retry_state_has_release_post_approval_tasks(run: &Run, stage_id: &str) -> Result<bool> {
    let plan = match (
        run.workflow_snapshot_json.as_deref(),
        run.catalog_snapshot_json.as_deref(),
    ) {
        (Some(workflow_snapshot_json), Some(catalog_snapshot_json)) => {
            workflow::compiler::compile_from_snapshot_json(
                workflow_snapshot_json,
                catalog_snapshot_json,
                run.agent_catalog_yaml_path.as_deref().unwrap_or("."),
            )?
        }
        _ => match (
            run.workflow_yaml_path.as_deref(),
            run.agent_catalog_yaml_path.as_deref(),
        ) {
            (Some(workflow_path), Some(catalog_path)) => {
                workflow::compiler::compile(workflow_path, catalog_path)?
            }
            _ => return Ok(false),
        },
    };

    Ok(plan.states.get(stage_id).is_some_and(|state| {
        state
            .post_approval_tasks
            .iter()
            .any(|task| is_release_agent(&task.agent.agent_id))
    }))
}

fn compile_run_plan_for_run(run: &Run) -> Result<Option<workflow::plan::RunPlan>> {
    match (
        run.workflow_snapshot_json.as_deref(),
        run.catalog_snapshot_json.as_deref(),
    ) {
        (Some(workflow_snapshot_json), Some(catalog_snapshot_json))
            if !workflow_snapshot_json.trim().is_empty()
                && !catalog_snapshot_json.trim().is_empty() =>
        {
            let catalog_path = run.agent_catalog_yaml_path.as_deref().unwrap_or(".");
            Ok(Some(workflow::compiler::compile_from_snapshot_json(
                workflow_snapshot_json,
                catalog_snapshot_json,
                catalog_path,
            )?))
        }
        _ => match (
            run.workflow_yaml_path.as_deref(),
            run.agent_catalog_yaml_path.as_deref(),
        ) {
            (Some(workflow_path), Some(catalog_path)) => Ok(Some(workflow::compiler::compile(
                workflow_path,
                catalog_path,
            )?)),
            _ => Ok(None),
        },
    }
}

async fn closeout_loop_budget_remaining_for_run(
    pool: &SqlitePool,
    run: &Run,
    refine_state_id: &str,
) -> Result<bool> {
    let Some(plan) = compile_run_plan_for_run(run)? else {
        return Ok(true);
    };
    let stages = stages::list_by_run(pool, run.id).await?;
    Ok(closeout_loop_budget_remaining(
        &plan,
        &stages,
        refine_state_id,
    ))
}

fn requires_effect_reconciliation_error(stage: &StageExecution) -> anyhow::Error {
    anyhow!(
        "requires_effect_reconciliation: retry for stage {} ({}) requires durable side-effect reconciliation before retry",
        stage.stage_id,
        stage.id
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

#[derive(Debug, Clone, PartialEq)]
struct TargetedRetryProviderFallback {
    reason: &'static str,
    from_backend_profile_id: Option<String>,
    from_provider: String,
    backend_profile_id: String,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    max_turns: Option<i64>,
    temperature: Option<f64>,
}

fn targeted_retry_catalog_profile_override(
    run: &Run,
    agent_id: &str,
    retry_payload: &serde_json::Value,
) -> Option<TargetedRetryProviderFallback> {
    let catalog: serde_json::Value =
        serde_json::from_str(run.catalog_snapshot_json.as_deref()?).ok()?;
    let current_backend_profile_id = catalog
        .get("agents")?
        .as_array()?
        .iter()
        .find(|agent| agent.get("id").and_then(serde_json::Value::as_str) == Some(agent_id))?
        .get("backend_profile")?
        .as_str()?;
    let from_backend_profile_id = retry_payload
        .get("backend_profile_id")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if from_backend_profile_id.as_deref() == Some(current_backend_profile_id) {
        return None;
    }

    let profile = catalog
        .get("backend_profiles")?
        .get(current_backend_profile_id)?
        .as_object()?;
    let provider = profile.get("provider")?.as_str()?.to_string();
    Some(TargetedRetryProviderFallback {
        reason: "current_catalog_binding_changed",
        from_backend_profile_id,
        from_provider: retry_payload
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        backend_profile_id: current_backend_profile_id.to_string(),
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
        temperature: profile
            .get("temperature")
            .and_then(serde_json::Value::as_f64),
    })
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
        "gemini"
            | "gemini_acp"
            | "claude"
            | "claude_acp"
            | "codex"
            | "codex_acp"
            | "junie"
            | "junie_acp"
    ) {
        return None;
    }
    let output_contract = retry_payload
        .get("output_contract")
        .and_then(serde_json::Value::as_str);
    let task_outputs: Vec<&str> = retry_payload
        .get("task_outputs")
        .and_then(serde_json::Value::as_array)
        .map(|outputs| {
            outputs
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect()
        })
        .unwrap_or_default();
    let is_proposal_review = output_contract == Some("proposal_review_v1");
    let is_proposal_review_aggregation =
        agent_id == "lead_orchestrator" && task_outputs.contains(&"proposal_review_summary");
    let is_proposal_authoring =
        agent_id == "proposal_writer" && task_outputs.contains(&"proposal_current");
    let is_docs_guardian = agent_id == "docs_guardian" && output_contract == Some("docs_report_v1");
    let is_security_checker =
        agent_id == "security_checker" && output_contract == Some("security_report_v1");
    let is_prepush_reviewer =
        agent_id == "prepush_code_reviewer" && output_contract == Some("prepush_review_v1");
    let is_code_writer_implementation = agent_id == "code_writer"
        && (output_contract == Some("implementation_self_assessment_v2")
            || task_outputs.iter().any(|output| {
                matches!(
                    *output,
                    "implementation_progress"
                        | "implementation_self_assessment"
                        | "implementation_self_assessment_v2"
                        | "changed_files_manifest"
                        | "tests_result"
                )
            }));
    if !is_proposal_review
        && !is_proposal_review_aggregation
        && !is_proposal_authoring
        && !is_docs_guardian
        && !is_security_checker
        && !is_prepush_reviewer
        && !is_code_writer_implementation
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
        is_security_checker,
        is_prepush_reviewer,
        is_code_writer_implementation,
        profiles,
    )?;
    let profile = profiles.get(fallback_id)?.as_object()?;
    let provider = profile.get("provider")?.as_str()?.to_string();
    if provider == from_provider {
        return None;
    }

    Some(TargetedRetryProviderFallback {
        reason: "source_provider_failed_without_required_output",
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
        temperature: profile
            .get("temperature")
            .and_then(serde_json::Value::as_f64),
    })
}

fn attach_p088_operator_retry_completion_recovery_payload(
    object: &mut serde_json::Map<String, serde_json::Value>,
    source_agent_execution_id: &str,
    evidence_path: &str,
) {
    object.insert(
        "p088".into(),
        serde_json::json!({
            "activation_source": "operator_retry_completion_recovery",
            "operator_retry_completion_recovery": true,
            "preserved_historical_evidence_packet_path": evidence_path,
            "source_agent_execution_id": source_agent_execution_id,
        }),
    );
    object.insert(
        "retry_reason".into(),
        serde_json::json!("operator_retry_completion_recovery"),
    );
}

fn targeted_retry_fallback_profile_id<'a>(
    agent_id: &str,
    from_provider: &str,
    is_proposal_review_aggregation: bool,
    is_proposal_authoring: bool,
    is_docs_guardian: bool,
    is_security_checker: bool,
    is_prepush_reviewer: bool,
    is_code_writer_implementation: bool,
    profiles: &'a serde_json::Map<String, serde_json::Value>,
) -> Option<&'a str> {
    if is_code_writer_implementation && matches!(from_provider, "junie" | "junie_acp") {
        return ["claude_builder_high"]
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
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
    if is_security_checker {
        let candidates: &[&str] = if matches!(from_provider, "claude" | "claude_acp") {
            &[
                "codex_architect_high",
                "codex_audit_high",
                "codex_writer_high",
            ]
        } else {
            &["claude_security_high", "claude_product_high"]
        };
        return candidates
            .iter()
            .copied()
            .find(|candidate| profiles.contains_key(*candidate));
    }
    if is_prepush_reviewer {
        let candidates: &[&str] = if matches!(from_provider, "claude" | "claude_acp") {
            &["codex_architect_high", "codex_writer_high"]
        } else {
            &["claude_prepush_medium", "claude_product_high"]
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
                .is_some_and(|diagnostic| {
                    diagnostic
                        .to_ascii_lowercase()
                        .contains("loop budget exhausted")
                }) =>
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
        let db_writer = Arc::new(DbWriter::new(pool.clone()));
        Self {
            pool,
            events,
            work_queue,
            db_writer,
            acp: None,
            capacity_config: Arc::new(capacity_config),
            retry_stage_failure_injection: None,
            boundary_policy: None,
        }
    }

    /// P081 Phase 3: inject the shared immutable BoundaryPolicy for audit-log entries.
    pub fn with_boundary_policy(mut self, policy: Arc<auth::boundary::BoundaryPolicy>) -> Self {
        self.boundary_policy = Some(policy);
        self
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
        Self::new_with_acp_capacity_and_db_writer(
            pool,
            events,
            work_queue,
            acp,
            capacity_config,
            None,
        )
    }

    pub fn new_with_acp_capacity_and_db_writer(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        acp: Arc<AcpRuntimeManager>,
        capacity_config: InvokeAgentCapacityConfig,
        db_writer: Option<Arc<DbWriter>>,
    ) -> Self {
        let db_writer = db_writer.unwrap_or_else(|| Arc::new(DbWriter::new(pool.clone())));
        Self {
            pool,
            events,
            work_queue,
            db_writer,
            acp: Some(acp),
            capacity_config: Arc::new(capacity_config),
            retry_stage_failure_injection: None,
            boundary_policy: None,
        }
    }

    async fn begin_command_transaction(
        &self,
        operation_name: &'static str,
        idempotency_key: impl Into<String>,
    ) -> Result<db::writer::QueuedTransaction> {
        self.db_writer
            .begin_immediate_transaction(
                class_a_operation(operation_name, WriteLane::OperatorCommand, idempotency_key),
                operation_name,
            )
            .await
    }

    pub fn db_writer(&self) -> Arc<DbWriter> {
        self.db_writer.clone()
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

    pub fn handle(
        &self,
        cmd: Command,
        caller: CallerContext,
    ) -> Pin<Box<dyn Future<Output = Result<Commanded>> + Send + '_>> {
        Box::pin(async move { self.handle_inner(cmd, caller).await })
    }

    async fn handle_inner(&self, cmd: Command, caller: CallerContext) -> Result<Commanded> {
        if matches!(&cmd, Command::OverrideArtifactContract(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: OverrideArtifactContract requires operator principal");
        }
        if matches!(
            &cmd,
            Command::StartRun(c) if c.rollout_contract_preflight_policy_json.is_some()
        ) && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!(
                "forbidden: StartRun rollout_contract_preflight_policy_json requires operator principal"
            );
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
        if matches!(&cmd, Command::ExtendWorkflowLoopBudget(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: ExtendWorkflowLoopBudget requires operator principal");
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
        if matches!(&cmd, Command::SettleProposalGate(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: SettleProposalGate requires operator principal");
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
                journal.caller_class.as_deref(),
                journal.mcp_idempotency_key.as_deref(),
                journal.boundary_row_id.as_deref(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;
        }

        // P081 Phase 5: idempotency short-circuit for ResolveApproval.
        // SEC-P081-MED-001: Use a dedicated BEGIN IMMEDIATE transaction for the lookup
        // to serialize against the settlement transaction and prevent concurrent retries
        // from observing the absence of a record before the first commit lands.
        if let Command::ResolveApproval(ref c) = cmd {
            if let Some(ref key) = c.idempotency_key {
                let action_name = match c.decision {
                    ApprovalResolutionDecision::Approved => "approve",
                    ApprovalResolutionDecision::Rejected => "reject",
                };
                let current_fp = {
                    let canonical = format!(
                        "{}\x1e{}",
                        journal.caller_principal_id.as_deref().unwrap_or(""),
                        journal.caller_class.as_deref().unwrap_or("")
                    );
                    let mut h = Sha256::new();
                    h.update(canonical.as_bytes());
                    format!("{:x}", h.finalize())
                };
                // SEC-P081-M002: canonical request hash covers action, approval_id,
                // caller_class, and principal_id. Excludes request_id, timestamps, and
                // retry metadata so retries with the same logical request compare equal.
                let current_request_hash = {
                    let canonical = format!(
                        "{}\x1e{}\x1e{}\x1e{}",
                        action_name,
                        c.approval_id,
                        journal.caller_class.as_deref().unwrap_or(""),
                        journal.caller_principal_id.as_deref().unwrap_or(""),
                    );
                    let mut h = Sha256::new();
                    h.update(canonical.as_bytes());
                    format!("{:x}", h.finalize())
                };
                // Open a short-lived BEGIN IMMEDIATE to serialize against settlement.
                match db::pool::begin_immediate_with_retry(&self.pool, "idempotency.check").await {
                    Ok(mut check_tx) => {
                        let lookup =
                            approval_mutation_idempotency::find_by_key_tx(&mut check_tx, key).await;
                        drop(check_tx); // release lock before settlement starts
                        match lookup {
                            Ok(Some(record)) => {
                                // SEC-P081-M002: check request_hash first when present.
                                // Same key + different canonical request → IDEMPOTENCY_CONFLICT.
                                if let (Some(stored_hash), _) =
                                    (&record.request_hash, &current_request_hash)
                                {
                                    if stored_hash != &current_request_hash {
                                        return Err(anyhow::anyhow!("IDEMPOTENCY_CONFLICT"));
                                    }
                                }
                                if record.approval_id == c.approval_id.to_string()
                                    && record.action == action_name
                                    && record.caller_fingerprint == current_fp
                                {
                                    let replay_result = match c.decision {
                                        ApprovalResolutionDecision::Approved => {
                                            CommandResult::StageApproved {
                                                approval_id: c.approval_id,
                                            }
                                        }
                                        ApprovalResolutionDecision::Rejected => {
                                            CommandResult::StageRejected {
                                                approval_id: c.approval_id,
                                            }
                                        }
                                    };
                                    // P081 Defect8: write approval_idempotency_duplicate audit row.
                                    // Best-effort: inability to write must not block returning the cached result.
                                    {
                                        db::metrics::record_p081_approval_idempotency_duplicate(
                                            action_name,
                                            journal.caller_class.as_deref().unwrap_or("unknown"),
                                        );
                                        let audit_id = uuid::Uuid::now_v7().to_string();
                                        let now_ms = chrono::Utc::now().timestamp_millis();
                                        let key_digest = {
                                            let mut h = Sha256::new();
                                            h.update(key.as_bytes());
                                            let full = h.finalize();
                                            full[..8]
                                                .iter()
                                                .map(|b| format!("{:02x}", b))
                                                .collect::<String>()
                                        };
                                        let raw_payload = serde_json::json!({
                                            "idempotency_key_digest": key_digest,
                                            "approval_id": record.approval_id,
                                            "action": action_name,
                                        })
                                        .to_string();
                                        let (stored_payload, _, truncated) =
                                            audit_log::build_envelope(&raw_payload);
                                        let transport = match journal.caller_surface.as_deref() {
                                            Some("mcp") => "mcp_tools_call",
                                            _ => "graphql_mutation",
                                        };
                                        // P081 Defect4: audit_log CHECK requires non-empty request_id.
                                        // Fall back to the journal id (unique, non-empty) rather than "".
                                        let effective_request_id = journal
                                            .request_id
                                            .as_deref()
                                            .filter(|s| !s.is_empty())
                                            .unwrap_or(&journal.id);
                                        let (dup_policy_mode, dup_fixture_ver) =
                                            match &self.boundary_policy {
                                                Some(p) => (
                                                    p.mode().to_string(),
                                                    p.fixture_digest().to_string(),
                                                ),
                                                None => (
                                                    "legacy_compat".to_string(),
                                                    "embedded".to_string(),
                                                ),
                                            };
                                        let entry = audit_log::AuditEntry {
                                            id: &audit_id,
                                            request_id: effective_request_id,
                                            timestamp_ms: now_ms,
                                            event_type: "approval_idempotency_duplicate",
                                            principal_id: journal.caller_principal_id.as_deref(),
                                            principal_class: journal
                                                .caller_principal_class
                                                .as_deref(),
                                            caller_class: journal.caller_class.as_deref(),
                                            token_id: journal.token_id.as_deref(),
                                            transport,
                                            action_attempted: action_name,
                                            decision: "allow",
                                            denial_reason_code: None,
                                            row_id: None,
                                            env_gate_state: None,
                                            source_ip_hash_or_local_process_id: None,
                                            boundary_policy_mode: &dup_policy_mode,
                                            fixture_version: &dup_fixture_ver,
                                            payload: &stored_payload,
                                            original_payload_bytes: if truncated {
                                                Some(&raw_payload)
                                            } else {
                                                None
                                            },
                                            diagnostic_truncated: truncated,
                                            checkpoint_id: None,
                                            created_at_ms: now_ms,
                                        };
                                        if let Err(e) = audit_log::append(&self.pool, &entry).await
                                        {
                                            tracing::warn!(
                                                error = %e,
                                                "approval_idempotency_duplicate audit write failed (best-effort); returning cached result"
                                            );
                                        }
                                    }
                                    return Ok(Commanded {
                                        result: replay_result,
                                        journal_id: record.command_journal_id,
                                    });
                                }
                                return Err(anyhow::anyhow!("IDEMPOTENCY_CONFLICT"));
                            }
                            Ok(None) => {}
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "approval_mutation_idempotency lookup failed; failing closed"
                                );
                                return Err(anyhow::anyhow!(
                                    "IDEMPOTENCY_LOOKUP_ERROR: storage error during idempotency check"
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "failed to open idempotency check transaction; failing closed"
                        );
                        return Err(anyhow::anyhow!(
                            "IDEMPOTENCY_LOOKUP_ERROR: could not open check transaction"
                        ));
                    }
                }
            }
        }

        let result = self.execute_command(cmd, &journal, &caller).await;

        // P081: Handle concurrent idempotency race replay from inner settlement transaction.
        // When two requests with the same key race past the pre-check, the second one's
        // settlement transaction finds the first one's committed record and returns this
        // sentinel. Convert it to Ok(Commanded) with the ORIGINAL journal_id so callers
        // get a consistent idempotent response. The fresh journal entry for this request
        // was never committed (the settlement transaction was rolled back), so we must not
        // call complete_entry / fail_entry for it — returning early avoids that.
        if let Err(ref e) = result {
            if let Some(replay) = e.downcast_ref::<ConcurrentIdempotencyRaceReplay>() {
                let replay_result = if replay.was_approved {
                    CommandResult::StageApproved {
                        approval_id: replay.approval_id,
                    }
                } else {
                    CommandResult::StageRejected {
                        approval_id: replay.approval_id,
                    }
                };
                return Ok(Commanded {
                    result: replay_result,
                    journal_id: replay.command_journal_id.clone(),
                });
            }
        }

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
            Command::CreateIdea(c) => {
                let idea = domain::idea::Idea {
                    id: domain::ids::IdeaId::new(),
                    title: c.title,
                    body: c.body,
                    workspace_root_path: c.workspace_root_path,
                    project_key: c.project_key,
                    status: domain::idea::IdeaStatus::Draft,
                    created_at: Utc::now(),
                    archived_at: None,
                };
                let mut tx = self
                    .begin_command_transaction("command.CreateIdea", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
                ideas::insert_tx(&mut tx, &idea).await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await.context("commit create idea command")?;
                Ok(CommandResult::IdeaCreated { idea })
            }
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

                let mut delivery_preflight_json =
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
                delivery_preflight_json = merge_rollout_contract_preflight_policy(
                    delivery_preflight_json,
                    c.rollout_contract_preflight_policy_json.as_deref(),
                    &journal,
                    caller,
                    now,
                )?;
                let phase_b_dogfood_snapshot =
                    phase_b_dogfood_exit_metric_snapshot(&c.workspace_root);
                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction("command.StartRun", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
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
                    // P077: Frozen closeout readiness mode from workflow snapshot metadata.
                    // Prefer the compiled plan's mode (from workflow YAML) over the command
                    // field, which defaults to None when not populated by the MCP caller.
                    closeout_readiness_mode: plan
                        .closeout_readiness_mode
                        .clone()
                        .or_else(|| c.closeout_readiness_mode.clone()),
                };
                ensure_run_meta_root_exists(&run)?;
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
                let mut tx = self
                    .begin_command_transaction("command.ApproveStage", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
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
                projections::rebuild_approval_inbox(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageApproved {
                    approval_id: approval.id,
                })
            }

            Command::RejectStage(c) => {
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction("command.RejectStage", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
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
                let mut retry_tx = self
                    .begin_command_transaction("command.RetryStage", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut retry_tx, journal).await?;
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
                ensure_run_meta_root_exists(&run)?;
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

                let has_release_post_approval_tasks =
                    match retry_state_has_release_post_approval_tasks(&run, &old_stage.stage_id) {
                        Ok(has_release_post_approval_tasks) => has_release_post_approval_tasks,
                        Err(e) => {
                            warn!(
                                run_id = %c.run_id,
                                stage_id = %old_stage.stage_id,
                                error = %e,
                                "RetryStage side-effect preflight could not inspect post_approval_tasks"
                            );
                            false
                        }
                    };
                // Ledger-backed preflight: check actual unresolved side effects for this stage.
                // Always enforced regardless of CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED.
                // Uses the open transaction to avoid deadlocking on pool acquire when
                // max_connections=1 (in-memory SQLite in tests).
                if let Err(ledger_err) =
                    retry_preflight_within_tx(&mut retry_tx, &c.run_id, &old_stage.id, None).await
                {
                    command_journal::fail_entry_tx(
                        &mut retry_tx,
                        &journal.id,
                        Utc::now(),
                        &ledger_err.to_string(),
                    )
                    .await?;
                    retry_tx.commit().await?;
                    db::pool::log_write_transaction("command.RetryStage", retry_tx_started);
                    return Err(ledger_err);
                }

                // Heuristic guard: still catch release stages not yet wired to the ledger.
                if retry_requires_effect_reconciliation(
                    old_stage,
                    None,
                    has_release_post_approval_tasks,
                ) {
                    let error = requires_effect_reconciliation_error(old_stage);
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
                let retry_authority_id = format!("p091-retry-authority:{}", new_stage.id);
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
                retry_stage_execution_authorities::supersede_active_for_stage_tx(
                    &mut retry_tx,
                    c.run_id,
                    &c.stage_id,
                    now,
                    "superseded_by_new_retry",
                )
                .await?;
                retry_stage_execution_authorities::create_tx(
                    &mut retry_tx,
                    &RetryStageExecutionAuthority {
                        id: retry_authority_id.clone(),
                        run_id: c.run_id,
                        stage_id: c.stage_id.clone(),
                        target_stage_execution_id: new_stage.id,
                        entry_kind: RetryAuthorityEntryKind::FullStageRetry,
                        source_command_journal_id: Some(journal_id.to_string()),
                        source_retry_work_item_id: Some(retry_advance_work_item_id.clone()),
                        source_invoke_work_item_id: None,
                        source_agent_execution_id: None,
                        authority_state: RetryAuthorityState::Active,
                        created_at: now,
                        updated_at: now,
                        terminal_reason: None,
                    },
                )
                .await?;
                self.maybe_inject_retry_stage_failure("create_retry_authority")?;
                sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
                    .bind(RunStatus::Running.to_string())
                    .bind(c.stage_id.clone())
                    .bind(c.run_id.to_string())
                    .execute(&mut **retry_tx)
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
                        id: retry_advance_work_item_id.clone(),
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({
                            "schema_version": "advance_run_payload.v1",
                            "run_id": c.run_id.to_string(),
                            "stage_id": c.stage_id.clone(),
                            "target_stage_execution_id": new_stage.id.to_string(),
                            "retry_authority_id": retry_authority_id,
                            "source_stage_execution_id": old_stage.id.to_string(),
                            "source_work_item_id": retry_advance_work_item_id,
                            "enqueue_reason": "retry_stage",
                            "reason": "operator_full_stage_retry"
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
                let mut tx = self
                    .begin_command_transaction(
                        "command.ResolveWorkflowConflictTransition",
                        journal.id.clone(),
                    )
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;

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
                if let Some(extension) = c.loop_budget_extension.as_ref() {
                    if let Some(target_conflict_id) = extension.target_conflict_id.as_deref() {
                        if target_conflict_id != c.conflict_id {
                            anyhow::bail!(
                                "loop budget extension target_conflict_id {} does not match resolved conflict {}",
                                target_conflict_id,
                                c.conflict_id
                            );
                        }
                    }
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
                let loop_budget_extension_result =
                    if let Some(extension) = c.loop_budget_extension.as_ref() {
                        Some(extend_workflow_loop_budget_tx(&mut tx, c.run_id, extension).await?)
                    } else {
                        None
                    };

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
                        "loop_budget_extension": loop_budget_extension_result.as_ref().map(|extension| serde_json::json!({
                            "counter": extension.counter,
                            "variable_name": extension.variable_name,
                            "previous_max": extension.previous_max,
                            "additional_cycles": extension.additional_cycles,
                            "new_max": extension.new_max,
                            "reason": extension.reason,
                            "target_conflict_id": extension.target_conflict_id,
                            "workflow_snapshot_hash": extension.workflow_snapshot_hash,
                        })),
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
                    .execute(&mut **tx)
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

            Command::ExtendWorkflowLoopBudget(ExtendWorkflowLoopBudgetCmd {
                run_id,
                extension,
            }) => {
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction(
                        "command.ExtendWorkflowLoopBudget",
                        journal.id.clone(),
                    )
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;

                let run = runs::find_by_id_tx(&mut tx, run_id)
                    .await?
                    .ok_or_else(|| anyhow!("Run {run_id} not found"))?;
                let result = extend_workflow_loop_budget_tx(&mut tx, run_id, &extension).await?;
                sqlx::query("UPDATE runs SET status = ?1 WHERE id = ?2")
                    .bind(RunStatus::Running.to_string())
                    .bind(run_id.to_string())
                    .execute(&mut **tx)
                    .await?;
                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: format!(
                            "workflow-loop-budget-extend:{}:{}",
                            run_id,
                            uuid::Uuid::new_v4()
                        ),
                        kind: WorkItemKind::AdvanceRun,
                        payload_json: serde_json::json!({
                            "schema_version": "advance_run_payload.v1",
                            "run_id": run_id.to_string(),
                            "reason": "workflow_loop_budget_extended",
                            "counter": result.counter,
                            "previous_max": result.previous_max,
                            "new_max": result.new_max,
                            "target_conflict_id": result.target_conflict_id,
                        })
                        .to_string(),
                        status: WorkItemStatus::Pending,
                        run_id: Some(run_id),
                        stage_id: run.current_state.clone(),
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
                    "command.ExtendWorkflowLoopBudget",
                    0,
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.ExtendWorkflowLoopBudget", tx_started);
                self.work_queue
                    .publish_scheduler_notification(scheduler_refresh);
                projections::rebuild_all_for_run(&self.pool, run_id).await?;

                Ok(CommandResult::WorkflowLoopBudgetExtended {
                    run_id,
                    counter: result.counter,
                    previous_max: result.previous_max,
                    new_max: result.new_max,
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
                let mut tx = self
                    .begin_command_transaction(
                        "command.OverrideLegacyDiscoveryPolicy",
                        journal.id.clone(),
                    )
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
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
                let mut tx = self
                    .begin_command_transaction("command.CancelRun", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;

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

                // Ledger-backed preflight: block cancel when any unresolved side effects exist
                // for this run, regardless of CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED.
                // Use the tx-scoped variant to avoid deadlocking on single-connection pools.
                if let Err(ledger_err) = run_cancel_preflight_within_tx(&mut tx, &c.run_id).await {
                    command_journal::fail_entry_tx(
                        &mut tx,
                        &journal.id,
                        Utc::now(),
                        &ledger_err.to_string(),
                    )
                    .await?;
                    tx.commit().await?;
                    db::pool::log_write_transaction("command.CancelRun", tx_started);
                    return Err(ledger_err);
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
                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction("command.RunStewardAnalysis", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
                work_items::enqueue_tx(
                    &mut tx,
                    &WorkItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: WorkItemKind::StewardAnalysis,
                        payload_json: serde_json::to_string(&serde_json::json!({
                            "reason": c.reason,
                            "artifact_base": artifact_base,
                        }))?,
                        status: WorkItemStatus::Pending,
                        run_id: None,
                        stage_id: None,
                        created_at: now,
                        scheduled_at: now,
                        attempt_count: 0,
                        last_error: None,
                    },
                )
                .await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.RunStewardAnalysis", tx_started);
                Ok(CommandResult::StewardAnalysisQueued)
            }

            Command::ResolveLeadMediationConfirmation(c) => {
                // BLK-006: Guard against resolution when mediation is disabled
                if !crate::mediation::feature_flag::is_phase_b_mediation_enabled() {
                    return Err(anyhow!("Phase B mediation is disabled"));
                }

                let now = Utc::now();
                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction(
                        "command.ResolveLeadMediationConfirmation",
                        journal.id.clone(),
                    )
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;

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
                let mut tx = self
                    .begin_command_transaction("command.ResetSession", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;

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

            // ── P072/P081: Converged approval resolution by approval_id ──────
            Command::ResolveApproval(c) => {
                let now = Utc::now();
                let decision = match c.decision {
                    ApprovalResolutionDecision::Approved => ApprovalDecision::Granted,
                    ApprovalResolutionDecision::Rejected => ApprovalDecision::Rejected,
                };
                let action_name = match c.decision {
                    ApprovalResolutionDecision::Approved => "approve",
                    ApprovalResolutionDecision::Rejected => "reject",
                };

                let has_post_tasks = if decision == ApprovalDecision::Granted {
                    self.check_has_post_approval_tasks(c.run_id, &c.stage_id)
                        .await
                } else {
                    false
                };

                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction("command.ResolveApproval", journal.id.clone())
                    .await?;

                // P081: check terminal state BEFORE command_journal::record_tx so that
                // denied/terminal attempts create zero command_journal rows.
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
                        // P081 committed-unack replay: if the caller supplied an idempotency key,
                        // check whether the record for this key was committed by the first attempt
                        // (visible under the same BEGIN IMMEDIATE transaction). If found and the
                        // key matches caller+action, return the original result without writing
                        // new side effects. This closes the race between concurrent retries that
                        // both passed the precheck before the first attempt committed.
                        if let Some(ref key) = c.idempotency_key {
                            let caller_fp = {
                                let canonical = format!(
                                    "{}\x1e{}",
                                    journal.caller_principal_id.as_deref().unwrap_or(""),
                                    journal.caller_class.as_deref().unwrap_or("")
                                );
                                let mut h = Sha256::new();
                                h.update(canonical.as_bytes());
                                format!("{:x}", h.finalize())
                            };
                            if let Ok(Some(record)) =
                                approval_mutation_idempotency::find_by_key_tx(&mut tx, key).await
                            {
                                if record.approval_id == c.approval_id.to_string()
                                    && record.action == action_name
                                    && record.caller_fingerprint == caller_fp
                                {
                                    drop(tx);
                                    db::pool::log_write_transaction(
                                        "command.ResolveApproval",
                                        tx_started,
                                    );
                                    db::metrics::record_p081_boundary_commit_transaction_latency(
                                        "graphql_mutation",
                                        action_name,
                                        "idempotency_replay",
                                        tx_started.elapsed(),
                                    );
                                    // P081 fix: return sentinel with original journal_id so
                                    // the outer handle() returns Commanded { journal_id:
                                    // record.command_journal_id } instead of the fresh,
                                    // never-committed journal id for this request.
                                    let was_approved =
                                        matches!(c.decision, ApprovalResolutionDecision::Approved);
                                    return Err(anyhow::Error::new(
                                        ConcurrentIdempotencyRaceReplay {
                                            command_journal_id: record.command_journal_id.clone(),
                                            was_approved,
                                            approval_id: c.approval_id,
                                        },
                                    ));
                                }
                            }
                        }
                        drop(tx);
                        db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                        db::metrics::record_p081_boundary_commit_transaction_latency(
                            "graphql_mutation",
                            action_name,
                            "terminal_rejected",
                            tx_started.elapsed(),
                        );
                        // P081: approval is terminal but the idempotency key didn't match
                        // (different key, or no key supplied). This is NOT a same-key replay —
                        // it's a new attempt to re-settle a terminal approval, which is forbidden.
                        // Return APPROVAL_NOT_ACTIONABLE (not AlreadyResolved) per P081 contract.
                        return Err(ApprovalResolutionConflict::ApprovalNotActionable {
                            approval_id: c.approval_id,
                            journal_id: journal.id.clone(),
                        }
                        .into());
                    }
                    None => {
                        drop(tx);
                        db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                        db::metrics::record_p081_boundary_commit_transaction_latency(
                            "graphql_mutation",
                            action_name,
                            "not_found",
                            tx_started.elapsed(),
                        );
                        return Err(anyhow!("Approval {} not found", c.approval_id));
                    }
                };
                if approval.run_id != c.run_id || approval.stage_id != c.stage_id {
                    let err = anyhow!(
                        "Approval {} provenance mismatch: command run/stage {}:{} but approval belongs to {}:{}",
                        c.approval_id, c.run_id, c.stage_id, approval.run_id, approval.stage_id
                    );
                    drop(tx);
                    db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                    db::metrics::record_p081_boundary_commit_transaction_latency(
                        "graphql_mutation",
                        action_name,
                        "provenance_mismatch",
                        tx_started.elapsed(),
                    );
                    return Err(err);
                }
                let authoritative_run_id = approval.run_id;
                let authoritative_stage_id = approval.stage_id.clone();

                // Approval is actionable: record in command_journal inside the transaction.
                record_command_journal_tx(&mut tx, journal).await?;

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
                                .execute(&mut **tx)
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

                // P081 Phase 5: idempotency record in same transaction as settlement.
                if let Some(ref key) = c.idempotency_key {
                    // SEC-P081-001: Use SHA-256 over canonical fields separated by RS (0x1E)
                    // so principal ids containing '/' cannot collide with caller_class.
                    let caller_fp = {
                        let canonical = format!(
                            "{}\x1e{}",
                            journal.caller_principal_id.as_deref().unwrap_or(""),
                            journal.caller_class.as_deref().unwrap_or("")
                        );
                        let mut h = Sha256::new();
                        h.update(canonical.as_bytes());
                        format!("{:x}", h.finalize())
                    };
                    // SEC-P081-M002: canonical request hash for conflict detection.
                    let req_hash = {
                        let canonical = format!(
                            "{}\x1e{}\x1e{}\x1e{}",
                            action_name,
                            c.approval_id,
                            journal.caller_class.as_deref().unwrap_or(""),
                            journal.caller_principal_id.as_deref().unwrap_or(""),
                        );
                        let mut h = Sha256::new();
                        h.update(canonical.as_bytes());
                        format!("{:x}", h.finalize())
                    };
                    let record = approval_mutation_idempotency::build_record(
                        key,
                        &c.approval_id.to_string(),
                        action_name,
                        &caller_fp,
                        journal.request_id.as_deref(),
                        Some(&req_hash),
                        &journal.id,
                        None,
                    );
                    approval_mutation_idempotency::insert_tx(&mut tx, &record)
                        .await
                        .map_err(|e| {
                            // A UNIQUE constraint violation means a concurrent request with the same
                            // idempotency_key committed first. Map to IDEMPOTENCY_CONFLICT so callers
                            // get the documented error code rather than a generic failure.
                            let msg = e.to_string();
                            if msg.contains("UNIQUE") || msg.contains("unique") {
                                anyhow::anyhow!("IDEMPOTENCY_CONFLICT")
                            } else {
                                anyhow::anyhow!("idempotency insert failed: {e}")
                            }
                        })?;
                }

                // P081 Phase 3: audit_log row for the allowed approval resolution.
                // Committed in the same write unit as command_journal and settlement.
                {
                    let audit_id = uuid::Uuid::now_v7().to_string();
                    let action_attempted = match c.decision {
                        ApprovalResolutionDecision::Approved => "approveApproval",
                        ApprovalResolutionDecision::Rejected => "rejectApproval",
                    };
                    let transport = match caller.surface {
                        domain::commands::CallerSurface::Graphql => "graphql_mutation",
                        domain::commands::CallerSurface::Mcp => "mcp_tools_call",
                    };
                    let audit_payload = serde_json::json!({
                        "approval_id": c.approval_id.to_string(),
                        "run_id": authoritative_run_id.to_string(),
                        "stage_id": authoritative_stage_id,
                        "decision": action_name,
                    })
                    .to_string();
                    let (policy_mode, fixture_ver) = match &self.boundary_policy {
                        Some(p) => (p.mode().to_string(), p.fixture_digest().to_string()),
                        None => ("legacy_compat".to_string(), "embedded".to_string()),
                    };
                    let ts_ms = now.timestamp_millis();
                    // Use journal.id as synthetic request_id when caller didn't supply one,
                    // since the CHECK constraint requires length > 0.
                    let audit_request_id = journal
                        .request_id
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(journal.id.as_str());
                    let audit_entry = audit_log::AuditEntry {
                        id: &audit_id,
                        request_id: audit_request_id,
                        timestamp_ms: ts_ms,
                        event_type: "approval_resolved",
                        principal_id: journal.caller_principal_id.as_deref(),
                        principal_class: journal.caller_principal_class.as_deref(),
                        caller_class: journal.caller_class.as_deref(),
                        token_id: journal.token_id.as_deref(),
                        transport,
                        action_attempted,
                        decision: "allow",
                        denial_reason_code: None,
                        row_id: Some(match transport {
                            "graphql_mutation" => {
                                "p081.ui_operator.graphql_mutation.approval_action"
                            }
                            _ => "p081.agent_operator.mcp_tools_call.command",
                        }),
                        env_gate_state: None,
                        source_ip_hash_or_local_process_id: None,
                        boundary_policy_mode: &policy_mode,
                        fixture_version: &fixture_ver,
                        payload: &audit_payload,
                        original_payload_bytes: None,
                        diagnostic_truncated: false,
                        checkpoint_id: None,
                        created_at_ms: ts_ms,
                    };
                    audit_log::append_tx(&mut tx, &audit_entry).await.map_err(|e| {
                        tracing::warn!(error = %e, "audit_log append failed in ResolveApproval");
                        e.context("audit_log append failed: failing closed per P081")
                    })?;
                }

                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.ResolveApproval", tx_started);
                db::metrics::record_p081_boundary_commit_transaction_latency(
                    "graphql_mutation",
                    action_name,
                    "committed",
                    tx_started.elapsed(),
                );
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
            Command::SettleProposalGate(mut c) => {
                // BLK-008: bind principal from authenticated CallerContext —
                // never trust the caller-supplied payload field.
                c.principal = caller.principal_id.clone();

                // BLK-010: Bind capability to the canonical CapabilityToolId::ProposalGateSettle
                // token and reject mismatches. Bind authority to registered allow-list.
                validate_proposal_gate_authorization(&c)?;
                validate_accepted_risk_lineage(&c)?;

                let settle_started = Instant::now();
                let gate_id = format!("p{}:{}", c.proposal_id, c.proposal_id);
                let gate_generation_id = uuid::Uuid::new_v4().to_string();
                let run = runs::find_by_id(&self.pool, c.run_id)
                    .await?
                    .ok_or_else(|| anyhow!("SettleProposalGate run not found"))?;
                let run_id_str = c.run_id.to_string();
                let mut upstream_generation_ids =
                    closeout::list_closeout_fingerprint_source_generation_ids(
                        &self.pool,
                        &run_id_str,
                    )
                    .await
                    .unwrap_or_else(|_| c.source_generation_ids.clone());
                if upstream_generation_ids.is_empty() {
                    upstream_generation_ids = c.source_generation_ids.clone();
                }
                let worktree_truth = resolve_closeout_worktree_truth(&run).await;
                if let Some(reason) = worktree_truth.diagnostic_reason.as_deref() {
                    warn!(
                        run_id = %run_id_str,
                        reason,
                        "P077: current worktree fingerprint truth unavailable; closeout will fail closed"
                    );
                }
                let closeout_fingerprint = build_closeout_fingerprint(
                    &run,
                    &c.stage_id,
                    worktree_truth.worktree_head.clone(),
                    worktree_truth.dirty_or_changed_file_digest.clone(),
                    upstream_generation_ids.clone(),
                    worktree_truth.latency_ms,
                );
                let fingerprint_latency_exceeded = worktree_truth.unavailable
                    || worktree_truth.latency_exceeded
                    || worktree_truth.latency_ms > CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS;
                c.worktree_head = closeout_fingerprint.worktree_head.clone();
                c.dirty_or_changed_file_digest =
                    closeout_fingerprint.dirty_or_changed_file_digest.clone();
                c.source_generation_ids = upstream_generation_ids.clone();
                c.current_fingerprint = closeout_fingerprint.short_hash();
                if matches!(c.action, ProposalGateSettlementAction::Execute)
                    && c.receipt_json
                        .as_deref()
                        .is_none_or(|raw| raw.trim().is_empty())
                {
                    let execution_root = proposal_gate_execution_root(&run);
                    c.receipt_json = Some(execute_managed_proposal_gate_receipt(
                        &c,
                        &gate_id,
                        &execution_root,
                    )?);
                }
                let gate_result = build_proposal_gate_result_from_settlement(
                    &c,
                    &journal.id,
                    &gate_id,
                    &gate_generation_id,
                    settle_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                )?;

                let tx_started = Instant::now();
                let mut tx = self
                    .begin_command_transaction("command.SettleProposalGate", journal.id.clone())
                    .await?;
                record_command_journal_tx(&mut tx, journal).await?;
                command_journal::complete_entry_tx(&mut tx, &journal.id, Utc::now()).await?;
                tx.commit().await?;
                db::pool::log_write_transaction("command.SettleProposalGate", tx_started);

                // Resolve closeout readiness mode from the frozen run column.
                let mode_column =
                    closeout::find_closeout_readiness_mode(&self.pool, &run_id_str).await?;
                let has_enforcement_migration =
                    closeout::has_enforcement_migration_record(&self.pool, &run_id_str).await?;
                let mode_result = resolve_closeout_readiness_mode(
                    mode_column.as_deref(),
                    has_enforcement_migration,
                );

                // Load active self-assessment if present.
                let self_assessment_stored =
                    artifact_contracts::find_active_implementation_self_assessment_summary(
                        &self.pool, c.run_id,
                    )
                    .await?;
                let self_assessment_ref = self_assessment_stored.as_ref().map(|s| &s.summary);

                // P077 BLK-006: source controlled_reports_green from active artifact contracts.
                let controlled_reports_green =
                    closeout::compute_controlled_reports_green(&self.pool, &run_id_str)
                        .await
                        .ok()
                        .flatten();

                // P077 BLK-011: read prior blocker_digest for soft-convergence detection.
                let prior_blocker_digest =
                    closeout::find_active_blocker_digest(&self.pool, &run_id_str)
                        .await
                        .ok()
                        .flatten();
                let loop_budget_remaining = closeout_loop_budget_remaining_for_run(
                    &self.pool,
                    &run,
                    "state_10_implementation_refined",
                )
                .await
                .unwrap_or_else(|error| {
                    warn!(
                        run_id = %run_id_str,
                        error = %error,
                        "failed to resolve P077 loop budget; failing closeout readiness closed"
                    );
                    false
                });
                let implementation_review_status =
                    match artifact_contracts::canonical_contract_field_result(
                        &self.pool,
                        run.id,
                        "implementation_review_summary",
                        "status",
                    )
                    .await
                    {
                        Ok(artifact_contracts::CanonicalContractField::Resolved(value)) => {
                            value.as_str().map(ToOwned::to_owned)
                        }
                        _ => None,
                    };

                // Synthesize closeout readiness.
                let synth_result =
                    synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
                        run_id: &run_id_str,
                        stage_id: &c.stage_id,
                        gate_result: &gate_result,
                        mode_result: &mode_result,
                        implementation_review_status: implementation_review_status.as_deref(),
                        self_assessment: self_assessment_ref,
                        accepted_risks: &c.accepted_risks,
                        loop_budget_remaining,
                        fingerprint: Some(closeout_fingerprint),
                        fingerprint_latency_exceeded,
                        controlled_reports_green,
                        previous_blocker_digest: prior_blocker_digest.as_deref(),
                    });

                // Atomically activate gate + readiness generations, then rebuild projections.
                let closeout_tx_result =
                    closeout::execute_closeout_transaction_with_projection_rebuild(
                        &self.pool,
                        closeout::CloseoutTransactionInputs {
                            gate_result: &gate_result,
                            readiness: &synth_result.readiness,
                            accepted_risks: &c.accepted_risks,
                            blocker_digest: synth_result.current_blocker_digest.as_deref(),
                        },
                    )
                    .await?;

                Ok(CommandResult::ProposalGateSettled {
                    run_id: c.run_id,
                    gate_id,
                    journal_id: journal.id.clone(),
                    gate_generation_id: closeout_tx_result.gate_generation_id,
                    readiness_generation_id: closeout_tx_result.readiness_generation_id,
                })
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
        let retry_authority_id = format!("p091-retry-authority:{}", new_stage.id);
        let retry_tx_started = Instant::now();
        let mut retry_tx = self
            .begin_command_transaction("command.RetryStage", journal.id.clone())
            .await?;
        record_command_journal_tx(&mut retry_tx, journal).await?;
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
        retry_stage_execution_authorities::supersede_active_for_stage_tx(
            &mut retry_tx,
            run_id,
            stage_id,
            now,
            "superseded_by_new_retry",
        )
        .await?;
        retry_stage_execution_authorities::create_tx(
            &mut retry_tx,
            &RetryStageExecutionAuthority {
                id: retry_authority_id.clone(),
                run_id,
                stage_id: stage_id.to_string(),
                target_stage_execution_id: new_stage.id,
                entry_kind: RetryAuthorityEntryKind::FullStageRetry,
                source_command_journal_id: Some(journal_id.to_string()),
                source_retry_work_item_id: Some(retry_advance_work_item_id.clone()),
                source_invoke_work_item_id: None,
                source_agent_execution_id: None,
                authority_state: RetryAuthorityState::Active,
                created_at: now,
                updated_at: now,
                terminal_reason: None,
            },
        )
        .await?;
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
            .execute(&mut **retry_tx)
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
                id: retry_advance_work_item_id.clone(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": stage_id,
                    "target_stage_execution_id": new_stage.id.to_string(),
                    "retry_authority_id": retry_authority_id,
                    "source_stage_execution_id": old_stage.id.to_string(),
                    "source_work_item_id": retry_advance_work_item_id,
                    "enqueue_reason": "retry_stage",
                    "reason": retry_reason
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
        let has_release_post_approval_tasks = match retry_state_has_release_post_approval_tasks(
            &run,
            &old_stage.stage_id,
        ) {
            Ok(has_release_post_approval_tasks) => has_release_post_approval_tasks,
            Err(e) => {
                warn!(
                    run_id = %run_id,
                    stage_id = %old_stage.stage_id,
                    error = %e,
                    "RetryAgentExecution side-effect preflight could not inspect post_approval_tasks"
                );
                false
            }
        };
        if retry_requires_effect_reconciliation(
            &old_stage,
            Some(&target_exec.agent_id),
            has_release_post_approval_tasks,
        ) {
            let error = requires_effect_reconciliation_error(&old_stage);
            self.record_failed_command_transaction(
                journal,
                "command.RetryAgentExecution",
                &error.to_string(),
            )
            .await?;
            return Err(error);
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
        let p088_completion_retry_evidence =
            code_writer_completion_receipts::find_by_execution_id(&self.pool, agent_execution_id)
                .await?
                .and_then(|readback| {
                    readback
                        .receipt
                        .failed_stage_evidence_path
                        .clone()
                        .or(readback.receipt.receipt_artifact_path.clone())
                });
        let provider_fallback =
            targeted_retry_catalog_profile_override(&run, &target_exec.agent_id, &retry_payload)
                .or_else(|| {
                    targeted_retry_provider_fallback(
                        &run,
                        &target_exec.agent_id,
                        &retry_payload,
                        runtime_facts.as_ref(),
                    )
                });
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
        let retry_authority_id = format!("p091-retry-authority:{}", new_stage.id);
        if let Some(object) = retry_payload.as_object_mut() {
            object.insert("run_id".into(), serde_json::json!(run_id.to_string()));
            object.insert("stage_id".into(), serde_json::json!(stage_id));
            object.insert(
                "stage_execution_id".into(),
                serde_json::json!(new_stage.id.to_string()),
            );
            object.insert(
                "target_stage_execution_id".into(),
                serde_json::json!(new_stage.id.to_string()),
            );
            object.insert(
                "retry_authority_id".into(),
                serde_json::json!(retry_authority_id.clone()),
            );
            object.insert(
                "source_stage_execution_id".into(),
                serde_json::json!(old_stage.id.to_string()),
            );
            object.insert(
                "source_agent_execution_id".into(),
                serde_json::json!(agent_execution_id.to_string()),
            );
            object.insert(
                "source_work_item_id".into(),
                serde_json::json!(source_item.id.clone()),
            );
            object.remove("p058_claimed");
            object.insert(
                "targeted_retry".into(),
                serde_json::json!({
                    "journal_id": journal_id,
                    "retry_authority_id": retry_authority_id.clone(),
                    "target_stage_execution_id": new_stage.id.to_string(),
                    "source_stage_execution_id": old_stage.id.to_string(),
                    "source_agent_execution_id": agent_execution_id.to_string(),
                    "source_work_item_id": source_item.id.clone(),
                    "reason": "operator_targeted_retry"
                }),
            );
            if let Some(evidence_path) = p088_completion_retry_evidence.as_deref() {
                attach_p088_operator_retry_completion_recovery_payload(
                    object,
                    &agent_execution_id.to_string(),
                    evidence_path,
                );
            }
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
                if let Some(temperature) = fallback.temperature {
                    object.insert("temperature".into(), serde_json::json!(temperature));
                }
                if let Some(targeted_retry) = object
                    .get_mut("targeted_retry")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    targeted_retry.insert(
                        "provider_fallback".into(),
                        serde_json::json!({
                            "reason": fallback.reason,
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
        let mut retry_tx = self
            .begin_command_transaction("command.RetryAgentExecution", journal.id.clone())
            .await?;
        record_command_journal_tx(&mut retry_tx, journal).await?;
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
        let authority = retry_stage_execution_authorities::create_active_targeted_agent_retry_tx(
            &mut retry_tx,
            run_id,
            stage_id,
            new_stage.id,
            Some(journal_id.to_string()),
            None,
            retry_work_item_id.clone(),
            Some(agent_execution_id.to_string()),
            now,
        )
        .await?;
        debug_assert_eq!(authority.id, retry_authority_id);
        sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
            .bind(RunStatus::Running.to_string())
            .bind(stage_id)
            .bind(run_id.to_string())
            .execute(&mut **retry_tx)
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
        let mut tx = self
            .begin_command_transaction(context, journal.id.clone())
            .await?;
        record_command_journal_tx(&mut tx, journal).await?;
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
        let mut tx = self
            .begin_command_transaction(context, journal.id.clone())
            .await?;
        record_command_journal_tx(&mut tx, journal).await?;
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

        let plan = match compile_run_plan_for_run(&run) {
            Ok(Some(plan)) => plan,
            Ok(None) => return false,
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

/// P077 BLK-010: Validate that the caller-supplied capability matches the canonical
/// CapabilityToolId::ProposalGateSettle token and that the authority is in the
/// registered allow-list. Prevents arbitrary capability/authority strings from
/// polluting the audit lineage. Extracted for unit-testability.
fn validate_proposal_gate_authorization(c: &SettleProposalGateCmd) -> Result<()> {
    // Hard-coded literal to avoid a theoretical fail-open if enum serialization
    // ever returned Err and unwrap_or_default produced an empty string that
    // compared equal to an empty caller capability (p077-sec-005).
    const CANONICAL_CAPABILITY: &str = "ProposalGateSettle";
    if c.capability.trim().is_empty() {
        anyhow::bail!("empty capability: must be '{}'", CANONICAL_CAPABILITY);
    }
    if c.capability != CANONICAL_CAPABILITY {
        anyhow::bail!(
            "invalid capability '{}': must be '{}'",
            c.capability,
            CANONICAL_CAPABILITY
        );
    }
    const ALLOWED_AUTHORITIES: &[&str] =
        &["release_owner", "control_plane_owner", "proposal_owner"];
    if !ALLOWED_AUTHORITIES.contains(&c.authority.as_str()) {
        anyhow::bail!(
            "invalid authority '{}': must be one of {:?}",
            c.authority,
            ALLOWED_AUTHORITIES
        );
    }
    Ok(())
}

fn validate_accepted_risk_lineage(c: &SettleProposalGateCmd) -> Result<()> {
    for risk in &c.accepted_risks {
        if let Err(errors) = risk.validate() {
            anyhow::bail!(
                "invalid accepted risk lineage for '{}': {}",
                risk.risk_id,
                errors.join("; ")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::{IdeaId, RunId};

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
    fn p084_rollout_preflight_policy_is_server_stamped_into_delivery_preflight() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-02T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let journal = CommandJournalEntry {
            id: "journal-1".into(),
            command_type: "StartRun",
            payload_json: "{}".into(),
            run_id: None,
            created_at: now,
            caller_surface: Some("mcp".into()),
            caller_principal_id: Some("operator-1".into()),
            caller_principal_class: Some("operator".into()),
            caller_tool: Some("runs.start".into()),
            request_id: None,
            caller_class: None,
            token_id: None,
        };
        let caller = CallerContext::mcp(
            "operator-1",
            &domain::PrincipalClass::Operator,
            "runs.start",
        );
        let raw_policy = serde_json::json!({
            "waiver": {
                "state": "active",
                "decision": "waive",
                "reason_code": "emergency_override",
                "expires_at": "2026-05-03T12:00:00Z"
            },
            "enforcement_mode": {
                "mode": "permissive",
                "reason_code": "dogfood_window",
                "expires_at": "2026-05-03T12:00:00Z"
            }
        })
        .to_string();

        let merged = merge_rollout_contract_preflight_policy(
            Some(r#"{"passed":true,"checks":[]}"#.into()),
            Some(&raw_policy),
            &journal,
            &caller,
            now,
        )
        .unwrap()
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let waiver = &value["rollout_contract_preflight"]["waiver"];

        assert_eq!(value["passed"], serde_json::json!(true));
        assert_eq!(waiver["authorized"], serde_json::json!(true));
        assert_eq!(waiver["principal_id"], serde_json::json!("operator-1"));
        assert_eq!(waiver["audit_event_id"], serde_json::json!("journal-1"));
    }

    #[test]
    fn p084_rollout_preflight_policy_rejects_principal_spoofing() {
        let now = Utc::now();
        let journal = CommandJournalEntry {
            id: "journal-1".into(),
            command_type: "StartRun",
            payload_json: "{}".into(),
            run_id: None,
            created_at: now,
            caller_surface: Some("mcp".into()),
            caller_principal_id: Some("operator-1".into()),
            caller_principal_class: Some("operator".into()),
            caller_tool: Some("runs.start".into()),
            request_id: None,
            caller_class: None,
            token_id: None,
        };
        let caller = CallerContext::mcp(
            "operator-1",
            &domain::PrincipalClass::Operator,
            "runs.start",
        );
        let raw_policy = serde_json::json!({
            "waiver": {
                "state": "active",
                "decision": "waive",
                "reason_code": "emergency_override",
                "expires_at": "2099-01-01T00:00:00Z",
                "principal_id": "spoofed"
            }
        })
        .to_string();

        let err = merge_rollout_contract_preflight_policy(
            None,
            Some(&raw_policy),
            &journal,
            &caller,
            now,
        )
        .expect_err("spoofed principal_id must fail closed");
        assert!(err.to_string().contains("server-stamped"));
    }

    #[test]
    fn p084_rollout_preflight_policy_rejects_oversized_payload() {
        let now = Utc::now();
        let journal = CommandJournalEntry {
            id: "journal-1".into(),
            command_type: "StartRun",
            payload_json: "{}".into(),
            run_id: None,
            created_at: now,
            caller_surface: Some("mcp".into()),
            caller_principal_id: Some("operator-1".into()),
            caller_principal_class: Some("operator".into()),
            caller_tool: Some("runs.start".into()),
            request_id: None,
            caller_class: None,
            token_id: None,
        };
        let caller = CallerContext::mcp(
            "operator-1",
            &domain::PrincipalClass::Operator,
            "runs.start",
        );
        let raw_policy = " ".repeat(MAX_ROLLOUT_CONTRACT_PREFLIGHT_POLICY_JSON_BYTES + 1);

        let err = merge_rollout_contract_preflight_policy(
            None,
            Some(&raw_policy),
            &journal,
            &caller,
            now,
        )
        .expect_err("oversized policy payload must fail closed");

        assert!(err.to_string().contains("exceeds maximum length"));
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

    #[test]
    fn compile_run_plan_prefers_frozen_snapshots_over_yaml_paths() {
        let workflow_path =
            "/Users/user/Documents/Chainworks Forge/examples/workflows/full-mvp-live.yaml";
        let catalog_path = "/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml";
        let frozen = workflow::compiler::compile(workflow_path, catalog_path)
            .expect("example workflow should compile for snapshot fixture");

        let run = Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status: RunStatus::Running,
            workflow_id: "full-mvp-live".into(),
            workflow_title: "Full MVP Live".into(),
            workspace_root: "/tmp".into(),
            artifact_root: "/tmp".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_11_manual_release".into()),
            workflow_yaml_path: Some("/definitely/missing/workflow.yaml".into()),
            agent_catalog_yaml_path: Some("/definitely/missing/agents.yaml".into()),
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some(frozen.workflow_snapshot_hash.clone()),
            catalog_snapshot_hash: Some(frozen.catalog_snapshot_hash.clone()),
            workflow_snapshot_json: Some(frozen.workflow_snapshot_json.clone()),
            catalog_snapshot_json: Some(frozen.catalog_snapshot_json.clone()),
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        };

        let plan = compile_run_plan_for_run(&run)
            .expect("snapshot-backed run should compile")
            .expect("plan should exist");

        assert!(
            plan.states
                .get("state_11_manual_release")
                .is_some_and(|state| !state.post_approval_tasks.is_empty()),
            "snapshot-backed compile should not depend on YAML paths once the run is frozen"
        );
    }

    fn p077_settle_cmd(
        action: domain::commands::ProposalGateSettlementAction,
    ) -> domain::commands::SettleProposalGateCmd {
        domain::commands::SettleProposalGateCmd {
            run_id: RunId::new(),
            proposal_id: "077".into(),
            stage_id: "state_9_implementation_reviewed".into(),
            action,
            principal: "operator-1".into(),
            capability: "ProposalGateSettle".into(),
            journal_id: "journal-input".into(),
            authority: "release_owner".into(),
            reason: "operator settled proof gate".into(),
            source_artifacts: vec!["implementation/self-assessment.json".into()],
            workflow_digest: "sha256:workflow".into(),
            worktree_head: "abcdef123456".into(),
            dirty_or_changed_file_digest: "sha256:dirty".into(),
            source_generation_ids: vec!["generation-1".into()],
            current_fingerprint: "sha256:fingerprint-current".into(),
            timeout_ms: None,
            receipt_json: None,
            accepted_risks: Vec::new(),
        }
    }

    #[test]
    fn p077_record_settlement_populates_executor_metadata() {
        let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::RecordSettlement);

        let result = build_proposal_gate_result_from_settlement(
            &cmd,
            "journal-1",
            "p077:077",
            "gate-generation-1",
            9,
        )
        .expect("record settlement should execute the managed gate path");

        assert_eq!(result.status, ProposalGateStatus::Passed);
        assert_eq!(
            result.executor_version.as_deref(),
            Some("proposal-gate-executor.v1")
        );
        assert!(result
            .evidence_digest
            .as_deref()
            .is_some_and(|d| { d.starts_with("sha256:") && d.len() == "sha256:".len() + 64 }));
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.elapsed_ms, Some(9));
        assert_eq!(
            result
                .authorization_lineage
                .as_ref()
                .map(|l| l.current_fingerprint.as_str()),
            Some("sha256:fingerprint-current")
        );
    }

    #[test]
    fn p077_import_receipt_rejects_fingerprint_mismatch() {
        let mut cmd =
            p077_settle_cmd(domain::commands::ProposalGateSettlementAction::ImportReceipt);
        cmd.receipt_json = Some(serde_json::json!({
            "schema_version": "proposal_gate_receipt.v1",
            "status": "passed",
            "proposal_id": "077",
            "run_id": cmd.run_id.to_string(),
            "stage_id": "state_9_implementation_reviewed",
            "executor_version": "proposal-gate-executor.v1",
            "evidence_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "stdout_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "stderr_digest": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "exit_code": 0,
            "elapsed_ms": 17,
            "current_fingerprint": "sha256:stale"
        }).to_string());

        let err = build_proposal_gate_result_from_settlement(
            &cmd,
            "journal-1",
            "p077:077",
            "gate-generation-1",
            9,
        )
        .expect_err("stale receipt must be rejected before activation");

        assert!(err.to_string().contains("current_fingerprint"));
    }

    #[test]
    fn p077_import_receipt_rejects_unmanaged_receipt_without_digest() {
        let mut cmd =
            p077_settle_cmd(domain::commands::ProposalGateSettlementAction::ImportReceipt);
        cmd.receipt_json = Some(serde_json::json!({
            "schema_version": "proposal_gate_receipt.v1",
            "status": "passed",
            "proposal_id": "077",
            "run_id": cmd.run_id.to_string(),
            "stage_id": "state_9_implementation_reviewed",
            "executor_version": "proposal-gate-executor.v1",
            "stdout_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "stderr_digest": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "exit_code": 0,
            "elapsed_ms": 17,
            "current_fingerprint": "sha256:fingerprint-current"
        }).to_string());

        let err = build_proposal_gate_result_from_settlement(
            &cmd,
            "journal-1",
            "p077:077",
            "gate-generation-1",
            9,
        )
        .expect_err("receipt without durable evidence digest is unmanaged");

        assert!(err.to_string().contains("evidence_digest"));
    }

    #[test]
    fn p077_execute_gate_generates_passed_managed_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let scripts = temp.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        let gate = scripts.join("test-gate.sh");
        std::fs::write(
            &gate,
            "#!/usr/bin/env bash\nset -euo pipefail\necho proposal-077-ok\n",
        )
        .unwrap();
        make_executable(&gate);

        let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        let raw = execute_managed_proposal_gate_receipt(&cmd, "p077:077", temp.path())
            .expect("managed executor should produce a receipt");
        let receipt: ProposalGateReceiptV1 = serde_json::from_str(&raw).unwrap();

        assert_eq!(receipt.schema_version, PROPOSAL_GATE_RECEIPT_SCHEMA_VERSION);
        assert_eq!(receipt.executor_version, PROPOSAL_GATE_EXECUTOR_VERSION);
        assert_eq!(receipt.status, "passed");
        assert_eq!(receipt.exit_code, 0);
        assert_eq!(receipt.stdout_digest, sha256_digest(b"proposal-077-ok\n"));
        assert_eq!(receipt.stderr_digest, sha256_digest(b""));
        assert!(receipt.evidence_digest.starts_with("sha256:"));
        assert_eq!(receipt.current_fingerprint, "sha256:fingerprint-current");
    }

    #[test]
    fn p077_execute_gate_output_digests_follow_actual_stdout_and_stderr() {
        let run_with_output = |stdout: &str, stderr: &str| {
            let temp = tempfile::tempdir().unwrap();
            let scripts = temp.path().join("scripts");
            std::fs::create_dir_all(&scripts).unwrap();
            let gate = scripts.join("test-gate.sh");
            std::fs::write(
                &gate,
                format!(
                    "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s' '{}'\nprintf '%s' '{}' >&2\n",
                    stdout, stderr
                ),
            )
            .unwrap();
            make_executable(&gate);

            let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
            let raw = execute_managed_proposal_gate_receipt(&cmd, "p077:077", temp.path())
                .expect("managed executor should produce receipt");
            serde_json::from_str::<ProposalGateReceiptV1>(&raw).unwrap()
        };

        let first = run_with_output("stdout-a", "stderr-a");
        let second = run_with_output("stdout-b", "stderr-b");

        assert_eq!(first.stdout_digest, sha256_digest(b"stdout-a"));
        assert_eq!(first.stderr_digest, sha256_digest(b"stderr-a"));
        assert_eq!(second.stdout_digest, sha256_digest(b"stdout-b"));
        assert_eq!(second.stderr_digest, sha256_digest(b"stderr-b"));
        assert_ne!(first.stdout_digest, second.stdout_digest);
        assert_ne!(first.stderr_digest, second.stderr_digest);
    }

    #[test]
    fn p077_execute_gate_generates_failed_receipt_for_nonzero_exit() {
        let temp = tempfile::tempdir().unwrap();
        let scripts = temp.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        let gate = scripts.join("test-gate.sh");
        std::fs::write(
            &gate,
            "#!/usr/bin/env bash\nset -euo pipefail\necho p077-failed >&2\nexit 17\n",
        )
        .unwrap();
        make_executable(&gate);

        let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        let raw = execute_managed_proposal_gate_receipt(&cmd, "p077:077", temp.path())
            .expect("managed executor should preserve failed receipts");
        let receipt: ProposalGateReceiptV1 = serde_json::from_str(&raw).unwrap();

        assert_eq!(receipt.status, "failed");
        assert_eq!(receipt.exit_code, 17);
        assert_eq!(receipt.stdout_digest, sha256_digest(b""));
        assert_eq!(receipt.stderr_digest, sha256_digest(b"p077-failed\n"));
        assert_eq!(
            receipt.failure_classification.as_deref(),
            Some("code_owned_budget_remaining")
        );
        assert!(receipt
            .diagnostic_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("proposal-077 gate failed")));
    }

    #[test]
    fn p077_execute_gate_generates_failed_receipt_for_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let scripts = temp.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        let gate = scripts.join("test-gate.sh");
        std::fs::write(&gate, "#!/usr/bin/env bash\nset -euo pipefail\nsleep 2\n").unwrap();
        make_executable(&gate);

        let mut cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        cmd.timeout_ms = Some(10);
        let raw = execute_managed_proposal_gate_receipt(&cmd, "p077:077", temp.path())
            .expect("managed executor timeout should preserve a failed receipt");
        let receipt: ProposalGateReceiptV1 = serde_json::from_str(&raw).unwrap();

        assert_eq!(receipt.status, "failed");
        assert_eq!(receipt.exit_code, PROPOSAL_GATE_EXECUTOR_TIMEOUT_EXIT_CODE);
        assert_eq!(
            receipt.failure_classification.as_deref(),
            Some("unclear_or_non_code_owned")
        );
        assert!(receipt
            .diagnostic_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("timed out")));
    }

    #[test]
    fn p077_execute_gate_errors_when_script_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        let err = execute_managed_proposal_gate_receipt(&cmd, "p077:077", temp.path())
            .expect_err("missing managed executor script must fail before settlement");
        assert!(err.to_string().contains("could not find"));
    }

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &std::path::Path) {}

    // ── BLK-010: capability/authority binding rejection tests ─────────────────

    #[test]
    fn p077_mismatched_capability_is_rejected() {
        let mut cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        cmd.capability = "SomeOtherCapability".into();

        let err = validate_proposal_gate_authorization(&cmd)
            .expect_err("mismatched capability must be rejected");

        assert!(
            err.to_string().contains("invalid capability"),
            "error should mention invalid capability: {err}"
        );
        assert!(
            err.to_string().contains("ProposalGateSettle"),
            "error should name the expected canonical token: {err}"
        );
    }

    #[test]
    fn p077_unknown_authority_is_rejected() {
        let mut cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        cmd.authority = "rogue_authority".into();

        let err = validate_proposal_gate_authorization(&cmd)
            .expect_err("unknown authority must be rejected");

        assert!(
            err.to_string().contains("invalid authority"),
            "error should mention invalid authority: {err}"
        );
        assert!(
            err.to_string().contains("release_owner"),
            "error should name at least one allowed authority: {err}"
        );
    }

    #[test]
    fn p077_canonical_capability_and_known_authority_pass_validation() {
        let cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        validate_proposal_gate_authorization(&cmd)
            .expect("canonical capability + known authority must pass validation");
    }

    #[test]
    fn p077_all_allowed_authorities_pass_validation() {
        for authority in ["release_owner", "control_plane_owner", "proposal_owner"] {
            let mut cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
            cmd.authority = authority.into();
            validate_proposal_gate_authorization(&cmd)
                .unwrap_or_else(|e| panic!("authority '{authority}' should pass: {e}"));
        }
    }

    #[test]
    fn workflow_conflict_resolution_accepts_lowercase_loop_budget_exhausted_candidate() {
        let candidate = CandidateTransitionEvaluation {
            transition_id: "state_9_implementation_reviewed__to__state_10_implementation_refined__1"
                .into(),
            from_state_id: "state_9_implementation_reviewed".into(),
            to_state_id: "state_10_implementation_refined".into(),
            condition_expression_id: Some("transition_condition_1".into()),
            result: CandidateTransitionResult::NotMatched,
            required_artifacts: vec!["implementation_review_summary".into()],
            missing_artifacts: vec![],
            missing_fields: vec![],
            source_artifact_ids: vec!["implementation_review_summary".into()],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(
                "implementation_review_summary.status=needs_code_fixes requires refinement, but loop budget exhausted for implementation_revision_count: 12/12 iterations"
                    .into(),
            ),
        };

        validate_operator_selected_candidate(&candidate)
            .expect("operator override should accept orchestrator loop-budget diagnostics");
    }

    #[test]
    fn p077_empty_capability_is_rejected() {
        let mut cmd = p077_settle_cmd(domain::commands::ProposalGateSettlementAction::Execute);
        cmd.capability = "".into();
        let err = validate_proposal_gate_authorization(&cmd)
            .expect_err("empty capability must always be rejected");
        assert!(
            err.to_string().contains("empty capability")
                || err.to_string().contains("invalid capability"),
            "error should describe the empty-capability rejection: {err}"
        );
    }

    #[test]
    fn proposal_088_targeted_retry_payload_carries_completion_recovery_evidence() {
        let mut payload = serde_json::json!({
            "run_id": "run-1",
            "stage_id": "state_7_implementation_started",
            "agent_id": "code_writer"
        });

        attach_p088_operator_retry_completion_recovery_payload(
            payload.as_object_mut().expect("payload object"),
            "agent-exec-1",
            ".chainworks/evidence/p088/failed-stage.json",
        );

        assert_eq!(
            payload
                .pointer("/p088/activation_source")
                .and_then(serde_json::Value::as_str),
            Some("operator_retry_completion_recovery")
        );
        assert_eq!(
            payload
                .pointer("/p088/preserved_historical_evidence_packet_path")
                .and_then(serde_json::Value::as_str),
            Some(".chainworks/evidence/p088/failed-stage.json")
        );
        assert_eq!(
            payload
                .get("retry_reason")
                .and_then(serde_json::Value::as_str),
            Some("operator_retry_completion_recovery")
        );
    }
}
