use async_graphql::*;
use chrono;
use domain::continuation::ContinuationRecord;

use crate::types::p031::GqlFreshnessState;

pub fn display_candidate_status(raw: &str) -> String {
    // AgentExecution status values for code_writer candidates
    match raw {
        "completed" => "Completed".to_string(),
        "failed" => "Failed".to_string(),
        _ => format!("UNKNOWN({raw})"),
    }
}

fn display_status(raw: &str) -> String {
    match raw {
        "accepted" => "Accepted".to_string(),
        "queued" => "Queued".to_string(),
        "starting" => "Starting".to_string(),
        "running" => "Running".to_string(),
        "preflight_passed" => "Preflight Passed".to_string(),
        "prompt_sent" => "Prompt Sent".to_string(),
        "observing" => "Observing".to_string(),
        "worktree_observed" => "Worktree Observed".to_string(),
        "needs_continuation_reconciliation" => "Needs Reconciliation".to_string(),
        "finalizing" => "Finalizing".to_string(),
        "cancelling" => "Cancelling".to_string(),
        "succeeded" => "Succeeded".to_string(),
        "no_progress" => "No Progress".to_string(),
        "failed" => "Failed".to_string(),
        "cancelled" => "Cancelled".to_string(),
        _ => format!("UNKNOWN({raw})"),
    }
}

fn display_mode(raw: &str) -> String {
    match raw {
        "live_handle_continuation" => "Live Handle Continuation".to_string(),
        "provider_session_resurrection" => "Provider Session Resurrection".to_string(),
        _ => format!("UNKNOWN({raw})"),
    }
}

fn display_trigger_kind(raw: &str) -> String {
    match raw {
        "operator_mcp" => "Operator MCP".to_string(),
        "lead_auto" => "Lead Auto".to_string(),
        _ => format!("UNKNOWN({raw})"),
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "succeeded" | "no_progress" | "failed" | "cancelled")
}

/// P086: Read-only GraphQL projection of a continuation turn.
/// Exposes raw (daemon-canonical) values alongside display labels;
/// unrecognised raw values produce UNKNOWN(<raw>) display strings
/// so the UI degrades gracefully when daemon adds new states.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ContinuationRecord")]
pub struct GqlContinuationRecord {
    pub id: ID,
    pub run_id: ID,
    pub stage_execution_id: ID,
    pub agent_execution_id: ID,

    pub mode_raw: String,
    pub mode_display: String,

    pub trigger_kind_raw: String,
    pub trigger_kind_display: String,

    pub status_raw: String,
    pub status_display: String,
    pub is_terminal: bool,

    pub failure_reason: Option<String>,
    pub reconciliation_status: Option<String>,

    pub request_fingerprint_sha256: String,
    pub canonical_request_artifact_id: Option<ID>,
    pub attach_receipt_artifact_id: Option<ID>,
    pub evidence_bundle_artifact_id: Option<ID>,
    pub worktree_readback_artifact_id: Option<ID>,
    pub continuation_report_artifact_id: Option<ID>,
    pub response_fingerprint_sha256: Option<String>,
    pub response_artifact_id: Option<ID>,
    pub result_or_no_progress_artifact_id: Option<ID>,

    pub conflict_count: i64,

    pub created_at: String,
    pub updated_at: String,

    /// Projection freshness relative to now. Continuation records are read
    /// directly from the DB (no separate projection table), so this reflects
    /// direct-read recency: Live when updated_at is within 5 s, Stale otherwise.
    pub freshness_state: GqlFreshnessState,
    pub projection_lag_ms: Option<i64>,
}

