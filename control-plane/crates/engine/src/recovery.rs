use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::{info, warn};

use acp::AcpRuntimeManager;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, approvals, artifact_contracts,
    code_writer_completion_receipts, escalation, output_contract_repair as ocr_repo, projections,
    retry_payload_recovery_events, retry_stage_execution_authorities, runs, sessions, side_effects,
    stages, startup_repairs, work_items, workflow_conflicts,
};
use db::work_item::{WorkItemKind, WorkItemStatus};
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter};
use domain::agent::{
    AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, AgentStatus,
    OperatorActionHint,
};
use domain::approval::ApprovalDecision;
use domain::code_writer_completion::{
    CodeWriterCompletionOutputDecisionRecord, CodeWriterCompletionReceiptRecord,
    CodeWriterCompletionTextCaptureRecord,
};
use domain::escalation::EscalationEvent;
use domain::provider::InvokeAgentCapacityConfig;
use domain::retry_authority::{RetryAuthorityState, RetryPayloadRecoveryEvent};
use domain::run::Run;
use domain::stage::{StageSettlementKind, StageStatus};
use sqlx::Row;

use crate::event_bus::EventSender;
use crate::work_queue::WorkQueue;

pub struct RecoveryService {
    pool: SqlitePool,
    work_queue: WorkQueue,
    #[allow(dead_code)]
    events: EventSender,
    db_writer: Arc<DbWriter>,
    acp: Option<Arc<AcpRuntimeManager>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum P092RecoveryMode {
    Diagnostic,
    Enforce,
}

impl P092RecoveryMode {
    fn from_env() -> Self {
        match std::env::var("CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_MODE")
            .unwrap_or_else(|_| "diagnostic".to_string())
            .as_str()
        {
            "enforce" => Self::Enforce,
            _ => Self::Diagnostic,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Diagnostic => "diagnostic",
            Self::Enforce => "enforce",
        }
    }
}

struct P092RecoverySummary {
    candidates: usize,
    excluded: usize,
    repaired: usize,
    mode: &'static str,
    disabled: bool,
}

#[derive(Clone, Debug)]
struct P086OrphanReapOutcome {
    attempted: bool,
    verified: bool,
    error: Option<String>,
    policy: &'static str,
    signals_sent: Vec<&'static str>,
    term_deadline_ms: u64,
    kill_deadline_ms: u64,
    started_at: String,
    completed_at: String,
}

fn p086_process_exists(pid: libc::pid_t) -> bool {
    if pid <= 0 {
        return false;
    }
    let mut status: libc::c_int = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status as *mut libc::c_int, libc::WNOHANG) };
    if waited == pid {
        return false;
    }
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return true;
    }
    matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    )
}

fn p086_process_group_id(pid: libc::pid_t) -> Option<libc::pid_t> {
    if pid <= 0 {
        return None;
    }
    let pgid = unsafe { libc::getpgid(pid) };
    (pgid > 0).then_some(pgid)
}

async fn p086_reap_registered_provider_process_group(
    child_pid: Option<i64>,
    process_group_id: Option<i64>,
    process_uid: Option<i64>,
) -> P086OrphanReapOutcome {
    let started_at = Utc::now().to_rfc3339();
    let Some(child_pid) = child_pid.and_then(|pid| (pid > 0).then_some(pid as libc::pid_t)) else {
        let completed_at = Utc::now().to_rfc3339();
        return P086OrphanReapOutcome {
            attempted: false,
            verified: false,
            error: Some("missing_provider_process_binding".to_string()),
            policy: "missing_durable_provider_process_binding",
            signals_sent: Vec::new(),
            term_deadline_ms: 0,
            kill_deadline_ms: 0,
            started_at,
            completed_at,
        };
    };
    let pgid = process_group_id
        .and_then(|pid| (pid > 0).then_some(pid as libc::pid_t))
        .unwrap_or(child_pid);
    let current_uid = unsafe { libc::getuid() } as i64;
    if let Some(expected_uid) = process_uid {
        if expected_uid != current_uid {
            let completed_at = Utc::now().to_rfc3339();
            return P086OrphanReapOutcome {
                attempted: true,
                verified: false,
                error: Some(format!(
                    "provider_process_uid_mismatch: expected {expected_uid}, current {current_uid}"
                )),
                policy: "registered_process_group_uid_guard",
                signals_sent: Vec::new(),
                term_deadline_ms: 0,
                kill_deadline_ms: 0,
                started_at,
                completed_at,
            };
        }
    }
    if !p086_process_exists(child_pid) {
        let completed_at = Utc::now().to_rfc3339();
        return P086OrphanReapOutcome {
            attempted: true,
            verified: true,
            error: None,
            policy: "registered_process_group_already_exited",
            signals_sent: Vec::new(),
            term_deadline_ms: 0,
            kill_deadline_ms: 0,
            started_at,
            completed_at,
        };
    }
    match p086_process_group_id(child_pid) {
        Some(actual_pgid) if actual_pgid == pgid => {}
        Some(actual_pgid) => {
            let completed_at = Utc::now().to_rfc3339();
            return P086OrphanReapOutcome {
                attempted: true,
                verified: false,
                error: Some(format!(
                    "provider_process_group_mismatch: expected {pgid}, actual {actual_pgid}"
                )),
                policy: "registered_process_group_pgid_guard",
                signals_sent: Vec::new(),
                term_deadline_ms: 0,
                kill_deadline_ms: 0,
                started_at,
                completed_at,
            };
        }
        None => {
            let completed_at = Utc::now().to_rfc3339();
            return P086OrphanReapOutcome {
                attempted: true,
                verified: true,
                error: None,
                policy: "registered_process_group_gone_before_signal",
                signals_sent: Vec::new(),
                term_deadline_ms: 0,
                kill_deadline_ms: 0,
                started_at,
                completed_at,
            };
        }
    }

    let target = -pgid;
    let mut signals_sent = vec!["SIGTERM"];
    let term_deadline_ms = 200;
    let kill_deadline_ms = 200;
    unsafe {
        libc::kill(target, libc::SIGTERM);
    }
    tokio::time::sleep(Duration::from_millis(term_deadline_ms)).await;
    if p086_process_exists(child_pid) {
        signals_sent.push("SIGKILL");
        unsafe {
            libc::kill(target, libc::SIGKILL);
        }
        tokio::time::sleep(Duration::from_millis(kill_deadline_ms)).await;
    }
    let verified = !p086_process_exists(child_pid);
    let completed_at = Utc::now().to_rfc3339();
    P086OrphanReapOutcome {
        attempted: true,
        verified,
        error: (!verified).then(|| "provider_process_still_live_after_signal".to_string()),
        policy: "registered_provider_process_group_signal",
        signals_sent,
        term_deadline_ms,
        kill_deadline_ms,
        started_at,
        completed_at,
    }
}

