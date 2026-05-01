use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConflictReason {
    InvalidNextStageHint,
    NoDeclarativeTransitionMatched,
    MultipleDeclarativeTransitionsMatchedWithoutTieBreak,
    RequiredArtifactOrFieldMissingForTransition,
    AggregateTransitionTruthConflicted,
    WorkflowConflictUnverifiable,
    ImplementationHandoffUnavailable,
}

impl std::fmt::Display for WorkflowConflictReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WorkflowConflictReason::InvalidNextStageHint => "invalid_next_stage_hint",
            WorkflowConflictReason::NoDeclarativeTransitionMatched => {
                "no_declarative_transition_matched"
            }
            WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak => {
                "multiple_declarative_transitions_matched_without_tie_break"
            }
            WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition => {
                "required_artifact_or_field_missing_for_transition"
            }
            WorkflowConflictReason::AggregateTransitionTruthConflicted => {
                "aggregate_transition_truth_conflicted"
            }
            WorkflowConflictReason::WorkflowConflictUnverifiable => {
                "workflow_conflict_unverifiable"
            }
            WorkflowConflictReason::ImplementationHandoffUnavailable => {
                "implementation_handoff_unavailable"
            }
        })
    }
}

impl WorkflowConflictReason {
    pub fn graphql_name(&self) -> &'static str {
        match self {
            WorkflowConflictReason::InvalidNextStageHint => "INVALID_NEXT_STAGE_HINT",
            WorkflowConflictReason::NoDeclarativeTransitionMatched => {
                "NO_DECLARATIVE_TRANSITION_MATCHED"
            }
            WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak => {
                "MULTIPLE_DECLARATIVE_TRANSITIONS_MATCHED_WITHOUT_TIE_BREAK"
            }
            WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition => {
                "REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION"
            }
            WorkflowConflictReason::AggregateTransitionTruthConflicted => {
                "AGGREGATE_TRANSITION_TRUTH_CONFLICTED"
            }
            WorkflowConflictReason::WorkflowConflictUnverifiable => {
                "WORKFLOW_CONFLICT_UNVERIFIABLE"
            }
            WorkflowConflictReason::ImplementationHandoffUnavailable => {
                "IMPLEMENTATION_HANDOFF_UNAVAILABLE"
            }
        }
    }
}

impl std::str::FromStr for WorkflowConflictReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "invalid_next_stage_hint" => WorkflowConflictReason::InvalidNextStageHint,
            "no_declarative_transition_matched" => {
                WorkflowConflictReason::NoDeclarativeTransitionMatched
            }
            "multiple_declarative_transitions_matched_without_tie_break" => {
                WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak
            }
            "required_artifact_or_field_missing_for_transition" => {
                WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition
            }
            "aggregate_transition_truth_conflicted" => {
                WorkflowConflictReason::AggregateTransitionTruthConflicted
            }
            "workflow_conflict_unverifiable" => {
                WorkflowConflictReason::WorkflowConflictUnverifiable
            }
            "implementation_handoff_unavailable" => {
                WorkflowConflictReason::ImplementationHandoffUnavailable
            }
            other => return Err(format!("Unknown WorkflowConflictReason: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConflictStatus {
    Unresolved,
    LeadMediationPending,
    OperatorConfirmationRequired,
    Resolved,
    Superseded,
    TerminalUnverifiable,
}

impl std::fmt::Display for WorkflowConflictStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WorkflowConflictStatus::Unresolved => "unresolved",
            WorkflowConflictStatus::LeadMediationPending => "lead_mediation_pending",
            WorkflowConflictStatus::OperatorConfirmationRequired => {
                "operator_confirmation_required"
            }
            WorkflowConflictStatus::Resolved => "resolved",
            WorkflowConflictStatus::Superseded => "superseded",
            WorkflowConflictStatus::TerminalUnverifiable => "terminal_unverifiable",
        })
    }
}

impl WorkflowConflictStatus {
    pub fn is_current_blocking(&self) -> bool {
        matches!(
            self,
            WorkflowConflictStatus::Unresolved
                | WorkflowConflictStatus::LeadMediationPending
                | WorkflowConflictStatus::OperatorConfirmationRequired
        )
    }

