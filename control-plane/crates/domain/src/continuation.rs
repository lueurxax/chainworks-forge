use serde::{Deserialize, Serialize};

/// P086: How the continuation attaches to the provider session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationMode {
    LiveHandleContinuation,
    ProviderSessionResurrection,
    OutputOnlyRecovery,
}

impl std::fmt::Display for ContinuationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContinuationMode::LiveHandleContinuation => write!(f, "live_handle_continuation"),
            ContinuationMode::ProviderSessionResurrection => {
                write!(f, "provider_session_resurrection")
            }
            ContinuationMode::OutputOnlyRecovery => write!(f, "output_only_recovery"),
        }
    }
}

impl std::str::FromStr for ContinuationMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "live_handle_continuation" => Ok(ContinuationMode::LiveHandleContinuation),
            "provider_session_resurrection" => Ok(ContinuationMode::ProviderSessionResurrection),
            "output_only_recovery" => Ok(ContinuationMode::OutputOnlyRecovery),
            other => Err(format!("unknown ContinuationMode: {other}")),
        }
    }
}

/// P086: What initiated this continuation turn.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationTriggerKind {
    OperatorMcp,
    LeadAuto,
}

impl std::fmt::Display for ContinuationTriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContinuationTriggerKind::OperatorMcp => write!(f, "operator_mcp"),
            ContinuationTriggerKind::LeadAuto => write!(f, "lead_auto"),
        }
    }
}

impl std::str::FromStr for ContinuationTriggerKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "operator_mcp" => Ok(ContinuationTriggerKind::OperatorMcp),
            "lead_auto" => Ok(ContinuationTriggerKind::LeadAuto),
            other => Err(format!("unknown ContinuationTriggerKind: {other}")),
        }
    }
}

/// P086: Lifecycle status of a continuation row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationStatus {
    Accepted,
    Queued,
    Starting,
    Running,
    PreflightPassed,
    PromptSent,
    Observing,
    WorktreeObserved,
    NeedsContinuationReconciliation,
    Finalizing,
    Cancelling,
    Succeeded,
    NoProgress,
    Failed,
    Cancelled,
}

impl ContinuationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ContinuationStatus::Succeeded
                | ContinuationStatus::NoProgress
                | ContinuationStatus::Failed
                | ContinuationStatus::Cancelled
        )
    }
}

impl std::fmt::Display for ContinuationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ContinuationStatus::Accepted => "accepted",
            ContinuationStatus::Queued => "queued",
            ContinuationStatus::Starting => "starting",
            ContinuationStatus::Running => "running",
            ContinuationStatus::PreflightPassed => "preflight_passed",
            ContinuationStatus::PromptSent => "prompt_sent",
            ContinuationStatus::Observing => "observing",
            ContinuationStatus::WorktreeObserved => "worktree_observed",
            ContinuationStatus::NeedsContinuationReconciliation => {
                "needs_continuation_reconciliation"
            }
            ContinuationStatus::Finalizing => "finalizing",
            ContinuationStatus::Cancelling => "cancelling",
            ContinuationStatus::Succeeded => "succeeded",
            ContinuationStatus::NoProgress => "no_progress",
            ContinuationStatus::Failed => "failed",
            ContinuationStatus::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for ContinuationStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "accepted" => Ok(ContinuationStatus::Accepted),
            "queued" => Ok(ContinuationStatus::Queued),
            "starting" => Ok(ContinuationStatus::Starting),
            "running" => Ok(ContinuationStatus::Running),
            "preflight_passed" => Ok(ContinuationStatus::PreflightPassed),
            "prompt_sent" => Ok(ContinuationStatus::PromptSent),
            "observing" => Ok(ContinuationStatus::Observing),
            "worktree_observed" => Ok(ContinuationStatus::WorktreeObserved),
            "needs_continuation_reconciliation" => {
                Ok(ContinuationStatus::NeedsContinuationReconciliation)
            }
            "finalizing" => Ok(ContinuationStatus::Finalizing),
            "cancelling" => Ok(ContinuationStatus::Cancelling),
            "succeeded" => Ok(ContinuationStatus::Succeeded),
            "no_progress" => Ok(ContinuationStatus::NoProgress),
            "failed" => Ok(ContinuationStatus::Failed),
            "cancelled" => Ok(ContinuationStatus::Cancelled),
            other => Err(format!("unknown ContinuationStatus: {other}")),
        }
    }
}

/// P086: Projection record for a single continuation turn (read-only).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinuationRecord {
    pub id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub command_journal_id: String,
    pub mode: String,
    pub trigger_kind: String,
    pub status: String,
    pub failure_reason: Option<String>,
    pub reconciliation_status: Option<String>,
    pub resurrection_phase: Option<String>,
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub request_fingerprint_sha256: String,
    pub canonical_request_artifact_id: Option<String>,
    pub attach_receipt_artifact_id: Option<String>,
    pub evidence_bundle_artifact_id: Option<String>,
    pub worktree_readback_artifact_id: Option<String>,
    pub continuation_report_artifact_id: Option<String>,
    pub response_fingerprint_sha256: Option<String>,
    pub response_artifact_id: Option<String>,
    pub result_or_no_progress_artifact_id: Option<String>,
    pub conflict_count: i64,
    pub lead_decision_artifact_id: Option<String>,
    pub lead_decision_artifact_sha256: Option<String>,
    pub continuation_instruction_sha256: Option<String>,
    pub budget_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// P086: Projection record for a continuation candidate (eligibility view).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinuationCandidate {
    pub agent_execution_id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_role: String,
    pub provider_session_id: Option<String>,
    pub status: String,
    pub eligible: bool,
    pub disabled_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_status_roundtrip() {
        let statuses = [
            "accepted",
            "queued",
            "starting",
            "running",
            "preflight_passed",
            "prompt_sent",
            "observing",
            "worktree_observed",
            "needs_continuation_reconciliation",
            "finalizing",
            "cancelling",
            "succeeded",
            "no_progress",
            "failed",
            "cancelled",
        ];
        for s in &statuses {
            let parsed: ContinuationStatus = s.parse().expect("parse");
            assert_eq!(parsed.to_string(), *s, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn continuation_status_terminal() {
        assert!(ContinuationStatus::Succeeded.is_terminal());
        assert!(ContinuationStatus::NoProgress.is_terminal());
        assert!(ContinuationStatus::Failed.is_terminal());
        assert!(ContinuationStatus::Cancelled.is_terminal());
        assert!(!ContinuationStatus::Accepted.is_terminal());
        assert!(!ContinuationStatus::PromptSent.is_terminal());
        assert!(!ContinuationStatus::Cancelling.is_terminal());
    }

    #[test]
    fn continuation_mode_roundtrip() {
        for s in &[
            "live_handle_continuation",
            "provider_session_resurrection",
            "output_only_recovery",
        ] {
            let m: ContinuationMode = s.parse().expect("parse");
            assert_eq!(m.to_string(), *s);
        }
    }

    #[test]
    fn continuation_trigger_kind_roundtrip() {
        for s in &["operator_mcp", "lead_auto"] {
            let t: ContinuationTriggerKind = s.parse().expect("parse");
            assert_eq!(t.to_string(), *s);
        }
    }
}
