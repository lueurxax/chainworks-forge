use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::{AgentOutputSettlement, ArtifactSourceClaimState};
use crate::ids::{AgentExecutionId, ArtifactId, RunId, StageExecutionId};
use crate::mediation::OwnerKind;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedContractStatus {
    pub contract_id: String,
    pub raw_status: String,
    pub canonical_status: String,
    pub valid: bool,
    pub validation_errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn contract_status_allowed_values(contract_id: &str) -> Option<&'static [&'static str]> {
    match contract_id {
        "prepush_review_v1" | "security_report_v1" => {
            Some(&["pass", "block", "invalid", "unknown"])
        }
        "docs_report_v1" => Some(&["pass", "not_needed", "block", "invalid", "unknown"]),
        "audit_report_v1" => Some(&["implemented", "needs_code_fixes", "invalid", "unknown"]),
        "implementation_review_summary_v1" => Some(&[
            "code_complete",
            "needs_code_fixes",
            "release_evidence_blocked",
            "invalid",
        ]),
        "implementation_self_assessment_v2" => Some(&[
            "complete",
            "needs_code_fixes",
            "blocked",
            "handoff_required",
            "unknown",
            "invalid",
        ]),
        "tests_result_v1" => Some(&["green", "red", "blocked", "unknown"]),
        _ => None,
    }
}

pub fn normalize_contract_status(
    contract_id: &str,
    raw_status: &str,
) -> Result<NormalizedContractStatus, String> {
    let raw = raw_status.trim();
    let normalized = match contract_id {
        "prepush_review_v1" => match raw {
            "PASS" | "PASS_WITH_NOTES" | "pass" | "conditional_pass" => Some("pass"),
            "BLOCK" | "needs_fixes" | "block" | "changes_required" | "fail" | "failed" => {
                Some("block")
            }
            "invalid" => Some("invalid"),
            "unknown" => Some("unknown"),
            _ => None,
        },
        "docs_report_v1" => match raw {
            "success" | "synced" | "pass" | "aligned" => Some("pass"),
            "not_needed" => Some("not_needed"),
            "blocked" | "block" => Some("block"),
            "invalid" => Some("invalid"),
            "unknown" => Some("unknown"),
            _ => None,
        },
        "security_report_v1" => match raw {
            "PASS" | "pass" | "pass_with_notes" => Some("pass"),
            "BLOCK" | "block" | "fail" | "failed" => Some("block"),
            "invalid" => Some("invalid"),
            "unknown" => Some("unknown"),
            _ => None,
        },
        "audit_report_v1" => match raw {
            "Implemented" | "implemented" => Some("implemented"),
            "Partially Implemented" | "Partially implemented" | "needs_code_fixes" => {
                Some("needs_code_fixes")
            }
            "invalid" => Some("invalid"),
            "unknown" => Some("unknown"),
            _ => None,
        },
        "implementation_review_summary_v1" => match raw {
            "code_complete" | "implemented" => Some("code_complete"),
            "needs_code_fixes" | "changes_required" | "blocked" | "block" => {
                Some("needs_code_fixes")
            }
            "release_evidence_blocked" => Some("release_evidence_blocked"),
            "invalid" => Some("invalid"),
            _ => None,
        },
        "implementation_self_assessment_v2" => match raw {
            "complete" | "implemented" | "implementation_complete" => Some("complete"),
            "needs_code_fixes" | "incomplete" => Some("needs_code_fixes"),
            "blocked" => Some("blocked"),
            "handoff_required" => Some("handoff_required"),
            "unknown" => Some("unknown"),
            "invalid" => Some("invalid"),
            _ => None,
        },
        "tests_result_v1" => match raw {
            "green" | "red" | "blocked" | "unknown" => Some(raw),
            _ => None,
        },
        _ => match raw {
            "pass" | "block" | "invalid" | "unknown" => Some(raw),
            _ => None,
        },
    };

    let mut warnings = Vec::new();
    if let Some(canonical) = normalized {
        if canonical != raw {
            warnings.push(format!("normalized {raw} to {canonical}"));
        }
        Ok(NormalizedContractStatus {
            contract_id: contract_id.to_string(),
            raw_status: raw.to_string(),
            canonical_status: canonical.to_string(),
            valid: canonical != "invalid",
            validation_errors: Vec::new(),
            warnings,
        })
    } else {
        Ok(NormalizedContractStatus {
            contract_id: contract_id.to_string(),
            raw_status: raw.to_string(),
            canonical_status: "invalid".to_string(),
            valid: false,
            validation_errors: vec![format!("unknown status value: {raw}")],
            warnings,
        })
    }
}

pub fn known_contract_id(contract_id: &str) -> bool {
    matches!(
        contract_id,
        "audit_report_v1"
            | "security_report_v1"
            | "prepush_review_v1"
            | "docs_report_v1"
            | "proposal_review_summary_v2"
            | "implementation_self_assessment_v2"
            | "tests_result_v1"
            | "implementation_review_summary_v1"
            | "run_state_projection_v1"
    )
}