    /// Returns true when the conflict is already terminal, resolved, superseded,
    /// in mediation, or already requiring operator confirmation — i.e., not eligible
    /// for new Phase B mediation initiation.
    pub fn is_terminal_or_operator(&self) -> bool {
        matches!(
            self,
            WorkflowConflictStatus::TerminalUnverifiable
                | WorkflowConflictStatus::Resolved
                | WorkflowConflictStatus::Superseded
                | WorkflowConflictStatus::LeadMediationPending
                | WorkflowConflictStatus::OperatorConfirmationRequired
        )
    }

    pub fn graphql_name(&self) -> &'static str {
        match self {
            WorkflowConflictStatus::Unresolved => "UNRESOLVED",
            WorkflowConflictStatus::LeadMediationPending => "LEAD_MEDIATION_PENDING",
            WorkflowConflictStatus::OperatorConfirmationRequired => {
                "OPERATOR_CONFIRMATION_REQUIRED"
            }
            WorkflowConflictStatus::Resolved => "RESOLVED",
            WorkflowConflictStatus::Superseded => "SUPERSEDED",
            WorkflowConflictStatus::TerminalUnverifiable => "TERMINAL_UNVERIFIABLE",
        }
    }
}

impl std::str::FromStr for WorkflowConflictStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "unresolved" => WorkflowConflictStatus::Unresolved,
            "lead_mediation_pending" => WorkflowConflictStatus::LeadMediationPending,
            "operator_confirmation_required" => {
                WorkflowConflictStatus::OperatorConfirmationRequired
            }
            "resolved" => WorkflowConflictStatus::Resolved,
            "superseded" => WorkflowConflictStatus::Superseded,
            "terminal_unverifiable" => WorkflowConflictStatus::TerminalUnverifiable,
            other => return Err(format!("Unknown WorkflowConflictStatus: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateTransitionResult {
    Matched,
    NotMatched,
    MissingInput,
    InvalidExpression,
    EvaluationError,
}

impl std::fmt::Display for CandidateTransitionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CandidateTransitionResult::Matched => "matched",
            CandidateTransitionResult::NotMatched => "not_matched",
            CandidateTransitionResult::MissingInput => "missing_input",
            CandidateTransitionResult::InvalidExpression => "invalid_expression",
            CandidateTransitionResult::EvaluationError => "evaluation_error",
        })
    }
}

impl CandidateTransitionResult {
    pub fn graphql_name(&self) -> &'static str {
        match self {
            CandidateTransitionResult::Matched => "MATCHED",
            CandidateTransitionResult::NotMatched => "NOT_MATCHED",
            CandidateTransitionResult::MissingInput => "MISSING_INPUT",
            CandidateTransitionResult::InvalidExpression => "INVALID_EXPRESSION",
            CandidateTransitionResult::EvaluationError => "EVALUATION_ERROR",
        }
    }
}