impl From<ContinuationRecord> for GqlContinuationRecord {
    fn from(r: ContinuationRecord) -> Self {
        let mode_display = display_mode(&r.mode);
        let trigger_kind_display = display_trigger_kind(&r.trigger_kind);
        let status_display = display_status(&r.status);
        let is_terminal = is_terminal_status(&r.status);

        // Compute projection lag: time since updated_at.
        let (freshness_state, projection_lag_ms) = compute_continuation_freshness(&r.updated_at);

        GqlContinuationRecord {
            id: r.id.into(),
            run_id: r.run_id.into(),
            stage_execution_id: r.stage_execution_id.into(),
            agent_execution_id: r.agent_execution_id.into(),
            mode_raw: r.mode,
            mode_display,
            trigger_kind_raw: r.trigger_kind,
            trigger_kind_display,
            status_raw: r.status,
            status_display,
            is_terminal,
            failure_reason: r.failure_reason,
            reconciliation_status: r.reconciliation_status,
            request_fingerprint_sha256: r.request_fingerprint_sha256,
            canonical_request_artifact_id: r.canonical_request_artifact_id.map(Into::into),
            attach_receipt_artifact_id: r.attach_receipt_artifact_id.map(Into::into),
            evidence_bundle_artifact_id: r.evidence_bundle_artifact_id.map(Into::into),
            worktree_readback_artifact_id: r.worktree_readback_artifact_id.map(Into::into),
            continuation_report_artifact_id: r.continuation_report_artifact_id.map(Into::into),
            response_fingerprint_sha256: r.response_fingerprint_sha256,
            response_artifact_id: r.response_artifact_id.map(Into::into),
            result_or_no_progress_artifact_id: r.result_or_no_progress_artifact_id.map(Into::into),
            conflict_count: r.conflict_count,
            created_at: r.created_at,
            updated_at: r.updated_at,
            freshness_state,
            projection_lag_ms,
        }
    }
}

/// P086: Read-only GraphQL projection of a continuation candidate.
/// Exposes eligibility, raw agent status, and optional disabled reason.
/// provider_session_id is included only for Operator principals; callers
/// are responsible for omitting it when the principal is not Operator.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ContinuationCandidate")]
pub struct GqlContinuationCandidate {
    pub agent_execution_id: ID,
    pub run_id: ID,
    pub stage_execution_id: ID,
    pub agent_role: String,
    pub status_raw: String,
    pub status_display: String,
    pub eligible: bool,
    pub disabled_reason: Option<String>,
    /// Null when the principal is not Operator (omitted to prevent session-id leakage).
    pub provider_session_id: Option<String>,
}

/// P086: Aggregate status and history for a single agent execution's continuations.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ContinuationStatus")]
pub struct GqlContinuationStatus {
    pub agent_execution_id: ID,
    /// Most recent non-terminal continuation, if any.
    pub active: Option<GqlContinuationRecord>,
    /// Full history, most recent first (capped at 200).
    pub history: Vec<GqlContinuationRecord>,
    /// Staleness of the most recent read. Live when history is non-empty and
    /// freshly read; Unavailable when the agent_execution_id was not found.
    pub freshness_state: GqlFreshnessState,
}

/// P086: List of continuation candidates for a run.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ContinuationCandidatesResult")]
pub struct GqlContinuationCandidatesResult {
    pub run_id: ID,
    pub candidates: Vec<GqlContinuationCandidate>,
    pub freshness_state: GqlFreshnessState,
}

/// P086: Durable rollout metrics summary for continuation behavior on a run.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ContinuationMetricsSummary")]
pub struct GqlContinuationMetricsSummary {
    pub run_id: ID,
    pub admission_total: i64,
    pub accepted_total: i64,
    pub rejected_total: i64,
    pub replay_total: i64,
    pub success_total: i64,
    pub no_progress_total: i64,
    pub failed_total: i64,
    pub cancelled_total: i64,
    pub fresh_session_avoided_total: i64,
    pub lead_auto_total: i64,
    pub operator_mcp_total: i64,
    pub changed_files_total: i64,
    pub tests_or_gates_total: i64,
    pub terminal_total: i64,
    pub useful_progress_total: i64,
    pub useful_progress_rate: f64,
    pub no_progress_rate: f64,
    pub tests_passed_after_continuation_total: i64,
    pub followup_validation_total: i64,
    pub followup_validation_success_total: i64,
    pub followup_validation_success_rate: f64,
    pub lead_auto_success_total: i64,
    pub lead_auto_success_rate: f64,
    pub operator_mcp_success_total: i64,
    pub operator_mcp_success_rate: f64,
    pub time_saved_seconds_total: i64,
    pub time_saved_sample_count: i64,
    pub average_time_saved_seconds: f64,
    pub provider_session_budget_input_tokens_total: i64,
    pub provider_session_budget_output_tokens_total: i64,
    pub provider_session_budget_cached_input_tokens_total: i64,
    pub provider_session_budget_cost_cents_total: i64,
    pub provider_session_resurrection_attach_success_total: i64,
    pub provider_session_resurrection_attach_failure_total: i64,
    pub orphan_reap_attempted_total: i64,
    pub orphan_reap_verified_total: i64,
    pub resurrection_unsupported_total: i64,
}