fn p092_recovery_disabled() -> bool {
    std::env::var("CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_DISABLED")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn p092_recovery_batch_limit() -> usize {
    std::env::var("CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(100))
        .unwrap_or(20)
}

fn p092_stale_payload_fields(
    payload_target_stage_execution_id: Option<&str>,
    current_target_stage_execution_id: &str,
    top_source_stage_execution_id: Option<&str>,
    top_source_agent_execution_id: Option<&str>,
    top_source_work_item_id: Option<&str>,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if payload_target_stage_execution_id
        .is_some_and(|value| value != current_target_stage_execution_id)
    {
        fields.push("target_stage_execution_id");
    }
    if top_source_stage_execution_id.is_some() {
        fields.push("source_stage_execution_id");
    }
    if top_source_agent_execution_id.is_some() {
        fields.push("source_agent_execution_id");
    }
    if top_source_work_item_id.is_some() {
        fields.push("source_work_item_id");
    }
    fields
}

fn p092_recovery_reason_code(stale_fields: &[&'static str]) -> &'static str {
    if stale_fields.contains(&"target_stage_execution_id") {
        "retry_payload_stale_target_stage_repaired"
    } else if stale_fields
        .iter()
        .any(|field| field.starts_with("source_"))
    {
        "retry_payload_source_provenance_ignored_for_target"
    } else {
        "valid_retry_invoke_completion_recovered"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    #[test]
    fn proposal_058_recovery_action_uses_failure_kind_and_output_settlement() {
        let mut facts = domain::agent::AgentExecutionRuntimeFacts::defaults_for(
            domain::ids::AgentExecutionId::new(),
            Utc::now(),
        );
        facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;

        assert_eq!(
            recovery_action_from_runtime_facts(Some(&facts)),
            ("wait_until_retry_after", "provider_quota_retry_budget")
        );

        facts.failure_kind = None;
        facts.output_settlement = AgentOutputSettlement::ValidOutputsFromFailedExecution;
        assert_eq!(
            recovery_action_from_runtime_facts(Some(&facts)),
            (
                "accept_or_retry_degraded_outputs",
                "valid_outputs_from_failed_execution"
            )
        );
    }

    #[tokio::test]
    async fn p086_reap_registered_provider_process_group_kills_test_process_group() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .expect("spawn test sleep process");
        let pid = child.id() as libc::pid_t;
        let pgid = p086_process_group_id(pid).expect("test child must have process group");

        let outcome = p086_reap_registered_provider_process_group(
            Some(pid as i64),
            Some(pgid as i64),
            Some(unsafe { libc::getuid() } as i64),
        )
        .await;

        assert!(outcome.attempted);
        assert!(outcome.verified, "unexpected reap outcome: {outcome:?}");
        assert_eq!(outcome.policy, "registered_provider_process_group_signal");
        assert!(outcome.signals_sent.contains(&"SIGTERM"));
        assert!(outcome.term_deadline_ms > 0);
        assert!(outcome.kill_deadline_ms > 0);
        let _ = child.kill();
    }

    #[tokio::test]
    async fn p086_reap_registered_provider_process_group_fails_closed_on_uid_mismatch() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .expect("spawn test sleep process");
        let pid = child.id() as libc::pid_t;
        let pgid = p086_process_group_id(pid).expect("test child must have process group");
        let current_uid = unsafe { libc::getuid() } as i64;

        let outcome = p086_reap_registered_provider_process_group(
            Some(pid as i64),
            Some(pgid as i64),
            Some(current_uid + 1),
        )
        .await;

        assert!(outcome.attempted);
        assert!(!outcome.verified);
        assert_eq!(outcome.policy, "registered_process_group_uid_guard");
        assert!(p086_process_exists(pid));
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub struct RecoverySummary {
    pub runs_inspected: usize,
    pub runs_repaired: usize,
    pub work_items_requeued: usize,
    pub agent_executions_settled: u64,
    pub recovered_item_count: i64,
    pub queued_under_startup_recovery_backpressure_count: i64,
    pub oldest_recovered_queued_age_ms: Option<i64>,
    pub affected_run_count: i64,
    pub next_retry_or_backoff_time: Option<chrono::DateTime<Utc>>,
}

#[derive(Deserialize)]
struct P088CompletionReceiptArtifact {
    schema_version: String,
    receipt: CodeWriterCompletionReceiptRecord,
    #[serde(default)]
    text_captures: Vec<CodeWriterCompletionTextCaptureRecord>,
    #[serde(default)]
    output_decisions: Vec<CodeWriterCompletionOutputDecisionRecord>,
}

pub async fn persist_failed_stage_recovery_snapshot(
    pool: &SqlitePool,
    stage_execution_id: domain::ids::StageExecutionId,
    failed_at: chrono::DateTime<Utc>,
) -> Result<String> {
    let snapshot = match stages::find_by_id(pool, stage_execution_id).await? {
        Some(stage) => {
            let executions = agent_executions::find_by_stage(pool, stage_execution_id).await?;
            let latest_execution = executions.last();
            let runtime_facts = match latest_execution {
                Some(execution) => {
                    agent_execution_runtime_facts::find_by_execution_id(pool, execution.id).await?
                }
                None => None,
            };
            let (action, reason) = recovery_action_from_runtime_facts(runtime_facts.as_ref());
            serde_json::json!({
                "status": "available",
                "action": action,
                "reason": reason,
                "stage_execution_id": stage_execution_id.to_string(),
                "run_id": stage.run_id.to_string(),
                "stage_id": stage.stage_id,
                "failed_at": failed_at,
                "latest_agent_execution_id": latest_execution.map(|execution| execution.id.to_string()),
                "latest_agent_status": latest_execution.map(|execution| execution.status.to_string()),
                "failure_kind": runtime_facts.as_ref().and_then(|facts| facts.failure_kind.as_ref()).map(ToString::to_string),
                "output_settlement": runtime_facts.as_ref().map(|facts| facts.output_settlement.to_string()),
                "retry_after": runtime_facts.as_ref().and_then(|facts| facts.retry_after.map(|dt| dt.to_rfc3339())),
                "validation_failure_present": stage.validation_failure_json.is_some(),
            })
        }
        None => serde_json::json!({
            "status": "unavailable",
            "reason": "stage_execution_not_found",
            "stage_execution_id": stage_execution_id.to_string(),
            "failed_at": failed_at,
        }),
    };
    let encoded = serde_json::to_string_pretty(&snapshot)?;
    stages::update_recovery_snapshot_json(pool, stage_execution_id, &encoded).await?;
    Ok(encoded)
}

fn recovery_action_from_runtime_facts(
    facts: Option<&domain::agent::AgentExecutionRuntimeFacts>,
) -> (&'static str, &'static str) {
    let Some(facts) = facts else {
        return ("retry_stage", "stage_settled_failed");
    };
    match (&facts.failure_kind, &facts.output_settlement) {
        (Some(AgentFailureKind::ProviderQuota), _) => {
            ("wait_until_retry_after", "provider_quota_retry_budget")
        }
        (Some(AgentFailureKind::ProviderPermissionRequired), _) => {
            ("authorize_provider", "provider_permission_required")
        }
        (Some(AgentFailureKind::McpPermissionModalStall), _) => {
            ("authorize_xcode", "mcp_permission_modal_stall")
        }
        (Some(AgentFailureKind::MissingRequiredOutputs), _)
        | (_, AgentOutputSettlement::MissingRequiredOutputs) => {
            ("inspect_outputs_then_retry", "missing_required_outputs")
        }
        (Some(AgentFailureKind::InvalidOutputContract), _)
        | (_, AgentOutputSettlement::InvalidRequiredOutputs) => {
            ("inspect_contract_then_retry", "invalid_required_outputs")
        }
        (_, AgentOutputSettlement::ValidOutputsFromFailedExecution) => (
            "accept_or_retry_degraded_outputs",
            "valid_outputs_from_failed_execution",
        ),
        (Some(AgentFailureKind::SupersededByRetry), _) => {
            ("inspect_retry_successor", "superseded_by_retry")
        }
        _ => ("retry_stage", "runtime_failure"),
    }
}

fn inject_retry_authority_id_into_payload(
    payload_json: &str,
    retry_authority_id: &str,
) -> Result<String> {
    let mut payload: serde_json::Value = serde_json::from_str(payload_json)?;
    let Some(object) = payload.as_object_mut() else {
        return Err(anyhow!("auto-contract retry payload is not an object"));
    };
    object.insert(
        "retry_authority_id".to_string(),
        serde_json::json!(retry_authority_id),
    );
    let targeted_retry = object
        .entry("targeted_retry".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(targeted_retry) = targeted_retry.as_object_mut() else {
        return Err(anyhow!(
            "auto-contract retry targeted_retry is not an object"
        ));
    };
    targeted_retry.insert(
        "retry_authority_id".to_string(),
        serde_json::json!(retry_authority_id),
    );
    Ok(serde_json::to_string(&payload)?)
}

async fn stage_has_pending_or_running_invoke_work(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
    stage_execution_id: domain::ids::StageExecutionId,
) -> Result<bool> {
    let items = work_items::list_by_run(pool, run_id).await?;
    let stage_execution_id = stage_execution_id.to_string();
    Ok(items.iter().any(|item| {
        item.kind == WorkItemKind::InvokeAgent
            && matches!(
                item.status,
                WorkItemStatus::Pending | WorkItemStatus::Running
            )
            && serde_json::from_str::<serde_json::Value>(&item.payload_json)
                .ok()
                .and_then(|payload| {
                    payload
                        .get("stage_execution_id")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .as_deref()
                == Some(stage_execution_id.as_str())
    }))
}

async fn stage_has_pending_or_running_advance_work(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
    stage_execution_id: domain::ids::StageExecutionId,
) -> Result<bool> {
    let items = work_items::list_by_run(pool, run_id).await?;
    let stage_execution_id = stage_execution_id.to_string();
    Ok(items.iter().any(|item| {
        item.kind == WorkItemKind::AdvanceRun
            && matches!(
                item.status,
                WorkItemStatus::Pending | WorkItemStatus::Running
            )
            && serde_json::from_str::<serde_json::Value>(&item.payload_json)
                .ok()
                .and_then(|payload| {
                    payload
                        .get("target_stage_execution_id")
                        .or_else(|| payload.get("stage_execution_id"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .as_deref()
                == Some(stage_execution_id.as_str())
    }))
}

async fn run_has_pending_or_running_work(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<bool> {
    let items = work_items::list_by_run(pool, run_id).await?;
    Ok(items.iter().any(|item| {
        matches!(
            item.status,
            WorkItemStatus::Pending | WorkItemStatus::Running
        )
    }))
}

async fn run_has_recovered_p091_orphan(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM stage_executions
           WHERE run_id = ?1
             AND terminal_reason = 'stale_retry_recovered'"#,
    )
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

struct P091OrphanRepairPass {
    id: String,
    run_id: String,
    mode: String,
    disabled: bool,
    candidates_total: i64,
    excluded_total: i64,
    would_repair_total: i64,
    repaired_total: i64,
    disabled_total: i64,
    samples: Vec<serde_json::Value>,
}

impl P091OrphanRepairPass {
    fn from_env(run_id: String) -> Self {
        let disabled = std::env::var("CHAINWORKS_P091_DISABLE_STARTUP_ORPHAN_REPAIR")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let mode = if disabled {
            "disabled".to_string()
        } else {
            match std::env::var("CHAINWORKS_P091_STARTUP_ORPHAN_REPAIR_MODE")
                .unwrap_or_else(|_| "diagnostic".to_string())
                .as_str()
            {
                "enforce" => "enforce".to_string(),
                _ => "diagnostic".to_string(),
            }
        };
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id,
            mode,
            disabled,
            candidates_total: 0,
            excluded_total: 0,
            would_repair_total: 0,
            repaired_total: 0,
            disabled_total: 0,
            samples: Vec::new(),
        }
    }

    fn sample(&mut self, stage_execution_id: String, reason: &str) {
        if self.samples.len() >= 20 {
            return;
        }
        self.samples.push(serde_json::json!({
            "stage_execution_id": stage_execution_id,
            "reason": reason,
        }));
    }

    fn exclude(&mut self, stage_execution_id: String, reason: &str) {
        self.excluded_total += 1;
        self.sample(stage_execution_id, reason);
    }
}

impl RecoveryService {
    pub fn new(pool: SqlitePool, work_queue: WorkQueue, events: EventSender) -> Self {
        let db_writer = DbWriter::new(pool.clone());
        Self {
            pool,
            work_queue,
            events,
            db_writer: Arc::new(db_writer),
            acp: None,
        }
    }

    pub fn new_with_db_writer(
        pool: SqlitePool,
        work_queue: WorkQueue,
        events: EventSender,
        db_writer: Arc<DbWriter>,
    ) -> Self {
        Self {
            pool,
            work_queue,
            events,
            db_writer,
            acp: None,
        }
    }

    pub fn new_with_db_writer_and_acp(
        pool: SqlitePool,
        work_queue: WorkQueue,
        events: EventSender,
        db_writer: Arc<DbWriter>,
        acp: Arc<AcpRuntimeManager>,
    ) -> Self {
        Self {
            pool,
            work_queue,
            events,
            db_writer,
            acp: Some(acp),
        }
    }

    pub fn new_with_capacity(
        pool: SqlitePool,
        _work_queue: WorkQueue,
        events: EventSender,
        invoke_agent_capacity: InvokeAgentCapacityConfig,
    ) -> Self {
        let work_queue = WorkQueue::with_events_and_capacity(
            pool.clone(),
            events.clone(),
            invoke_agent_capacity,
        );
        let db_writer = Arc::new(DbWriter::new(pool.clone()));
        Self {
            pool,
            work_queue,
            events,
            db_writer,
            acp: None,
        }
    }

    async fn begin_transaction(
        &self,
        operation_name: &'static str,
        idempotency_key: impl Into<String>,
    ) -> Result<db::writer::QueuedTransaction> {
        self.db_writer
            .begin_immediate_transaction(
                class_a_operation(operation_name, WriteLane::CriticalBarrier, idempotency_key),
                operation_name,
            )
            .await
    }

    pub async fn run_startup_repair(&self) -> Result<RecoverySummary> {
        let active_runs = runs::list_active(&self.pool).await?;
        let runs_inspected = active_runs.len();
        let mut runs_repaired = 0usize;
        let mut work_items_requeued = 0usize;
        let now = Utc::now();
        for run in &active_runs {
            match self
                .recover_p058_shutdown_drain_force_detach_for_run(run)
                .await
            {
                Ok(recovered) => {
                    if recovered > 0 {
                        work_items_requeued += recovered;
                        warn!(
                            run_id = %run.id,
                            recovered = recovered,
                            "Startup recovery opened P058 provider force-detach replay pauses"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to recover P058 shutdown-drain force-detach state during startup"
                    );
                }
            }
        }
        let agent_executions_settled =
            work_items::settle_terminal_preclaimed_invoke_agent_executions(&self.pool, now).await?;

        info!(runs_inspected = %runs_inspected, "Starting startup recovery");
        if agent_executions_settled > 0 {
            warn!(
                settled = agent_executions_settled,
                "Startup recovery settled terminal preclaimed InvokeAgent executions"
            );
        }
        let p086_recovered = self.repair_p086_stale_continuation_workers().await?;
        if p086_recovered > 0 {
            warn!(
                repaired = p086_recovered,
                "Startup recovery released stale P086 continuation workers"
            );
        }
        let mut p092_recovered = 0usize;
        for run in &active_runs {
            match self
                .recover_p092_retry_payload_candidates_for_run(run, p092_recovery_batch_limit())
                .await
            {
                Ok(summary) => {
                    if summary.candidates > 0 {
                        p092_recovered += summary.repaired;
                        work_items_requeued += summary.repaired;
                        warn!(
                            run_id = %run.id,
                            candidates = summary.candidates,
                            excluded = summary.excluded,
                            repaired = summary.repaired,
                            mode = %summary.mode,
                            disabled = summary.disabled,
                            "Startup recovery inspected P092 retry payload recovery candidates"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to inspect P092 retry payload recovery candidates during startup"
                    );
                }
            }
            match self
                .repair_auto_contract_retry_authority_gaps_for_run(run)
                .await
            {
                Ok(repaired) => {
                    if repaired > 0 {
                        work_items_requeued += repaired;
                        warn!(
                            run_id = %run.id,
                            repaired = repaired,
                            "Startup recovery repaired auto-contract targeted retry authority gaps"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to repair auto-contract targeted retry authority gaps during startup"
                    );
                }
            }
            match self
                .recover_failed_terminal_targeted_advance_authorities_for_run(run)
                .await
            {
                Ok(recovered) => {
                    if recovered > 0 {
                        work_items_requeued += recovered;
                        warn!(
                            run_id = %run.id,
                            recovered = recovered,
                            "Startup recovery settled terminal targeted AdvanceRun authority rows"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to recover terminal targeted AdvanceRun authority rows during startup"
                    );
                }
            }
            match self
                .recover_completed_normal_invokes_blocked_by_stale_targeted_authority_for_run(run)
                .await
            {
                Ok(recovered) => {
                    if recovered > 0 {
                        work_items_requeued += recovered;
                        warn!(
                            run_id = %run.id,
                            recovered = recovered,
                            "Startup recovery completed normal InvokeAgent items blocked by stale targeted retry authority"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to recover normal InvokeAgent items blocked by stale targeted retry authority during startup"
                    );
                }
            }
        }
        let requeued_steward_analyses = work_items::requeue_running_steward_analysis_on_startup(
            &self.pool,
            now,
            "startup_repair_abandoned_steward_analysis",
        )
        .await?;
        if requeued_steward_analyses > 0 {
            work_items_requeued += requeued_steward_analyses as usize;
            warn!(
                requeued = requeued_steward_analyses,
                "Startup recovery requeued abandoned StewardAnalysis work items"
            );
        }
        let completed_invoke_agents =
            work_items::complete_running_invoke_agents_with_terminal_valid_outputs_on_startup(
                &self.pool,
                "startup_repair_completed_agent_valid_outputs",
            )
            .await?;
        if completed_invoke_agents > 0 {
            work_items_requeued += completed_invoke_agents as usize;
            warn!(
                completed = completed_invoke_agents,
                "Startup recovery completed InvokeAgent work items whose agent executions already had valid outputs"
            );
        }
        let requeued_invoke_agents = work_items::requeue_running_invoke_agent_on_startup(
            &self.pool,
            now,
            "startup_repair_abandoned_invoke_agent",
        )
        .await?;
        if requeued_invoke_agents > 0 {
            work_items_requeued += requeued_invoke_agents as usize;
            warn!(
                requeued = requeued_invoke_agents,
                "Startup recovery requeued abandoned InvokeAgent work items"
            );
        }
        if p092_recovered > 0 {
            self.work_queue.refresh_scheduler_projection().await?;
        }

        for run in &active_runs {
            let mut repaired_run = false;
            match self.recover_p088_completion_receipt_artifacts(run).await {
                Ok(recovered_receipts) => {
                    if recovered_receipts > 0 {
                        repaired_run = true;
                        info!(
                            run_id = %run.id,
                            recovered_receipts = recovered_receipts,
                            "Startup recovery reconciled P088 completion receipt artifacts"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to recover P088 completion receipt artifacts during startup"
                    );
                }
            }
            match self.recover_p090_output_settlement_rows(run).await {
                Ok(recovered_rows) => {
                    if recovered_rows > 0 {
                        repaired_run = true;
                        info!(
                            run_id = %run.id,
                            recovered_rows = recovered_rows,
                            "Startup recovery reconciled P090 output settlement rows"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to recover P090 output settlement rows during startup"
                    );
                }
            }
            match artifact_contracts::repair_contract_status_normalization_and_rebuild(
                &self.pool, run.id,
            )
            .await
            {
                Ok(repaired_contracts) => {
                    if repaired_contracts > 0 {
                        repaired_run = true;
                        info!(
                            run_id = %run.id,
                            repaired_contracts = repaired_contracts,
                            "Startup recovery repaired legacy artifact contract status normalization"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to repair artifact contract normalization during startup"
                    );
                }
            }
            match self.repair_p091_orphaned_retry_attempts(run).await {
                Ok(pass) => {
                    if pass.repaired_total > 0 {
                        repaired_run = true;
                    }
                    if pass.would_repair_total > 0 || pass.repaired_total > 0 {
                        info!(
                            run_id = %run.id,
                            mode = %pass.mode,
                            candidates = pass.candidates_total,
                            would_repair = pass.would_repair_total,
                            repaired = pass.repaired_total,
                            "P091 startup orphan retry repair pass complete"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to run P091 orphan retry repair during startup"
                    );
                }
            }
            match self.repair_run(run).await {
                Ok(requeued) => {
                    if requeued > 0 {
                        repaired_run = true;
                        work_items_requeued += requeued;
                    }
                }
                Err(e) => {
                    warn!(run_id = %run.id, error = %e, "Failed to repair run during startup");
                }
            }
            if let Err(e) = rebuild_startup_read_projections(&self.pool, run.id).await {
                warn!(
                    run_id = %run.id,
                    error = %e,
                    "Failed to rebuild startup read projections"
                );
            }
            if repaired_run {
                runs_repaired += 1;
            }
        }

        // P079 REL-r2-2: reclaim expired repair/fallback leases left from a prior
        // daemon instance that crashed or was shut down.
        match self.run_p079_lease_sweep().await {
            Ok(reclaimed) if reclaimed > 0 => {
                warn!(
                    reclaimed = reclaimed,
                    "Startup recovery reclaimed expired P079 repair/fallback leases"
                );
            }
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "P079 lease TTL sweep failed during startup recovery");
            }
        }

        info!(
            runs_inspected = %runs_inspected,
            runs_repaired = %runs_repaired,
            work_items_requeued = %work_items_requeued,
            "Startup recovery complete"
        );

        // P066 T14: Optional toolchain cache startup sweep.
        // Only runs if CHAINWORKS_TOOLCHAIN_HOME is set. On first daemon start
        // after a crash, this reclaims orphan Go session-scoped roots and quarantines
        // stale Xcode run-scoped roots that can't be proven safe.
        let toolchain_cache = if let Ok(home) = std::env::var("CHAINWORKS_TOOLCHAIN_HOME") {
            let sweep_started_at = Utc::now();
            let result =
                sweep_toolchain_cache_roots(&self.pool, Path::new(&home), sweep_started_at).await;
            match result {
                Ok(tc) => tc,
                Err(e) => {
                    warn!(error = %e, "Toolchain cache startup sweep failed");
                    startup_repairs::ToolchainCacheRecoveryReadback::default()
                }
            }
        } else {
            startup_repairs::ToolchainCacheRecoveryReadback::default()
        };

        let has_toolchain_sweep = toolchain_cache.last_sweep_started_at.is_some();
        let readback = if work_items_requeued > 0 || has_toolchain_sweep {
            if work_items_requeued > 0 {
                self.work_queue.refresh_scheduler_projection().await?;
            }
            let mut readback = startup_repairs::build_startup_recovery_readback(
                &self.pool,
                work_items_requeued as i64,
                runs_repaired as i64,
                Utc::now(),
            )
            .await?;
            readback.toolchain_cache = toolchain_cache;
            startup_repairs::record_startup_recovery_readback(&self.pool, &readback).await?;
            Some(readback)
        } else {
            None
        };

        let reported_runs_repaired = readback
            .as_ref()
            .map(|readback| readback.affected_run_count.max(runs_repaired as i64) as usize)
            .unwrap_or(runs_repaired);

        Ok(RecoverySummary {
            runs_inspected,
            runs_repaired: reported_runs_repaired,
            work_items_requeued,
            agent_executions_settled,
            recovered_item_count: readback
                .as_ref()
                .map(|readback| readback.recovered_item_count)
                .unwrap_or(work_items_requeued as i64),
            queued_under_startup_recovery_backpressure_count: readback
                .as_ref()
                .map(|readback| readback.queued_under_startup_recovery_backpressure_count)
                .unwrap_or(0),
            oldest_recovered_queued_age_ms: readback
                .as_ref()
                .and_then(|readback| readback.oldest_recovered_queued_age_ms),
            affected_run_count: readback
                .as_ref()
                .map(|readback| readback.affected_run_count)
                .unwrap_or(runs_repaired as i64),
            next_retry_or_backoff_time: readback
                .as_ref()
                .and_then(|readback| readback.next_retry_or_backoff_time),
        })
    }

    async fn repair_p086_stale_continuation_workers(&self) -> Result<usize> {
        let current_generation = format!("daemon-{}", std::process::id());
        let stale_before = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        let stale = db::repos::agent_work_continuations::list_stale_supervised_workers(
            &self.pool,
            &current_generation,
            &stale_before,
            100,
        )
        .await?;
        let mut repaired = 0usize;
        for worker in stale {
            let close_outcome = if let Some(acp) = &self.acp {
                match acp.close_session(&worker.session_generation_id).await {
                    Ok(()) => P086OrphanReapOutcome {
                        attempted: true,
                        verified: true,
                        error: None,
                        policy: "close_registered_acp_session_before_release",
                        signals_sent: Vec::new(),
                        term_deadline_ms: 0,
                        kill_deadline_ms: 0,
                        started_at: Utc::now().to_rfc3339(),
                        completed_at: Utc::now().to_rfc3339(),
                    },
                    Err(error) => {
                        let mut outcome = p086_reap_registered_provider_process_group(
                            worker.provider_child_pid,
                            worker.provider_process_group_id,
                            worker.provider_process_uid,
                        )
                        .await;
                        if let Some(reap_error) = outcome.error.take() {
                            outcome.error = Some(format!("{error}; {reap_error}"));
                        } else if !outcome.verified {
                            outcome.error = Some(error.to_string());
                        }
                        outcome
                    }
                }
            } else {
                let mut outcome = p086_reap_registered_provider_process_group(
                    worker.provider_child_pid,
                    worker.provider_process_group_id,
                    worker.provider_process_uid,
                )
                .await;
                if outcome.error.is_none() && !outcome.verified {
                    outcome.error = Some("acp_runtime_unavailable".to_string());
                }
                outcome
            };
            let evidence = serde_json::json!({
                "schema_version": "p086_stale_worker_recovery_v1",
                "continuation_id": worker.continuation_id,
                "old_worker_pid": worker.worker_pid,
                "provider_child_pid": worker.provider_child_pid,
                "provider_process_group_id": worker.provider_process_group_id,
                "provider_process_uid": worker.provider_process_uid,
                "old_daemon_generation_id": worker.daemon_generation_id,
                "current_daemon_generation_id": current_generation,
                "session_generation_id": worker.session_generation_id,
                "provider_session_id": worker.provider_session_id,
                "prior_status": worker.status,
                "last_heartbeat_at": worker.last_heartbeat_at,
                "observed_at": Utc::now().to_rfc3339(),
                "orphan_reap_policy": close_outcome.policy,
                "orphan_reap_attempted": close_outcome.attempted,
                "orphan_reap_verified": close_outcome.verified,
                "orphan_reap_error": close_outcome.error,
                "orphan_reap_signals_sent": close_outcome.signals_sent,
                "orphan_reap_term_deadline_ms": close_outcome.term_deadline_ms,
                "orphan_reap_kill_deadline_ms": close_outcome.kill_deadline_ms,
                "orphan_reap_started_at": close_outcome.started_at,
                "orphan_reap_completed_at": close_outcome.completed_at,
                "outcome": if close_outcome.verified {
                    "verified_reap_then_released"
                } else {
                    "fail_closed_without_verified_reap"
                }
            });
            db::repos::agent_work_continuations::release_supervised_worker(
                &self.pool,
                &worker.continuation_id,
                if close_outcome.verified {
                    "stale_generation_reaped"
                } else {
                    "stale_generation_reap_unverified"
                },
                Some(&evidence.to_string()),
            )
            .await?;
            db::repos::agent_work_continuations::update_continuation_status(
                &self.pool,
                &worker.continuation_id,
                "needs_continuation_reconciliation",
                if close_outcome.verified {
                    Some("stale_worker_reaped")
                } else {
                    Some("stale_worker_reap_unverified")
                },
            )
            .await?;
            db::repos::agent_work_continuations::record_p086_continuation_metric_event(
                &self.pool,
                None,
                None,
                None,
                Some(&worker.continuation_id),
                "continuation_orphan_reap_attempted_total",
                serde_json::json!({
                    "orphan_reap_policy": close_outcome.policy,
                    "outcome": if close_outcome.verified { "verified" } else { "unverified" }
                }),
                if close_outcome.attempted { 1 } else { 0 },
            )
            .await?;
            if close_outcome.verified {
                db::repos::agent_work_continuations::record_p086_continuation_metric_event(
                    &self.pool,
                    None,
                    None,
                    None,
                    Some(&worker.continuation_id),
                    "continuation_orphan_reap_verified_total",
                    serde_json::json!({
                        "orphan_reap_policy": close_outcome.policy,
                        "outcome": "verified"
                    }),
                    1,
                )
                .await?;
            }
            repaired += 1;
        }
        Ok(repaired)
    }

    pub async fn run_p092_retry_payload_recovery_for_active_runs(&self) -> Result<usize> {
        let mut repaired = 0usize;
        let mut remaining = p092_recovery_batch_limit();
        for run in runs::list_active(&self.pool).await? {
            if remaining == 0 {
                break;
            }
            let summary = self
                .recover_p092_retry_payload_candidates_for_run(&run, remaining)
                .await?;
            repaired += summary.repaired;
            remaining = remaining.saturating_sub(summary.candidates);
        }
        if repaired > 0 {
            self.work_queue.refresh_scheduler_projection().await?;
        }
        Ok(repaired)
    }

    pub async fn repair_stale_invoke_agent_startups(
        &self,
        now: DateTime<Utc>,
        stale_after: ChronoDuration,
    ) -> Result<u64> {
        let stale_cutoff = now - stale_after;
        let xcode_stale_cutoff = now - ChronoDuration::minutes(12);
        let mut requeued = work_items::requeue_stale_starting_invoke_agent_sessions(
            &self.pool,
            now,
            stale_cutoff,
            xcode_stale_cutoff,
            "startup_repair_stale_acp_startup",
        )
        .await?;
        requeued += work_items::requeue_stale_pre_session_invoke_agents(
            &self.pool,
            now,
            stale_cutoff,
            xcode_stale_cutoff,
            "startup_repair_stale_acp_pre_session_startup",
        )
        .await?;
        if requeued > 0 {
            self.work_queue.refresh_scheduler_projection().await?;
            warn!(
                requeued = requeued,
                stale_cutoff = %stale_cutoff,
                "Startup recovery requeued stale ACP startup InvokeAgent work items"
            );
        }
        Ok(requeued)
    }

    async fn recover_p088_completion_receipt_artifacts(&self, run: &Run) -> Result<usize> {
        let p088_root = Path::new(&run.artifact_root).join("evidence").join("p088");
        if !p088_root.exists() {
            return Ok(0);
        }

        let mut recovered = 0usize;
        for entry in std::fs::read_dir(&p088_root)? {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path().join("code-writer-completion-receipt-v1.json");
            if !path.exists() {
                continue;
            }
            let raw = match std::fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to read P088 receipt artifact during startup recovery");
                    continue;
                }
            };
            let artifact: P088CompletionReceiptArtifact = match serde_json::from_str(&raw) {
                Ok(artifact) => artifact,
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to parse P088 receipt artifact during startup recovery");
                    continue;
                }
            };
            if artifact.schema_version != "code_writer_completion_receipt_v1" {
                continue;
            }
            if artifact.receipt.run_id != run.id {
                warn!(
                    run_id = %run.id,
                    artifact_run_id = %artifact.receipt.run_id,
                    path = %path.display(),
                    "Skipping P088 receipt artifact with mismatched run id"
                );
                continue;
            }
            if code_writer_completion_receipts::find_by_execution_id(
                &self.pool,
                artifact.receipt.agent_execution_id,
            )
            .await?
            .is_some()
            {
                continue;
            }
            if agent_executions::find_by_id(&self.pool, artifact.receipt.agent_execution_id)
                .await?
                .is_none()
            {
                warn!(
                    run_id = %run.id,
                    agent_execution_id = %artifact.receipt.agent_execution_id,
                    path = %path.display(),
                    "Skipping orphan P088 receipt artifact without matching agent execution"
                );
                continue;
            }

            let mut receipt = artifact.receipt;
            receipt.receipt_artifact_path = Some(path.to_string_lossy().into_owned());
            receipt.failure_class = Some("completion_receipt_partial_write".to_string());
            receipt.transcript_absence_reason = Some("storage_write_failed".to_string());
            if let Err(e) = code_writer_completion_receipts::upsert(
                &self.pool,
                &receipt,
                &artifact.text_captures,
                &artifact.output_decisions,
            )
            .await
            {
                warn!(
                    run_id = %run.id,
                    agent_execution_id = %receipt.agent_execution_id,
                    path = %path.display(),
                    error = %e,
                    "Failed to persist recovered P088 receipt artifact"
                );
                continue;
            }
            recovered += 1;
        }
        Ok(recovered)
    }

    async fn recover_p092_retry_payload_candidates_for_run(
        &self,
        run: &Run,
        batch_limit: usize,
    ) -> Result<P092RecoverySummary> {
        let mode = P092RecoveryMode::from_env();
        let disabled = p092_recovery_disabled();
        let rows = sqlx::query(
            r#"
            SELECT wi.id AS invoke_work_item_id,
                   wi.payload_json,
                   wi.stage_id AS work_stage_id,
                   auth.id AS retry_authority_id,
                   auth.stage_id AS authority_stage_id,
                   auth.target_stage_execution_id,
                   ae.id AS completed_agent_execution_id,
                   json_extract(wi.payload_json, '$.target_stage_execution_id') AS payload_target_stage_execution_id,
                   json_extract(wi.payload_json, '$.source_stage_execution_id') AS top_source_stage_execution_id,
                   json_extract(wi.payload_json, '$.source_agent_execution_id') AS top_source_agent_execution_id,
                   json_extract(wi.payload_json, '$.source_work_item_id') AS top_source_work_item_id,
                   json_extract(wi.payload_json, '$.targeted_retry.source_stage_execution_id') AS nested_source_stage_execution_id,
                   json_extract(wi.payload_json, '$.targeted_retry.source_agent_execution_id') AS nested_source_agent_execution_id,
                   json_extract(wi.payload_json, '$.targeted_retry.source_work_item_id') AS nested_source_work_item_id
            FROM work_items wi
            JOIN retry_stage_execution_authorities auth
              ON auth.run_id = wi.run_id
             AND auth.authority_state = 'active'
             AND (
                 auth.id = json_extract(wi.payload_json, '$.retry_authority_id')
              OR auth.id = json_extract(wi.payload_json, '$.targeted_retry.retry_authority_id')
              OR auth.source_invoke_work_item_id = wi.id
              OR auth.target_stage_execution_id = json_extract(wi.payload_json, '$.stage_execution_id')
              OR auth.target_stage_execution_id = json_extract(wi.payload_json, '$.target_stage_execution_id')
             )
            JOIN stage_executions se
              ON se.id = auth.target_stage_execution_id
             AND se.status = 'running'
            JOIN agent_executions ae
              ON ae.id = json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id')
             AND ae.stage_execution_id = auth.target_stage_execution_id
             AND ae.status = 'completed'
            JOIN agent_execution_runtime_facts facts
              ON facts.agent_execution_id = ae.id
             AND facts.output_settlement IN ('valid_outputs_from_completed_execution', 'valid_outputs_from_failed_execution')
             AND COALESCE(facts.valid_required_outputs, 0) > 0
            WHERE wi.run_id = ?1
              AND wi.kind = 'invoke_agent'
              AND wi.status = 'running'
            ORDER BY wi.scheduled_at ASC, wi.rowid ASC
            LIMIT ?2
            "#,
        )
        .bind(run.id.to_string())
        .bind(batch_limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("select P092 retry payload recovery candidates")?;

        let candidate_count = rows.len();
        let mut summary = P092RecoverySummary {
            candidates: candidate_count,
            excluded: if disabled { candidate_count } else { 0 },
            repaired: 0,
            mode: mode.as_str(),
            disabled,
        };
        for row in rows {
            let invoke_work_item_id: String = row.get("invoke_work_item_id");
            let retry_authority_id: String = row.get("retry_authority_id");
            let target_stage_execution_id_raw: String = row.get("target_stage_execution_id");
            let completed_agent_execution_id: String = row.get("completed_agent_execution_id");
            let target_stage_execution_id = target_stage_execution_id_raw
                .parse()
                .context("parse P092 target stage execution id")?;
            let payload_json: String = row.get("payload_json");
            let payload = serde_json::from_str::<serde_json::Value>(&payload_json)
                .unwrap_or_else(|_| serde_json::json!({}));
            let payload_target: Option<String> = row.get("payload_target_stage_execution_id");
            let top_source_stage: Option<String> = row.get("top_source_stage_execution_id");
            let top_source_agent: Option<String> = row.get("top_source_agent_execution_id");
            let top_source_work_item: Option<String> = row.get("top_source_work_item_id");
            let nested_source_stage: Option<String> = row.get("nested_source_stage_execution_id");
            let nested_source_agent: Option<String> = row.get("nested_source_agent_execution_id");
            let nested_source_work_item: Option<String> = row.get("nested_source_work_item_id");
            let stale_fields = p092_stale_payload_fields(
                payload_target.as_deref(),
                &target_stage_execution_id_raw,
                top_source_stage.as_deref(),
                top_source_agent.as_deref(),
                top_source_work_item.as_deref(),
            );
            let provenance = serde_json::json!({
                "source_stage_execution_id": nested_source_stage.or(top_source_stage),
                "source_agent_execution_id": nested_source_agent.or(top_source_agent),
                "source_work_item_id": nested_source_work_item.or(top_source_work_item),
            });
            let now = Utc::now();
            let idempotency_key = format!(
                "p092:{}:{}:{}:{}",
                run.id, invoke_work_item_id, retry_authority_id, completed_agent_execution_id
            );
            let event = RetryPayloadRecoveryEvent {
                idempotency_key,
                run_id: run.id,
                invoke_work_item_id: invoke_work_item_id.clone(),
                retry_authority_id: Some(retry_authority_id.clone()),
                target_stage_execution_id: Some(target_stage_execution_id),
                completed_agent_execution_id: Some(completed_agent_execution_id.clone()),
                reason_code: p092_recovery_reason_code(&stale_fields).to_string(),
                mode: mode.as_str().to_string(),
                repaired: false,
                current_json: serde_json::json!({
                    "run_id": run.id.to_string(),
                    "target_stage_execution_id": target_stage_execution_id_raw,
                    "retry_authority_id": retry_authority_id,
                    "completed_agent_execution_id": completed_agent_execution_id,
                    "invoke_work_item_id": invoke_work_item_id,
                }),
                provenance_json: Some(provenance),
                repaired_fields_json: Some(serde_json::Value::Array(
                    stale_fields
                        .iter()
                        .map(|field| serde_json::Value::String((*field).to_string()))
                        .collect(),
                )),
                diagnostic_json: Some(serde_json::json!({
                    "would_repair": !disabled,
                    "disabled": if disabled { 1 } else { 0 },
                    "candidates_total": candidate_count,
                    "excluded_total": if disabled { candidate_count } else { 0 },
                    "repaired_total": 0,
                    "payload_was_object": payload.is_object(),
                })),
                created_at: now,
                updated_at: now,
            };
            retry_payload_recovery_events::upsert(&self.pool, &event).await?;
            if matches!(mode, P092RecoveryMode::Enforce) && !disabled {
                self.repair_p092_invoke_payload_and_complete(
                    &event.invoke_work_item_id,
                    &event.current_json,
                    event.provenance_json.as_ref(),
                )
                .await?;
                let mut repaired_event = event;
                repaired_event.repaired = true;
                repaired_event.updated_at = Utc::now();
                retry_payload_recovery_events::upsert(&self.pool, &repaired_event).await?;
                summary.repaired += 1;
            }
        }
        Ok(summary)
    }

    async fn repair_p092_invoke_payload_and_complete(
        &self,
        invoke_work_item_id: &str,
        current: &serde_json::Value,
        provenance: Option<&serde_json::Value>,
    ) -> Result<()> {
        let mut tx = self
            .begin_transaction(
                "recovery.P092RetryPayloadRepair",
                format!("recovery.P092RetryPayloadRepair:{invoke_work_item_id}"),
            )
            .await?;
        let row =
            sqlx::query("SELECT payload_json FROM work_items WHERE id = ?1 AND status = 'running'")
                .bind(invoke_work_item_id)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| {
                    anyhow!("P092 candidate {invoke_work_item_id} is no longer running")
                })?;
        let payload_json: String = row.get("payload_json");
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        let object = payload
            .as_object_mut()
            .ok_or_else(|| anyhow!("P092 candidate {invoke_work_item_id} payload is not object"))?;
        object.remove("source_stage_execution_id");
        object.remove("source_agent_execution_id");
        object.remove("source_work_item_id");
        object.insert("run_id".to_string(), current["run_id"].clone());
        object.insert(
            "stage_execution_id".to_string(),
            current["target_stage_execution_id"].clone(),
        );
        object.insert(
            "target_stage_execution_id".to_string(),
            current["target_stage_execution_id"].clone(),
        );
        object.insert(
            "retry_authority_id".to_string(),
            current["retry_authority_id"].clone(),
        );
        let mut targeted_retry = object
            .remove("targeted_retry")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        targeted_retry.insert(
            "retry_authority_id".to_string(),
            current["retry_authority_id"].clone(),
        );
        targeted_retry.insert(
            "target_stage_execution_id".to_string(),
            current["target_stage_execution_id"].clone(),
        );
        if let Some(provenance) = provenance {
            for key in [
                "source_stage_execution_id",
                "source_agent_execution_id",
                "source_work_item_id",
            ] {
                if !provenance[key].is_null() {
                    targeted_retry.insert(key.to_string(), provenance[key].clone());
                }
            }
        }
        object.insert(
            "targeted_retry".to_string(),
            serde_json::Value::Object(targeted_retry),
        );
        sqlx::query("UPDATE work_items SET payload_json = ?1 WHERE id = ?2 AND status = 'running'")
            .bind(serde_json::to_string(&payload)?)
            .bind(invoke_work_item_id)
            .execute(&mut **tx)
            .await?;
        tx.commit().await?;
        self.work_queue.complete(invoke_work_item_id).await?;
        Ok(())
    }

    async fn repair_auto_contract_retry_authority_gaps_for_run(&self, run: &Run) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
                SELECT id,
                       payload_json,
                       stage_id AS work_stage_id,
                       json_extract(payload_json, '$.stage_execution_id') AS target_stage_execution_id,
                       json_extract(payload_json, '$.p058_claimed.agent_execution_id') AS claimed_agent_execution_id,
                       json_extract(payload_json, '$.targeted_retry.source_agent_execution_id') AS source_agent_execution_id
                FROM work_items
                WHERE run_id = ?1
                  AND kind = 'invoke_agent'
                  AND status = 'running'
                  AND id LIKE 'auto-contract-output-retry:%'
                  AND json_extract(payload_json, '$.targeted_retry.reason') = 'auto_contract_output_retry'
                  AND NOT EXISTS (
                      SELECT 1
                      FROM retry_payload_recovery_events p092
                      WHERE p092.invoke_work_item_id = work_items.id
                        AND p092.mode = 'diagnostic'
                        AND p092.repaired = 0
                        AND COALESCE(json_extract(p092.diagnostic_json, '$.disabled'), 0) = 0
                  )
            )
            SELECT c.id,
                   c.payload_json,
                   c.work_stage_id,
                   c.target_stage_execution_id,
                   c.claimed_agent_execution_id,
                   c.source_agent_execution_id,
                   ae.status AS agent_status,
                   facts.output_settlement,
                   facts.valid_required_outputs
            FROM candidates c
            LEFT JOIN retry_stage_execution_authorities rsa
                   ON rsa.run_id = ?1
                  AND rsa.target_stage_execution_id = c.target_stage_execution_id
                  AND rsa.authority_state = 'active'
            LEFT JOIN agent_executions ae
                   ON ae.id = c.claimed_agent_execution_id
            LEFT JOIN agent_execution_runtime_facts facts
                   ON facts.agent_execution_id = c.claimed_agent_execution_id
            WHERE c.target_stage_execution_id IS NOT NULL
              AND rsa.id IS NULL
            ORDER BY c.id
            "#,
        )
        .bind(run.id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut repaired = 0usize;
        for row in rows {
            let item_id: String = row.get("id");
            let payload_json: String = row.get("payload_json");
            let Some(stage_id) = row.get::<Option<String>, _>("work_stage_id") else {
                continue;
            };
            let Some(target_raw) = row.get::<Option<String>, _>("target_stage_execution_id") else {
                continue;
            };
            let Ok(target_stage_execution_id) = target_raw.parse() else {
                continue;
            };
            let Some(claimed_agent_execution_id) =
                row.get::<Option<String>, _>("claimed_agent_execution_id")
            else {
                continue;
            };
            let Some(agent_status) = row.get::<Option<String>, _>("agent_status") else {
                continue;
            };
            let output_settlement = row.get::<Option<String>, _>("output_settlement");
            let valid_required_outputs = row
                .get::<Option<i64>, _>("valid_required_outputs")
                .unwrap_or_default();
            let is_valid_completed = agent_status == AgentStatus::Completed.to_string()
                && matches!(
                    output_settlement.as_deref(),
                    Some(
                        "valid_outputs_from_completed_execution"
                            | "valid_outputs_from_failed_execution"
                    )
                )
                && valid_required_outputs > 0;
            let is_terminal_failed = matches!(agent_status.as_str(), "failed" | "cancelled");
            if !is_valid_completed && !is_terminal_failed {
                continue;
            }

            let source_agent_execution_id = row
                .get::<Option<String>, _>("source_agent_execution_id")
                .or(Some(claimed_agent_execution_id));
            let now = Utc::now();
            let authority_id = {
                let mut tx = self
                    .begin_transaction(
                        "recovery.auto_contract_retry_authority_gap",
                        format!("recovery.auto_contract_retry_authority_gap:{item_id}"),
                    )
                    .await?;
                let authority =
                    retry_stage_execution_authorities::create_active_targeted_agent_retry_tx(
                        &mut tx,
                        run.id,
                        &stage_id,
                        target_stage_execution_id,
                        None,
                        None,
                        item_id.clone(),
                        source_agent_execution_id,
                        now,
                    )
                    .await?;
                let updated_payload =
                    inject_retry_authority_id_into_payload(&payload_json, &authority.id)?;
                sqlx::query(
                    r#"UPDATE work_items
                       SET payload_json = ?1
                       WHERE id = ?2 AND status = 'running'"#,
                )
                .bind(updated_payload)
                .bind(&item_id)
                .execute(&mut **tx)
                .await?;
                tx.commit().await?;
                authority.id
            };

            if is_valid_completed {
                work_items::complete(&self.pool, &item_id).await?;
            } else {
                work_items::fail(
                    &self.pool,
                    &item_id,
                    "startup_recovery_auto_contract_retry_terminal_agent_without_valid_outputs",
                )
                .await?;
            }
            info!(
                run_id = %run.id,
                item_id = %item_id,
                retry_authority_id = %authority_id,
                completed = is_valid_completed,
                "Startup recovery repaired legacy auto-contract targeted retry without authority"
            );
            repaired += 1;
        }

        if repaired > 0 {
            projections::rebuild_all_for_run(&self.pool, run.id).await?;
        }
        Ok(repaired)
    }

    async fn recover_completed_normal_invokes_blocked_by_stale_targeted_authority_for_run(
        &self,
        run: &Run,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            SELECT wi.id
            FROM work_items wi
            JOIN retry_stage_execution_authorities rsa
              ON rsa.run_id = ?1
             AND rsa.target_stage_execution_id = json_extract(wi.payload_json, '$.stage_execution_id')
             AND rsa.authority_state = 'active'
             AND rsa.source_invoke_work_item_id IS NOT NULL
             AND rsa.source_invoke_work_item_id != wi.id
            JOIN agent_executions ae
              ON ae.id = json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id')
             AND ae.status = 'completed'
            JOIN agent_execution_runtime_facts facts
              ON facts.agent_execution_id = ae.id
             AND facts.output_settlement IN (
                 'valid_outputs_from_completed_execution',
                 'valid_outputs_from_failed_execution'
             )
            WHERE wi.run_id = ?1
              AND wi.kind = 'invoke_agent'
              AND wi.status = 'running'
              AND wi.id NOT LIKE 'auto-contract-output-retry:%'
              AND json_type(wi.payload_json, '$.retry_authority_id') IS NULL
              AND json_type(wi.payload_json, '$.targeted_retry') IS NULL
              AND json_extract(wi.payload_json, '$.stage_execution_id') IS NOT NULL
              AND json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id') IS NOT NULL
            ORDER BY wi.id
            "#,
        )
        .bind(run.id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut recovered = 0usize;
        for row in rows {
            let item_id: String = row.get("id");
            work_items::complete(&self.pool, &item_id).await?;
            info!(
                run_id = %run.id,
                item_id = %item_id,
                "Startup recovery completed normal InvokeAgent with terminal valid agent and stale targeted retry authority"
            );
            recovered += 1;
        }

        if recovered > 0 {
            projections::rebuild_all_for_run(&self.pool, run.id).await?;
        }
        Ok(recovered)
    }

    async fn recover_p058_shutdown_drain_force_detach_for_run(&self, run: &Run) -> Result<usize> {
        let rows = sqlx::query(
            r#"SELECT ae.id AS agent_execution_id,
                      ae.stage_execution_id AS stage_execution_id,
                      ae.agent_id AS agent_id,
                      COALESCE(em.escalation_ledger_id, ae.escalation_ledger_id) AS escalation_ledger_id,
                      COALESCE(em.tier_id, ae.escalation_tier_id) AS tier_id,
                      COALESCE(em.tier_kind_raw, ae.escalation_tier_kind_raw) AS tier_kind_raw,
                      COALESCE(em.trigger_raw, ae.escalation_trigger_raw) AS trigger_raw
               FROM agent_executions ae
               INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
               LEFT JOIN escalation_execution_metadata em ON em.agent_execution_id = ae.id
               WHERE se.run_id = ?
                 AND se.status = 'running'
                 AND ae.status = 'running'
                 AND ae.escalation_ledger_id IS NOT NULL
               ORDER BY ae.started_at ASC, ae.id ASC"#,
        )
        .bind(run.id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut recovered = 0usize;
        for row in rows {
            let agent_execution_id_raw: String = row.try_get("agent_execution_id")?;
            let stage_execution_id_raw: String = row.try_get("stage_execution_id")?;
            let escalation_ledger_id: String = row.try_get("escalation_ledger_id")?;
            let tier_id: Option<String> = row.try_get("tier_id")?;
            let tier_kind_raw: Option<String> = row.try_get("tier_kind_raw")?;
            let trigger_raw: Option<String> = row.try_get("trigger_raw")?;
            let agent_execution_id = agent_execution_id_raw
                .parse()
                .map_err(|e| anyhow!("bad agent_execution_id: {e}"))?;
            let stage_execution_id = stage_execution_id_raw
                .parse()
                .map_err(|e| anyhow!("bad stage_execution_id: {e}"))?;

            let Some(mut ledger) =
                escalation::find_ledger_by_id(&self.pool, &escalation_ledger_id).await?
            else {
                continue;
            };
            if ledger.status_raw != "active" {
                continue;
            }

            let event_id = format!(
                "p058-force-detach-replay:{}:{}",
                escalation_ledger_id, agent_execution_id_raw
            );
            let already_recorded =
                escalation::find_events_by_ledger(&self.pool, &escalation_ledger_id)
                    .await?
                    .into_iter()
                    .any(|event| event.id == event_id);
            if already_recorded {
                continue;
            }

            let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
            facts.failure_kind = Some(AgentFailureKind::TransportClosed);
            facts.failure_kind_raw_debug = Some("shutdown_drain_timeout".to_string());
            facts.operator_action_hint = Some(OperatorActionHint::InspectLogs);
            facts.transport_error_code = Some("shutdown_drain_timeout".to_string());
            facts.supervision_classification = Some("shutdown_drain_timeout".to_string());
            facts.output_settlement = AgentOutputSettlement::None;

            ledger.status_raw = "paused".to_string();
            ledger.pause_reason_raw = Some("provider_session_force_detached".to_string());
            ledger.operator_action_hint = Some(
                "Provider session was still running at daemon startup after shutdown drain; inspect provider state before manual resume."
                    .to_string(),
            );
            ledger.runbook_anchor = Some("escalation/provider-session-force-detached".to_string());
            ledger.updated_at = now;

            let tx_started = std::time::Instant::now();
            let mut tx = self
                .begin_transaction(
                    "recovery.p058_force_detach_replay",
                    format!("recovery.p058_force_detach_replay:{agent_execution_id_raw}"),
                )
                .await?;
            agent_executions::update_completed_tx(
                &mut tx,
                agent_execution_id,
                AgentStatus::Failed,
                now,
            )
            .await?;
            agent_execution_runtime_facts::upsert_tx(&mut tx, &facts).await?;
            escalation::update_ledger_tx(&mut tx, &ledger).await?;
            escalation::insert_event_tx(
                &mut tx,
                &EscalationEvent {
                    id: event_id,
                    escalation_ledger_id: escalation_ledger_id.clone(),
                    event_kind_raw: "escalation.provider_session_force_detached".to_string(),
                    tier_id: tier_id.clone(),
                    tier_kind_raw: tier_kind_raw.clone(),
                    trigger_raw: trigger_raw.clone(),
                    pause_reason_raw: Some("provider_session_force_detached".to_string()),
                    payload_json: Some(
                        serde_json::json!({
                            "event_kind_raw": "escalation.provider_session_force_detached",
                            "pause_reason_raw": "provider_session_force_detached",
                            "digest_inputs": {
                                "failure_kind": "transport_closed",
                                "output_settlement_state": "none",
                                "validation_evidence_kind": "shutdown_drain_timeout",
                                "redacted_message_fragment_hash": format!("sha256:{:x}", Sha256::digest(stage_execution_id_raw.as_bytes())),
                            },
                            "redacted_evidence_ref": format!("sha256:{:x}", Sha256::digest(agent_execution_id_raw.as_bytes())),
                        })
                        .to_string(),
                    ),
                    redaction_version: Some("redaction_v1".to_string()),
                    created_at: now,
                },
            )
            .await?;
            work_items::cancel_pending_or_running_invoke_by_stage_execution_tx(
                &mut tx,
                run.id,
                &stage_execution_id_raw,
                now,
                "p058_startup_force_detach_replay",
            )
            .await?;
            stages::settle_tx(
                &mut tx,
                stage_execution_id,
                StageSettlementKind::Failed,
                now,
            )
            .await?;
            runs::update_status_tx(&mut tx, run.id, domain::run::RunStatus::Blocked).await?;
            tx.commit().await?;
            db::pool::log_write_transaction("recovery.p058_force_detach_replay", tx_started);
            recovered += 1;
        }

        Ok(recovered)
    }

    async fn recover_failed_terminal_targeted_advance_authorities_for_run(
        &self,
        run: &Run,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            SELECT wi.id,
                   json_extract(wi.payload_json, '$.retry_authority_id') AS retry_authority_id,
                   json_extract(wi.payload_json, '$.target_stage_execution_id') AS target_stage_execution_id,
                   se.status AS target_stage_status
            FROM work_items wi
            JOIN retry_stage_execution_authorities rsa
              ON rsa.id = json_extract(wi.payload_json, '$.retry_authority_id')
             AND rsa.run_id = ?1
             AND rsa.authority_state = 'active'
            JOIN stage_executions se
              ON se.id = json_extract(wi.payload_json, '$.target_stage_execution_id')
             AND se.run_id = ?1
             AND se.id = rsa.target_stage_execution_id
            WHERE wi.run_id = ?1
              AND wi.kind = 'advance_run'
              AND wi.status = 'failed'
              AND wi.id LIKE 'advance-after-invoke:%'
              AND wi.last_error LIKE 'advance_run_target_unexpected_terminal:%'
              AND se.status IN ('completed', 'failed', 'blocked', 'skipped')
            ORDER BY wi.id
            "#,
        )
        .bind(run.id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut recovered = 0usize;
        for row in rows {
            let item_id: String = row.get("id");
            let Some(retry_authority_id) = row.get::<Option<String>, _>("retry_authority_id")
            else {
                continue;
            };
            let Some(target_stage_execution_id) =
                row.get::<Option<String>, _>("target_stage_execution_id")
            else {
                continue;
            };
            let Some(target_stage_status) = row.get::<Option<String>, _>("target_stage_status")
            else {
                continue;
            };
            let now = Utc::now();
            let mut tx = self
                .begin_transaction(
                    "recovery.terminal_targeted_advance_authority",
                    format!("recovery.terminal_targeted_advance_authority:{item_id}"),
                )
                .await?;
            retry_stage_execution_authorities::mark_terminalized_tx(
                &mut tx,
                &retry_authority_id,
                now,
                "target_stage_terminal",
            )
            .await?;
            sqlx::query(
                r#"UPDATE work_items
                   SET status = 'completed',
                       completed_at = COALESCE(completed_at, ?1),
                       last_error = NULL
                   WHERE id = ?2
                     AND status = 'failed'
                     AND last_error LIKE 'advance_run_target_unexpected_terminal:%'"#,
            )
            .bind(now.to_rfc3339())
            .bind(&item_id)
            .execute(&mut **tx)
            .await?;
            tx.commit().await?;
            info!(
                run_id = %run.id,
                item_id = %item_id,
                retry_authority_id = %retry_authority_id,
                target_stage_execution_id = %target_stage_execution_id,
                target_stage_status = %target_stage_status,
                "Startup recovery converted terminal-targeted AdvanceRun failure into settled authority"
            );
            recovered += 1;
        }

        if recovered > 0 {
            projections::rebuild_all_for_run(&self.pool, run.id).await?;
        }
        Ok(recovered)
    }

    async fn recover_p090_output_settlement_rows(&self, run: &Run) -> Result<usize> {
        let rows = code_writer_completion_receipts::list_p090_recoverable_settlement_rows_by_run(
            &self.pool, run.id,
        )
        .await?;
        let mut recovered = 0usize;
        for row in rows {
            match row.materialization_state.as_str() {
                "staged" => {
                    let canonical_sha = sha256_file_if_exists(&row.canonical_path)?;
                    let staging_sha = row
                        .staging_path
                        .as_deref()
                        .map(sha256_file_if_exists)
                        .transpose()?
                        .flatten();
                    let expected_sha = row
                        .candidate_digest
                        .as_deref()
                        .or(staging_sha.as_deref())
                        .map(str::to_string);
                    if canonical_sha.is_some() && canonical_sha == expected_sha {
                        let mut recovered_row = row.clone();
                        recovered_row.materialization_state = "committed".to_string();
                        recovered_row.canonical_after_sha256 = canonical_sha.clone();
                        recovered_row.committed_at = Some(Utc::now());
                        code_writer_completion_receipts::update_p090_settlement_row_recovery_state(
                            &self.pool,
                            &row.id,
                            "committed",
                            canonical_sha.as_deref(),
                            recovered_row.committed_at,
                            None,
                        )
                        .await?;
                        self.publish_p090_recovered_active_artifact_generation(&recovered_row)
                            .await?;
                        recovered += 1;
                    } else if canonical_sha == row.canonical_before_sha256 {
                        code_writer_completion_receipts::update_p090_settlement_row_recovery_state(
                            &self.pool,
                            &row.id,
                            "failed",
                            canonical_sha.as_deref(),
                            None,
                            Some("startup_recovery_left_staged_output_unpromoted"),
                        )
                        .await?;
                        recovered += 1;
                    } else {
                        code_writer_completion_receipts::update_p090_settlement_row_recovery_state(
                            &self.pool,
                            &row.id,
                            "failed",
                            canonical_sha.as_deref(),
                            None,
                            Some("startup_recovery_detected_unverifiable_canonical_change"),
                        )
                        .await?;
                        recovered += 1;
                    }
                }
                "committed" if row.canonical_after_sha256.is_none() => {
                    let canonical_sha = sha256_file_if_exists(&row.canonical_path)?;
                    let mut recovered_row = row.clone();
                    recovered_row.canonical_after_sha256 = canonical_sha.clone();
                    recovered_row.committed_at = row.committed_at.or(Some(Utc::now()));
                    code_writer_completion_receipts::update_p090_settlement_row_recovery_state(
                        &self.pool,
                        &row.id,
                        "committed",
                        canonical_sha.as_deref(),
                        recovered_row.committed_at,
                        None,
                    )
                    .await?;
                    self.publish_p090_recovered_active_artifact_generation(&recovered_row)
                        .await?;
                    recovered += 1;
                }
                _ => {}
            }
        }
        Ok(recovered)
    }

    async fn publish_p090_recovered_active_artifact_generation(
        &self,
        row: &domain::code_writer_completion::CodeWriterOutputSettlementRow,
    ) -> Result<()> {
        if row.decision != "accepted" || row.materialization_state != "committed" {
            return Ok(());
        }
        if row.canonical_after_sha256.is_none()
            || !domain::artifact_contracts::known_contract_id(&row.contract_id)
        {
            return Ok(());
        }
        let raw_status = extract_p090_recovered_contract_status(&row.canonical_path)
            .unwrap_or_else(|| "unknown".to_string());
        artifact_contracts::upsert_generation_and_rebuild(
            &self.pool,
            domain::artifact_contracts::ActiveArtifactGenerationInput {
                run_id: row.run_id,
                artifact_id: domain::ids::ArtifactId::new(),
                contract_id: row.contract_id.clone(),
                canonical_path: row.output_name.clone(),
                raw_path: row.canonical_path.clone(),
                raw_status,
                generation_id: row
                    .active_pointer_generation_id
                    .clone()
                    .unwrap_or_else(|| row.id.clone()),
                source_agent_execution_id: Some(row.agent_execution_id.to_string()),
                source_stage_execution_id: Some(row.stage_execution_id.to_string()),
                source_session_generation_id: row.session_generation_id.clone(),
                source_work_item_id: None,
                supersedes_generation_id: None,
                output_settlement: AgentOutputSettlement::ValidOutputsFromFailedExecution,
                partial: true,
                warnings: vec![
                    "P090 startup recovery published accepted staged repair settlement row"
                        .to_string(),
                ],
            },
        )
        .await
    }

    async fn repair_run(&self, run: &Run) -> Result<usize> {
        let run_stages = stages::list_by_run(&self.pool, run.id).await?;
        let mut requeued = 0usize;
        let now = Utc::now();

        for stage in run_stages
            .iter()
            .filter(|stage| stage.status != StageStatus::Running)
        {
            let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
            if !executions
                .iter()
                .any(|execution| execution.status == AgentStatus::Running)
            {
                continue;
            }

            let tx_started = std::time::Instant::now();
            let mut tx = self
                .begin_transaction(
                    "recovery.clear_stale_stage_execution",
                    format!("recovery.clear_stale_stage_execution:{}", stage.id),
                )
                .await?;
            let cancelled_executions =
                agent_executions::cancel_running_by_stage_tx(&mut tx, stage.id, now).await?;
            let cancelled_work_items =
                work_items::cancel_pending_or_running_invoke_by_stage_execution_tx(
                    &mut tx,
                    run.id,
                    &stage.id.to_string(),
                    now,
                    "stale_stage_execution_startup_repair",
                )
                .await?;
            tx.commit().await?;
            db::pool::log_write_transaction("recovery.clear_stale_stage_execution", tx_started);

            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                stage_execution_id = %stage.id,
                cancelled_executions = cancelled_executions,
                cancelled_work_items = cancelled_work_items,
                "Startup repair cleared stale running InvokeAgent state for non-running stage"
            );
        }

        // Check for stages stuck in Running state — these might be orphaned from a crash
        let running_stages: Vec<_> = run_stages
            .iter()
            .filter(|s| s.status == StageStatus::Running)
            .collect();

        let mut blocked_running_stages = 0usize;

        for stage in &running_stages {
            let requeued_preclaimed = work_items::requeue_running_preclaimed_invoke_for_stage(
                &self.pool,
                run.id,
                stage.id,
                &stage.stage_id,
            )
            .await?;
            if requeued_preclaimed > 0 {
                info!(
                    run_id = %run.id,
                    stage_id = %stage.stage_id,
                    requeued = %requeued_preclaimed,
                    "Requeued preclaimed P058 InvokeAgent work item during startup repair"
                );
                requeued += requeued_preclaimed;
                continue;
            }
            if stage_has_pending_or_running_invoke_work(&self.pool, run.id, stage.id).await? {
                info!(
                    run_id = %run.id,
                    stage_id = %stage.stage_id,
                    stage_execution_id = %stage.id,
                    "Startup repair left running stage open because recovered InvokeAgent work is queued"
                );
                continue;
            }

            let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
            if executions.is_empty() {
                if !run_has_pending_or_running_work(&self.pool, run.id).await? {
                    info!(
                        run_id = %run.id,
                        stage_id = %stage.stage_id,
                        stage_execution_id = %stage.id,
                        "Startup repair kickstarting empty running stage"
                    );
                    self.work_queue
                        .enqueue(
                            WorkItemKind::AdvanceRun,
                            Some(run.id),
                            None,
                            serde_json::json!({
                                "run_id": run.id.to_string(),
                                "reason": "startup_empty_running_stage_kickstart",
                                "stage_execution_id": stage.id.to_string(),
                                "stage_id": stage.stage_id,
                            }),
                        )
                        .await?;
                    requeued += 1;
                }
                continue;
            }

            let provenance_suffix = self
                .latest_execution_provenance_suffix(stage.id)
                .await
                .unwrap_or_default();
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                "Found stage stuck in Running state during startup repair — re-enqueuing AdvanceRun"
            );
            // Mark as blocked so we can retry safely
            stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
            let drift_details = serde_json::json!({
                "source": "startup_repair",
                "reason": "stage_stuck_running",
                "stage_execution_id": stage.id.to_string(),
                "stage_id": stage.stage_id,
                "action": "marked_blocked_for_operator_retry",
            });
            runs::update_drift_detection(&self.pool, run.id, now, &drift_details.to_string())
                .await?;

            // Audit trail: record the repair action (proposal §6.3)
            let repair_id = uuid::Uuid::new_v4().to_string();
            let _ = startup_repairs::record(
                &self.pool,
                &repair_id,
                &run.id.to_string(),
                "stage_blocked",
                now,
                Some(&format!(
                    "Stage '{}' stuck in Running — marked Blocked{}",
                    stage.stage_id, provenance_suffix
                )),
            )
            .await;

            // Recovery recommendation: the operator should retry or cancel
            let rec_id = uuid::Uuid::new_v4().to_string();
            let _ = startup_repairs::recommend(
                &self.pool,
                &rec_id,
                &run.id.to_string(),
                Some(&stage.stage_id),
                "retry_stage",
                &format!(
                    "Stage was stuck in Running at daemon startup — consider retry or manual review{}",
                    provenance_suffix
                ),
                now,
            )
            .await;
            blocked_running_stages += 1;
        }

        if blocked_running_stages > 0 {
            // Re-enqueue an AdvanceRun for this run to recheck state
            self.work_queue
                .enqueue(
                    WorkItemKind::AdvanceRun,
                    Some(run.id),
                    None,
                    serde_json::json!({ "run_id": run.id.to_string(), "reason": "startup_repair" }),
                )
                .await?;
            requeued += 1;
        } else {
            if self.transition_cursor_blocks_startup_catchup(run).await? {
                let cancelled = work_items::cancel_pending_or_running_advance_by_run(
                    &self.pool,
                    run.id,
                    now,
                    "startup_repair_transition_cursor_parked",
                )
                .await?;
                if cancelled > 0 {
                    warn!(
                        run_id = %run.id,
                        cancelled = %cancelled,
                        "Startup recovery cancelled stale AdvanceRun work items for transition cursor parked run"
                    );
                }
                return Ok(requeued);
            }

            let mut requeued_targeted_advance = 0_u64;
            for authority in retry_stage_execution_authorities::list_by_run(&self.pool, run.id)
                .await?
                .into_iter()
                .filter(|authority| authority.authority_state == RetryAuthorityState::Active)
            {
                let tx_started = std::time::Instant::now();
                let mut tx = self
                    .begin_transaction(
                        "recovery.requeue_targeted_advance",
                        format!("recovery.requeue_targeted_advance:{}", authority.id),
                    )
                    .await?;
                let requeued_for_authority =
                    work_items::requeue_running_advance_by_retry_authority_tx(
                        &mut tx,
                        run.id,
                        &authority.id,
                        authority.target_stage_execution_id,
                        now,
                        "startup_repair_abandoned_targeted_advance_run",
                    )
                    .await?;
                tx.commit().await?;
                db::pool::log_write_transaction("recovery.requeue_targeted_advance", tx_started);
                requeued_targeted_advance += requeued_for_authority;
            }
            if requeued_targeted_advance > 0 {
                info!(
                    run_id = %run.id,
                    requeued = %requeued_targeted_advance,
                    "Startup recovery requeued abandoned targeted AdvanceRun work items"
                );
                requeued += requeued_targeted_advance as usize;
            }

            let requeued_running_advance = work_items::requeue_running_advance_by_run(
                &self.pool,
                run.id,
                now,
                "startup_repair_abandoned_advance_run",
            )
            .await?;
            if requeued_running_advance > 0 {
                info!(
                    run_id = %run.id,
                    requeued = %requeued_running_advance,
                    "Startup recovery requeued abandoned AdvanceRun work items"
                );
                requeued += requeued_running_advance as usize;
            }

            // Even if no stages are stuck in Running, the run may need
            // advancement — e.g. daemon crashed after settling a stage as
            // Completed but before evaluate_and_transition ran, or after
            // a loop-back transition updated current_state but before
            // enqueuing the follow-up AdvanceRun. Unconditionally enqueue
            // an AdvanceRun for any active run. The advance handler is
            // idempotent — if nothing needs doing, it returns Ok(()).
            let has_pending_work = run_has_pending_or_running_work(&self.pool, run.id)
                .await
                .unwrap_or(false);

            if !has_pending_work {
                if run_has_recovered_p091_orphan(&self.pool, run.id).await? {
                    info!(
                        run_id = %run.id,
                        "Active run has recovered P091 orphan and no live work — suppressing generic startup_catchup"
                    );
                    return Ok(requeued);
                }
                info!(
                    run_id = %run.id,
                    "Active run with no pending/running work — enqueuing AdvanceRun"
                );
                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(run.id),
                        None,
                        serde_json::json!({ "run_id": run.id.to_string(), "reason": "startup_catchup" }),
                    )
                    .await?;
                requeued += 1;
            }
        }

        Ok(requeued)
    }

    async fn repair_p091_orphaned_retry_attempts(&self, run: &Run) -> Result<P091OrphanRepairPass> {
        let mut pass = P091OrphanRepairPass::from_env(run.id.to_string());
        let now = Utc::now();
        if pass.disabled {
            pass.disabled_total = 1;
            self.record_p091_orphan_repair_pass(&pass, now).await?;
            return Ok(pass);
        }

        let stages_for_run = stages::list_by_run(&self.pool, run.id).await?;
        let mut repair_targets = Vec::new();
        let transition_cursor_parked = self.transition_cursor_blocks_startup_catchup(run).await?;
        let approvals_for_run = approvals::list_by_run(&self.pool, run.id).await?;
        let unresolved_side_effects =
            side_effects::list_unresolved_for_run(&self.pool, &run.id.to_string()).await?;
        for stage in stages_for_run
            .iter()
            .filter(|stage| matches!(stage.status, StageStatus::Pending | StageStatus::Running))
        {
            let active_retry_authority =
                retry_stage_execution_authorities::find_active_by_target(&self.pool, stage.id)
                    .await?;
            if stage_has_pending_or_running_invoke_work(&self.pool, run.id, stage.id).await?
                || stage_has_pending_or_running_advance_work(&self.pool, run.id, stage.id).await?
            {
                pass.exclude(stage.id.to_string(), "live_work_item");
                continue;
            }
            let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
            if executions
                .iter()
                .any(|execution| execution.status == AgentStatus::Running)
            {
                pass.exclude(stage.id.to_string(), "live_agent_execution");
                continue;
            }
            if transition_cursor_parked {
                pass.exclude(stage.id.to_string(), "transition_cursor_parked");
                continue;
            }
            if approvals_for_run.iter().any(|approval| {
                approval.stage_id == stage.stage_id
                    && matches!(
                        approval.decision,
                        ApprovalDecision::Pending | ApprovalDecision::Requested
                    )
            }) {
                pass.exclude(stage.id.to_string(), "pending_approval");
                continue;
            }
            if unresolved_side_effects
                .iter()
                .any(|effect| effect.stage_execution_id == stage.id)
            {
                pass.exclude(stage.id.to_string(), "unresolved_side_effect");
                continue;
            }
            let runtime_facts = {
                let mut facts_for_stage = Vec::new();
                for execution in &executions {
                    if let Some(facts) = agent_execution_runtime_facts::find_by_execution_id(
                        &self.pool,
                        execution.id,
                    )
                    .await?
                    {
                        facts_for_stage.push(facts);
                    }
                }
                facts_for_stage
            };
            if runtime_facts.iter().any(|facts| {
                facts
                    .retry_after
                    .map(|retry_after| retry_after > now)
                    .unwrap_or(false)
                    || matches!(
                        facts.operator_action_hint,
                        Some(domain::agent::OperatorActionHint::WaitUntilRetryAfter)
                    )
            }) {
                pass.exclude(stage.id.to_string(), "retry_after_or_quota_wait");
                continue;
            }
            if stage
                .recovery_snapshot_json
                .as_deref()
                .map(recovery_snapshot_represents_wait)
                .unwrap_or(false)
            {
                pass.exclude(stage.id.to_string(), "recovery_snapshot_wait");
                continue;
            }
            let qualifying_sibling = stages_for_run.iter().any(|sibling| {
                sibling.id != stage.id
                    && sibling.stage_id == stage.stage_id
                    && sibling.started_at >= stage.started_at
                    && matches!(
                        sibling.status,
                        StageStatus::Completed
                            | StageStatus::Failed
                            | StageStatus::Blocked
                            | StageStatus::Skipped
                    )
            });
            if !qualifying_sibling {
                pass.exclude(stage.id.to_string(), "no_qualifying_settled_sibling");
                continue;
            }

            pass.candidates_total += 1;
            pass.sample(
                stage.id.to_string(),
                "settled_sibling_without_live_retry_driver",
            );
            if pass.mode == "diagnostic" {
                pass.would_repair_total += 1;
                continue;
            }
            pass.would_repair_total += 1;
            repair_targets.push((
                stage.clone(),
                active_retry_authority.map(|authority| authority.id),
            ));
        }

        self.record_p091_orphan_repair_pass(&pass, now).await?;
        if pass.mode != "enforce" {
            return Ok(pass);
        }

        for (stage, active_authority_id) in repair_targets {
            let tx_started = std::time::Instant::now();
            let mut tx = self
                .begin_transaction(
                    "recovery.p091_orphan_retry_repair",
                    format!("recovery.p091_orphan_retry_repair:{}", stage.id),
                )
                .await?;
            stages::settle_with_terminal_reason_tx(
                &mut tx,
                stage.id,
                StageSettlementKind::Skipped,
                now,
                "stale_retry_recovered",
            )
            .await?;
            if let Some(authority_id) = active_authority_id {
                retry_stage_execution_authorities::mark_terminalized_tx(
                    &mut tx,
                    &authority_id,
                    now,
                    "stale_retry_recovered_after_conflict_resolution",
                )
                .await?;
            } else {
                retry_stage_execution_authorities::create_recovered_orphan_tx(
                    &mut tx,
                    format!("p091-recovered-orphan:{}", stage.id),
                    run.id,
                    stage.stage_id.clone(),
                    stage.id,
                    "stale_retry_recovered",
                    now,
                )
                .await?;
            }
            tx.commit().await?;
            db::pool::log_write_transaction("recovery.p091_orphan_retry_repair", tx_started);
            pass.repaired_total += 1;
        }

        self.record_p091_orphan_repair_pass(&pass, now).await?;
        Ok(pass)
    }

    async fn record_p091_orphan_repair_pass(
        &self,
        pass: &P091OrphanRepairPass,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let samples = serde_json::to_string(&pass.samples)?;
        sqlx::query(
            r#"INSERT INTO p091_orphan_repair_passes
               (id, mode, disabled, run_id, candidates_total, excluded_total,
                would_repair_total, repaired_total, disabled_total,
                bounded_samples_json, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
               ON CONFLICT(id) DO UPDATE SET
                 mode = excluded.mode,
                 disabled = excluded.disabled,
                 run_id = excluded.run_id,
                 candidates_total = excluded.candidates_total,
                 excluded_total = excluded.excluded_total,
                 would_repair_total = excluded.would_repair_total,
                 repaired_total = excluded.repaired_total,
                 disabled_total = excluded.disabled_total,
                 bounded_samples_json = excluded.bounded_samples_json,
                 created_at = excluded.created_at"#,
        )
        .bind(&pass.id)
        .bind(&pass.mode)
        .bind(if pass.disabled { 1 } else { 0 })
        .bind(&pass.run_id)
        .bind(pass.candidates_total)
        .bind(pass.excluded_total)
        .bind(pass.would_repair_total)
        .bind(pass.repaired_total)
        .bind(pass.disabled_total)
        .bind(samples)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn transition_cursor_blocks_startup_catchup(&self, run: &Run) -> Result<bool> {
        let Some(cursor) = workflow_conflicts::get_transition_cursor(&self.pool, run.id).await?
        else {
            return Ok(false);
        };
        let blocks_catchup = matches!(
            cursor.resume_policy.as_str(),
            "await_conflict_resolution" | "terminal_failure"
        );
        if blocks_catchup {
            info!(
                run_id = %run.id,
                current_state = %cursor.current_state_id,
                cursor_status = %cursor.cursor_status,
                resume_policy = %cursor.resume_policy,
                conflict_id = ?cursor.conflict_id,
                "Startup recovery leaving run parked at workflow transition cursor"
            );
        }
        Ok(blocks_catchup)
    }

    async fn latest_execution_provenance_suffix(
        &self,
        stage_execution_id: domain::ids::StageExecutionId,
    ) -> Option<String> {
        let executions = agent_executions::find_by_stage(&self.pool, stage_execution_id)
            .await
            .ok()?;
        let execution = executions.last()?;
        let mut details = Vec::new();

        if let Some(disposition) = execution.session_reuse_disposition.as_deref() {
            details.push(format!("reuse_disposition={disposition}"));
        }
        if let Some(reason) = execution.session_reset_reason.as_deref() {
            details.push(format!("reset_reason={reason}"));
        }
        if let Some(checkpoint_id) = execution.rehydrated_from_checkpoint_artifact_id.as_deref() {
            details.push(format!("checkpoint_artifact_id={checkpoint_id}"));
        }

        if details.is_empty() {
            None
        } else {
            Some(format!(
                "; latest execution provenance: {}",
                details.join(", ")
            ))
        }
    }

    /// Reclaim all P079 leases whose TTL has expired (REL-r2-2).
    ///
    /// Called on daemon startup. Budget rules:
    ///  - `reserved` lease: budget NOT consumed; result = unavailable; reason = ttl_expired_reserved
    ///  - `prompt_sent` lease: budget IS consumed; result = failed_transport; reason = ttl_expired_prompt_sent
    ///
    /// Each reclamation is atomic (event + lease in one transaction). On error, logs
    /// and continues so a single bad row doesn't block recovery of others.
    pub async fn run_p079_lease_sweep(&self) -> Result<usize> {
        let now_iso = Utc::now().to_rfc3339();
        let expired = match ocr_repo::list_expired_active_leases(&self.pool, &now_iso).await {
            Ok(leases) => leases,
            Err(e) => {
                warn!(error = %e, "P079 lease sweep: failed to list expired leases; skipping sweep");
                return Ok(0);
            }
        };

        let mut reclaimed = 0usize;
        for lease in &expired {
            let reclaim_ts = Utc::now().to_rfc3339();
            let (reason, event_status, event_presentation, budget_consumed) =
                match lease.lease_state {
                    domain::output_contract_repair::LeaseState::Reserved => {
                        ("ttl_expired_reserved", "blocked", "blocked", false)
                    }
                    domain::output_contract_repair::LeaseState::PromptSent => {
                        ("ttl_expired_prompt_sent", "failed", "failed", true)
                    }
                    domain::output_contract_repair::LeaseState::Settled => continue,
                };

            match ocr_repo::reclaim_expired_lease_and_event(
                &self.pool,
                &lease.lease_key,
                &lease.repair_event_id,
                event_status,
                event_presentation,
                "manual_investigation",
                reason,
                budget_consumed,
                &reclaim_ts,
            )
            .await
            {
                Ok(()) => {
                    reclaimed += 1;
                    warn!(
                        lease_key = %lease.lease_key,
                        repair_event_id = %lease.repair_event_id,
                        prior_state = %lease.lease_state,
                        reclamation_reason = %reason,
                        budget_consumed = budget_consumed,
                        "P079 lease TTL sweep: reclaimed expired lease"
                    );
                }
                Err(e) => {
                    warn!(
                        lease_key = %lease.lease_key,
                        repair_event_id = %lease.repair_event_id,
                        error = %e,
                        "P079 lease TTL sweep: reclamation failed (CAS or already terminal)"
                    );
                }
            }
        }
        Ok(reclaimed)
    }
}

fn sha256_file_if_exists(path: &str) -> Result<Option<String>> {
    let path = Path::new(path);
    if !path.exists() {
        return Ok(None);
    }
    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(path)?);
    Ok(Some(format!("{:x}", hasher.finalize())))
}

fn extract_p090_recovered_contract_status(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("implementation_status")
        .or_else(|| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// P066 T14: Scan TOOLCHAIN_HOME at daemon startup and:
/// 1. Reclaim orphan Go session-scoped roots (session_generation_id not live, age > 30 min).
/// 2. Quarantine Xcode run-scoped roots for active runs (in-memory lease state lost on crash).
///
/// Records counts in ToolchainCacheRecoveryReadback for persistence in startup_recovery_readbacks.
/// Tolerates missing TOOLCHAIN_HOME (first run, no agents executed yet).
async fn sweep_toolchain_cache_roots(
    pool: &SqlitePool,
    toolchain_home: &Path,
    sweep_started_at: chrono::DateTime<Utc>,
) -> Result<startup_repairs::ToolchainCacheRecoveryReadback> {
    const ORPHAN_THRESHOLD_MINUTES: i64 = 30;
    let orphan_threshold = Duration::from_secs(ORPHAN_THRESHOLD_MINUTES as u64 * 60);
    let now = SystemTime::now();

    let mut roots_seen = 0i64;
    let mut roots_reclaimed = 0i64;
    let mut cleanup_failures = 0i64;

    // ── Go session-scoped roots ──────────────────────────────────────────────
    let live_ids: HashSet<String> = sessions::list_live_session_generation_ids(pool)
        .await
        .unwrap_or_default();

    let go_dir = toolchain_home.join("providers").join("go");
    if let Ok(entries) = std::fs::read_dir(&go_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(gen_id) = dir_name(&path) else {
                continue;
            };
            roots_seen += 1;

            if live_ids.contains(gen_id.as_str()) {
                continue; // Live session — do not touch.
            }

            let age = dir_age(&path, now);
            if age < orphan_threshold {
                continue; // Too young to reclaim.
            }

            match std::fs::remove_dir_all(&path) {
                Ok(()) => {
                    roots_reclaimed += 1;
                    info!(
                        session_generation_id = %gen_id,
                        "Startup recovery reclaimed orphan Go session-scoped toolchain root"
                    );
                }
                Err(e) => {
                    cleanup_failures += 1;
                    warn!(
                        session_generation_id = %gen_id,
                        error = %e,
                        "Failed to reclaim orphan Go session-scoped toolchain root"
                    );
                }
            }
        }
    }

    // ── Xcode run-scoped roots ───────────────────────────────────────────────
    // On crash, in-memory XcodeRunLeaseRegistry state is lost. Any run-scoped
    // xcode/ directory for an ACTIVE run cannot be proven safe → quarantine.
    let xcode_dir = toolchain_home.join("providers").join("xcode");
    if let Ok(entries) = std::fs::read_dir(&xcode_dir) {
        let epoch_ms = sweep_started_at.timestamp_millis().to_string();
        for entry in entries.flatten() {
            let run_dir = entry.path();
            if !run_dir.is_dir() {
                continue;
            }
            let xcode_root = run_dir.join("xcode");
            if !xcode_root.is_dir() {
                continue;
            }
            let Some(run_id_str) = dir_name(&run_dir) else {
                continue;
            };

            if !is_active_run(pool, &run_id_str).await {
                continue; // Terminal run — housekeeping handles prune.
            }

            // Quarantine: move xcode/ → quarantine/{startup_epoch_ms}/
            let quarantine_dir = run_dir.join("quarantine").join(&epoch_ms);
            if let Some(parent) = quarantine_dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::rename(&xcode_root, &quarantine_dir) {
                Ok(()) => {
                    warn!(
                        run_id = %run_id_str,
                        quarantine_dir = %quarantine_dir.display(),
                        "Startup recovery quarantined Xcode run-scoped root after crash restart"
                    );
                }
                Err(e) => {
                    warn!(
                        run_id = %run_id_str,
                        error = %e,
                        "Failed to quarantine Xcode run-scoped root during startup recovery"
                    );
                }
            }
        }
    }

    Ok(startup_repairs::ToolchainCacheRecoveryReadback {
        session_scoped_roots_seen: Some(roots_seen),
        session_scoped_roots_reclaimed: Some(roots_reclaimed),
        session_scoped_cleanup_failures: Some(cleanup_failures),
        orphan_threshold_minutes: Some(ORPHAN_THRESHOLD_MINUTES),
        last_sweep_started_at: Some(sweep_started_at),
    })
}

/// Return the directory name (last path component) as a String.
fn dir_name(path: &PathBuf) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
}

/// Return the age of a directory based on its mtime. Falls back to 0 if stat fails.
fn dir_age(path: &PathBuf, now: SystemTime) -> Duration {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|mtime| now.duration_since(mtime).ok())
        .unwrap_or(Duration::ZERO)
}

/// Return true if the run exists in DB and is not in a terminal state.
async fn is_active_run(pool: &SqlitePool, run_id_str: &str) -> bool {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM runs WHERE id = ?1")
        .bind(run_id_str)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    status
        .map(|s| !matches!(s.as_str(), "completed" | "failed" | "cancelled"))
        .unwrap_or(false)
}

fn recovery_snapshot_represents_wait(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    [
        "waitingapproval",
        "waiting_approval",
        "approval",
        "manual_gate",
        "retry_after",
        "wait_until",
        "wait_until_retry_after",
        "provider_quota",
        "capacity",
        "backpressure",
        "side_effect",
        "transition_cursor",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

async fn rebuild_startup_read_projections(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<()> {
    projections::rebuild_all_for_run(pool, run_id).await
}