pub const PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID: &str = "proposal_review_summary_v2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalReviewSummaryTransitionTruth {
    pub contract_id: String,
    pub pass: Option<bool>,
    pub blocker_count: Option<u64>,
    pub has_blocking_issues: bool,
    pub has_required_changes: bool,
    pub decision: Option<String>,
}

impl ProposalReviewSummaryTransitionTruth {
    pub fn has_blocking_evidence(&self) -> bool {
        self.blocker_count.is_some_and(|count| count > 0)
            || self.has_blocking_issues
            || self.has_required_changes
    }
}

pub fn proposal_review_summary_v2_validation_error(raw_json: &Value) -> Option<String> {
    let object = raw_json.as_object()?;

    if object.get("pass").and_then(Value::as_bool).is_none() {
        return Some("proposal_review_summary_v2 field 'pass' must be a boolean".into());
    }
    for field in ["average_score", "aggregate_score", "min_individual_score"] {
        if object.get(field).and_then(Value::as_f64).is_none() {
            return Some(format!(
                "proposal_review_summary_v2 field '{field}' must be a number"
            ));
        }
    }
    if object
        .get("blocker_count")
        .and_then(Value::as_u64)
        .is_none()
    {
        return Some(
            "proposal_review_summary_v2 field 'blocker_count' must be a non-negative integer"
                .into(),
        );
    }
    for field in [
        "blocking_issues",
        "blocking_required_changes",
        "advisory_follow_ups",
        "recurring_themes",
    ] {
        if object.get(field).and_then(Value::as_array).is_none() {
            return Some(format!(
                "proposal_review_summary_v2 field '{field}' must be an array"
            ));
        }
    }
    for field in ["summary", "decision"] {
        if object.get(field).and_then(Value::as_str).is_none() {
            return Some(format!(
                "proposal_review_summary_v2 field '{field}' must be a string"
            ));
        }
    }

    let truth = proposal_review_summary_transition_truth(raw_json);
    if truth.contract_id != PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID {
        return Some(
            "proposal_review_summary_v2 payload could not be interpreted as a v2 proposal review summary"
                .into(),
        );
    }

    if let Some(error) = proposal_review_summary_transition_truth_conflict(raw_json) {
        return Some(error);
    }

    None
}

pub fn proposal_review_summary_transition_truth(
    raw_json: &Value,
) -> ProposalReviewSummaryTransitionTruth {
    let pass = raw_json.get("pass").and_then(Value::as_bool);
    let blocker_count = raw_json.get("blocker_count").and_then(Value::as_u64);
    let blocking_issues = raw_json.get("blocking_issues");
    let decision = raw_json
        .get("decision")
        .and_then(Value::as_str)
        .map(str::to_string);

    if looks_like_proposal_review_summary_v2(raw_json) {
        let blocking_required_changes = raw_json.get("blocking_required_changes");
        ProposalReviewSummaryTransitionTruth {
            contract_id: PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID.to_string(),
            pass,
            blocker_count,
            has_blocking_issues: json_value_has_entries(blocking_issues),
            has_required_changes: json_value_has_entries(blocking_required_changes),
            decision,
        }
    } else {
        let required_changes = raw_json.get("required_changes");
        ProposalReviewSummaryTransitionTruth {
            contract_id: "proposal_review_summary_v1".to_string(),
            pass,
            blocker_count,
            has_blocking_issues: json_value_has_entries(blocking_issues),
            has_required_changes: json_value_has_entries(required_changes),
            decision,
        }
    }
}

pub fn proposal_review_summary_transition_truth_conflict(raw_json: &Value) -> Option<String> {
    let truth = proposal_review_summary_transition_truth(raw_json);
    let contract_label = truth.contract_id.as_str();
    let has_blocking_evidence = truth.has_blocking_evidence();

    if truth.pass == Some(true) && has_blocking_evidence {
        return Some(format!(
            "{contract_label} has pass=true while blocker evidence is non-empty"
        ));
    }

    let explicitly_no_blocking_issues = raw_json
        .get("blocking_issues")
        .is_some_and(json_value_is_empty_collection);
    let explicitly_no_required_changes =
        if truth.contract_id == PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID {
            raw_json
                .get("blocking_required_changes")
                .is_some_and(json_value_is_empty_collection)
        } else {
            raw_json
                .get("required_changes")
                .is_some_and(json_value_is_empty_collection)
        };
    if truth.pass == Some(false)
        && truth.blocker_count == Some(0)
        && explicitly_no_blocking_issues
        && explicitly_no_required_changes
    {
        return Some(format!(
            "{contract_label} has pass=false while blocker evidence is explicitly empty"
        ));
    }

    if let Some(decision) = truth.decision.as_deref() {
        let decision = decision.to_ascii_lowercase();
        let decision_passes = decision.contains("pass")
            || decision.contains("approve")
            || decision.contains("approved");
        let decision_blocks = decision.contains("fail")
            || decision.contains("block")
            || decision.contains("revise")
            || decision.contains("changes_required");
        if decision_passes && (truth.pass == Some(false) || has_blocking_evidence) {
            return Some(format!(
                "{contract_label} decision indicates pass while authoritative fields block"
            ));
        }
        if decision_blocks && truth.pass == Some(true) && !has_blocking_evidence {
            return Some(format!(
                "{contract_label} decision indicates blocking while authoritative fields pass"
            ));
        }
    }

    None
}