impl std::str::FromStr for CandidateTransitionResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "matched" => CandidateTransitionResult::Matched,
            "not_matched" => CandidateTransitionResult::NotMatched,
            "missing_input" => CandidateTransitionResult::MissingInput,
            "invalid_expression" => CandidateTransitionResult::InvalidExpression,
            "evaluation_error" => CandidateTransitionResult::EvaluationError,
            other => return Err(format!("Unknown CandidateTransitionResult: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateTransitionEvaluation {
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
    pub condition_expression_id: Option<String>,
    pub result: CandidateTransitionResult,
    pub required_artifacts: Vec<String>,
    pub missing_artifacts: Vec<String>,
    pub missing_fields: Vec<String>,
    pub source_artifact_ids: Vec<String>,
    pub source_agent_execution_id: Option<String>,
    pub sanitized_diagnostic: Option<String>,
}

pub fn classify_workflow_conflict_reason(
    candidate_transitions: &[CandidateTransitionEvaluation],
) -> Option<WorkflowConflictReason> {
    let matched_count = candidate_transitions
        .iter()
        .filter(|candidate| candidate.result == CandidateTransitionResult::Matched)
        .count();
    if matched_count == 1 {
        return None;
    }
    if matched_count > 1 {
        return Some(WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak);
    }
    if candidate_transitions.iter().any(|candidate| {
        matches!(
            candidate.result,
            CandidateTransitionResult::InvalidExpression
                | CandidateTransitionResult::EvaluationError
        )
    }) {
        return Some(WorkflowConflictReason::WorkflowConflictUnverifiable);
    }
    if candidate_transitions
        .iter()
        .any(|candidate| candidate.result == CandidateTransitionResult::MissingInput)
    {
        return Some(WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition);
    }
    Some(WorkflowConflictReason::NoDeclarativeTransitionMatched)
}

pub fn candidate_transition_hash(
    candidate_transitions: &[CandidateTransitionEvaluation],
) -> String {
    sha256_prefixed_json(candidate_transitions)
}

pub fn workflow_conflict_fingerprint(
    run_id: &str,
    current_state_id: &str,
    reason: &WorkflowConflictReason,
    candidate_transition_hash: &str,
    advisory_evidence_refs: &[String],
) -> String {
    #[derive(Serialize)]
    struct FingerprintInput<'a> {
        schema: &'static str,
        run_id: &'a str,
        current_state_id: &'a str,
        reason: &'a WorkflowConflictReason,
        candidate_transition_hash: &'a str,
        advisory_evidence_refs: Vec<&'a str>,
    }

    let mut advisory_refs: Vec<&str> = advisory_evidence_refs
        .iter()
        .map(std::string::String::as_str)
        .collect();
    advisory_refs.sort_unstable();
    sha256_prefixed_json(&FingerprintInput {
        schema: "workflow_conflict_fingerprint_v1",
        run_id,
        current_state_id,
        reason,
        candidate_transition_hash,
        advisory_evidence_refs: advisory_refs,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowConflictRecord {
    pub conflict_id: String,
    pub conflict_fingerprint: String,
    pub run_id: String,
    pub stage_execution_id: Option<String>,
    pub lineage_id: Option<String>,
    pub current_state_id: String,
    pub reason: WorkflowConflictReason,
    pub operator_label: String,
    pub status: WorkflowConflictStatus,
    pub candidate_transitions: Vec<CandidateTransitionEvaluation>,
    pub candidate_transition_hash: String,
    pub advisory_evidence_refs: Vec<String>,
    pub lead_agent_id: Option<String>,
    pub mediation_record_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub superseded_by_conflict_id: Option<String>,
    pub resolution_record_json: Option<serde_json::Value>,
    pub terminal_failure_reason: Option<String>,
    pub diagnostic_redaction_tier: String,
}

pub fn workflow_conflict_suggested_operator_action(
    record: &WorkflowConflictRecord,
) -> Option<&'static str> {
    if record.reason != WorkflowConflictReason::NoDeclarativeTransitionMatched {
        return None;
    }
    if record.candidate_transitions.iter().any(|candidate| {
        candidate.result == CandidateTransitionResult::NotMatched
            && candidate
                .sanitized_diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.contains("Loop budget exhausted"))
    }) {
        Some("choose_transition_or_provide_refine_instruction")
    } else {
        None
    }
}

fn sha256_prefixed_json<T: Serialize + ?Sized>(value: &T) -> String {
    let json = serde_json::to_vec(value).expect("workflow conflict hash payload should serialize");
    let digest = Sha256::digest(json);
    format!("sha256:{digest:x}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAdvisoryRejectionRecord {
    pub rejection_id: String,
    pub run_id: String,
    pub stage_execution_id: Option<String>,
    pub lineage_id: Option<String>,
    pub current_state_id: String,
    pub selected_transition_id: String,
    pub selected_next_state_id: String,
    pub advisory_next_stage_hint: Option<String>,
    pub advisory_next_action: Option<String>,
    pub advisory_hint_hash: String,
    pub advisory_hint_provenance: Vec<AdvisoryHintExtraction>,
    pub graph_membership_result: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationHandoffStatus {
    pub schema_version: String,
    pub run_id: String,
    pub current_state_id: String,
    pub task_name: String,
    pub required_input_artifacts: Vec<String>,
    pub available_input_artifacts: Vec<String>,
    pub missing_input_artifacts: Vec<String>,
    pub approved_proposal_present: bool,
    #[serde(default)]
    pub approved_proposal_artifact_id: Option<String>,
    #[serde(default)]
    pub approved_proposal_digest: Option<String>,
    pub worktree_root: Option<String>,
    pub workspace_root: String,
    pub artifact_root: String,
    pub code_writer_start_status: String,
    pub status: String,
    #[serde(default)]
    pub missing_handoff_outputs: Vec<String>,
    #[serde(default)]
    pub last_handoff_agent_execution_id: Option<String>,
    #[serde(default)]
    pub retryable_from: Option<String>,
    #[serde(default)]
    pub blocked_before_code_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl ImplementationHandoffStatus {
    pub const SCHEMA_VERSION: &'static str = "p017_implementation_handoff_status_v1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTransitionCursorRecord {
    pub schema_version: String,
    pub run_id: String,
    pub current_state_id: String,
    pub cursor_status: String,
    pub resume_policy: String,
    pub selected_transition_id: Option<String>,
    pub selected_next_state_id: Option<String>,
    pub conflict_id: Option<String>,
    pub conflict_fingerprint: Option<String>,
    pub candidate_transition_hash: Option<String>,
    pub terminal_failure_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl WorkflowTransitionCursorRecord {
    pub const SCHEMA_VERSION: &'static str = "p017_workflow_transition_cursor_v1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisoryHintExtraction {
    pub source_artifact_id: String,
    pub source_agent_execution_id: Option<String>,
    pub advisory_path: String,
    pub raw_value_hash: String,
    pub redacted_value: Option<String>,
    pub graph_membership_result: String,
    pub superseded_by_projection: bool,
    pub included_in_candidate_transition_hash: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateFieldAuthority {
    TransitionAuthoritative,
    AdvisoryOnly,
    ContradictionBearing,
    NonAuthoritative,
}

impl std::fmt::Display for AggregateFieldAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AggregateFieldAuthority::TransitionAuthoritative => "transition_authoritative",
            AggregateFieldAuthority::AdvisoryOnly => "advisory_only",
            AggregateFieldAuthority::ContradictionBearing => "contradiction_bearing",
            AggregateFieldAuthority::NonAuthoritative => "non_authoritative",
        })
    }
}

impl std::str::FromStr for AggregateFieldAuthority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "transition_authoritative" => AggregateFieldAuthority::TransitionAuthoritative,
            "advisory_only" => AggregateFieldAuthority::AdvisoryOnly,
            "contradiction_bearing" => AggregateFieldAuthority::ContradictionBearing,
            "non_authoritative" => AggregateFieldAuthority::NonAuthoritative,
            other => return Err(format!("Unknown AggregateFieldAuthority: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateFieldAuthorityEntry {
    pub contract_id: String,
    pub field: String,
    pub authority: AggregateFieldAuthority,
    pub use_description: String,
}

pub fn proposal_review_summary_v1_field_authority(field: &str) -> Option<AggregateFieldAuthority> {
    Some(match field {
        "pass" | "blocker_count" | "blocking_issues" | "required_changes" => {
            AggregateFieldAuthority::TransitionAuthoritative
        }
        "decision" => AggregateFieldAuthority::ContradictionBearing,
        "next_action" | "next_stage" => AggregateFieldAuthority::AdvisoryOnly,
        "summary" => AggregateFieldAuthority::NonAuthoritative,
        _ => return None,
    })
}

pub fn proposal_review_summary_v1_authority_table() -> Vec<AggregateFieldAuthorityEntry> {
    [
        (
            "pass",
            AggregateFieldAuthority::TransitionAuthoritative,
            "Primary pass/fail branch input for review-loop transitions.",
        ),
        (
            "blocker_count",
            AggregateFieldAuthority::TransitionAuthoritative,
            "Confirms whether failed review must route to refinement.",
        ),
        (
            "blocking_issues",
            AggregateFieldAuthority::TransitionAuthoritative,
            "Provides blocker presence and issue refs for failed-review transition conditions.",
        ),
        (
            "required_changes",
            AggregateFieldAuthority::TransitionAuthoritative,
            "Provides concrete blocker/remediation evidence for failed-review transition conditions.",
        ),
        (
            "decision",
            AggregateFieldAuthority::ContradictionBearing,
            "May indicate internal aggregate inconsistency when it conflicts with pass, blocker_count, or blocking_issues.",
        ),
        (
            "next_action",
            AggregateFieldAuthority::AdvisoryOnly,
            "Recorded as advisory transition evidence; never selects a graph transition alone.",
        ),
        (
            "next_stage",
            AggregateFieldAuthority::AdvisoryOnly,
            "Graph membership is checked for advisory rejection evidence; absent states never become legal transitions.",
        ),
        (
            "summary",
            AggregateFieldAuthority::NonAuthoritative,
            "Operator explanation only; not transition input.",
        ),
    ]
    .into_iter()
    .map(|(field, authority, use_description)| AggregateFieldAuthorityEntry {
        contract_id: "proposal_review_summary_v1".to_string(),
        field: field.to_string(),
        authority,
        use_description: use_description.to_string(),
    })
    .collect()
}