impl From<domain::continuation::ContinuationCandidate> for GqlContinuationCandidate {
    fn from(c: domain::continuation::ContinuationCandidate) -> Self {
        let status_display = display_candidate_status(&c.status);
        GqlContinuationCandidate {
            agent_execution_id: c.agent_execution_id.into(),
            run_id: c.run_id.into(),
            stage_execution_id: c.stage_execution_id.into(),
            agent_role: c.agent_role,
            status_raw: c.status,
            status_display,
            eligible: c.eligible,
            disabled_reason: c.disabled_reason,
            provider_session_id: c.provider_session_id,
        }
    }
}

impl From<db::repos::agent_work_continuations::P086ContinuationMetricsSummary>
    for GqlContinuationMetricsSummary
{
    fn from(summary: db::repos::agent_work_continuations::P086ContinuationMetricsSummary) -> Self {
        GqlContinuationMetricsSummary {
            run_id: summary.run_id.into(),
            admission_total: summary.admission_total,
            accepted_total: summary.accepted_total,
            rejected_total: summary.rejected_total,
            replay_total: summary.replay_total,
            success_total: summary.success_total,
            no_progress_total: summary.no_progress_total,
            failed_total: summary.failed_total,
            cancelled_total: summary.cancelled_total,
            fresh_session_avoided_total: summary.fresh_session_avoided_total,
            lead_auto_total: summary.lead_auto_total,
            operator_mcp_total: summary.operator_mcp_total,
            changed_files_total: summary.changed_files_total,
            tests_or_gates_total: summary.tests_or_gates_total,
            terminal_total: summary.terminal_total,
            useful_progress_total: summary.useful_progress_total,
            useful_progress_rate: summary.useful_progress_rate,
            no_progress_rate: summary.no_progress_rate,
            tests_passed_after_continuation_total: summary.tests_passed_after_continuation_total,
            followup_validation_total: summary.followup_validation_total,
            followup_validation_success_total: summary.followup_validation_success_total,
            followup_validation_success_rate: summary.followup_validation_success_rate,
            lead_auto_success_total: summary.lead_auto_success_total,
            lead_auto_success_rate: summary.lead_auto_success_rate,
            operator_mcp_success_total: summary.operator_mcp_success_total,
            operator_mcp_success_rate: summary.operator_mcp_success_rate,
            time_saved_seconds_total: summary.time_saved_seconds_total,
            time_saved_sample_count: summary.time_saved_sample_count,
            average_time_saved_seconds: summary.average_time_saved_seconds,
            provider_session_budget_input_tokens_total: summary
                .provider_session_budget_input_tokens_total,
            provider_session_budget_output_tokens_total: summary
                .provider_session_budget_output_tokens_total,
            provider_session_budget_cached_input_tokens_total: summary
                .provider_session_budget_cached_input_tokens_total,
            provider_session_budget_cost_cents_total: summary
                .provider_session_budget_cost_cents_total,
            provider_session_resurrection_attach_success_total: summary
                .provider_session_resurrection_attach_success_total,
            provider_session_resurrection_attach_failure_total: summary
                .provider_session_resurrection_attach_failure_total,
            orphan_reap_attempted_total: summary.orphan_reap_attempted_total,
            orphan_reap_verified_total: summary.orphan_reap_verified_total,
            resurrection_unsupported_total: summary.resurrection_unsupported_total,
        }
    }
}

/// Compute freshness and lag from an RFC-3339 updated_at timestamp.
/// Returns (state, Some(lag_ms)). Lag > 5000 ms → ProjectionLag; otherwise Live.
pub fn compute_continuation_freshness(updated_at: &str) -> (GqlFreshnessState, Option<i64>) {
    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(updated_at) {
        let now = chrono::Utc::now();
        let lag_ms = (now - ts.with_timezone(&chrono::Utc))
            .num_milliseconds()
            .max(0);
        let state = if lag_ms > 5000 {
            GqlFreshnessState::ProjectionLag
        } else {
            GqlFreshnessState::Live
        };
        (state, Some(lag_ms))
    } else {
        (GqlFreshnessState::Unavailable, None)
    }
}