fn looks_like_proposal_review_summary_v2(raw_json: &Value) -> bool {
    raw_json.get("blocking_required_changes").is_some()
        || raw_json.get("advisory_follow_ups").is_some()
}

fn json_value_has_entries(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(entries)) => !entries.is_empty(),
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(Value::Bool(true)) => true,
        Some(Value::Number(number)) => number.as_u64().is_some_and(|count| count > 0),
        _ => false,
    }
}

fn json_value_is_empty_collection(value: &Value) -> bool {
    matches!(value, Value::Array(items) if items.is_empty())
        || matches!(value, Value::Object(entries) if entries.is_empty())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradedOutputPolicyMode {
    Deny,
    AllowValidContractOutputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailedExecutionSettlement {
    ValidOutputsFromFailedExecution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedOutputPolicy {
    pub mode: DegradedOutputPolicyMode,
    pub contracts: Vec<String>,
    pub failure_kinds: Vec<String>,
    pub max_settlement: Option<FailedExecutionSettlement>,
}

impl Default for DegradedOutputPolicy {
    fn default() -> Self {
        Self {
            mode: DegradedOutputPolicyMode::Deny,
            contracts: Vec::new(),
            failure_kinds: Vec::new(),
            max_settlement: Some(FailedExecutionSettlement::ValidOutputsFromFailedExecution),
        }
    }
}

impl DegradedOutputPolicy {
    pub fn allow_valid_contract_outputs(
        contracts: Vec<String>,
        failure_kinds: Vec<String>,
    ) -> Result<Self, String> {
        if contracts.is_empty() {
            return Err("allow_valid_contract_outputs requires at least one contract".into());
        }
        Ok(Self {
            mode: DegradedOutputPolicyMode::AllowValidContractOutputs,
            contracts,
            failure_kinds,
            max_settlement: Some(FailedExecutionSettlement::ValidOutputsFromFailedExecution),
        })
    }

    pub fn allows(
        &self,
        contract_id: &str,
        failure_kind: &str,
        settlement: FailedExecutionSettlement,
    ) -> bool {
        if self.mode != DegradedOutputPolicyMode::AllowValidContractOutputs {
            return false;
        }
        if self.max_settlement != Some(settlement) {
            return false;
        }
        if !self.contracts.iter().any(|item| item == contract_id) {
            return false;
        }
        self.failure_kinds.is_empty() || self.failure_kinds.iter().any(|item| item == failure_kind)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveArtifactGenerationInput {
    pub run_id: RunId,
    pub artifact_id: ArtifactId,
    pub contract_id: String,
    pub canonical_path: String,
    pub raw_path: String,
    pub raw_status: String,
    pub generation_id: String,
    pub source_agent_execution_id: Option<String>,
    pub source_stage_execution_id: Option<String>,
    pub source_session_generation_id: Option<String>,
    pub source_work_item_id: Option<String>,
    pub supersedes_generation_id: Option<String>,
    pub output_settlement: AgentOutputSettlement,
    pub partial: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceGenerationClaimKey {
    pub run_id: RunId,
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub stage_execution_id: Option<StageExecutionId>,
    pub agent_execution_id: AgentExecutionId,
    pub source_work_item_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceGenerationClaim {
    pub key: ArtifactSourceGenerationClaimKey,
    pub current_session_generation_id: Option<String>,
    pub claim_state: ArtifactSourceClaimState,
    pub superseding_work_item_id: Option<String>,
    pub superseded_by_agent_execution_id: Option<String>,
    pub supersession_journal_id: Option<String>,
    pub superseded_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceGenerationImportDecision {
    Activated,
    InvalidRejected,
    IgnoredLateOutputs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactContractOverrideInput {
    pub run_id: RunId,
    pub contract_id: String,
    pub override_type: String,
    pub from_status: String,
    pub to_status: String,
    pub reason: String,
    pub owner: String,
    pub source_artifacts: Vec<String>,
    pub expires_at_stage: String,
    pub journal_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactContractOverride {
    pub override_id: String,
    pub run_id: RunId,
    pub contract_id: String,
    pub override_type: String,
    pub from_status: String,
    pub to_status: String,
    pub reason: String,
    pub owner: String,
    pub source_artifacts: Vec<String>,
    pub expires_at_stage: String,
    pub journal_id: String,
    pub created_at: DateTime<Utc>,
    pub expired_at: Option<DateTime<Utc>>,
    pub active: bool,
}

include!("implementation_self_assessment_contract.rs");
