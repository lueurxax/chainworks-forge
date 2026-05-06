// P077: Active artifact contract — proposal_gate_result_v1
//
// Statuses from R14 §architecture.contracts:
//   passed, failed, waived, missing_definition, stale, invalid, unauthorized, superseded
//
// Schema-invalid payloads are contract-invalid (do not become active).
// Well-formed fail-closed domain statuses ARE valid active generations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const PROPOSAL_GATE_RESULT_V1_CONTRACT_ID: &str = "proposal_gate_result_v1";
pub const PROPOSAL_GATE_RESULT_ARTIFACT_PATH: &str = "review/proposal-gate-result.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGateStatus {
    Passed,
    Failed,
    Waived,
    MissingDefinition,
    Stale,
    Invalid,
    Unauthorized,
    Superseded,
}

impl ProposalGateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalGateStatus::Passed => "passed",
            ProposalGateStatus::Failed => "failed",
            ProposalGateStatus::Waived => "waived",
            ProposalGateStatus::MissingDefinition => "missing_definition",
            ProposalGateStatus::Stale => "stale",
            ProposalGateStatus::Invalid => "invalid",
            ProposalGateStatus::Unauthorized => "unauthorized",
            ProposalGateStatus::Superseded => "superseded",
        }
    }

    /// True for statuses that are well-formed but indicate a fail-closed
    /// domain condition (not a parse/schema error). Per R14 §architecture:
    /// these ARE valid active generations.
    pub fn is_fail_closed_domain_status(&self) -> bool {
        matches!(
            self,
            ProposalGateStatus::Failed
                | ProposalGateStatus::MissingDefinition
                | ProposalGateStatus::Stale
                | ProposalGateStatus::Unauthorized
        )
    }

    /// True iff this status allows entry to manual release (with appropriate
    /// risk lineage for waived).
    pub fn allows_entry(&self) -> bool {
        matches!(
            self,
            ProposalGateStatus::Passed | ProposalGateStatus::Waived
        )
    }
}

impl std::fmt::Display for ProposalGateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ProposalGateStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "passed" => Ok(ProposalGateStatus::Passed),
            "failed" => Ok(ProposalGateStatus::Failed),
            "waived" => Ok(ProposalGateStatus::Waived),
            "missing_definition" => Ok(ProposalGateStatus::MissingDefinition),
            "stale" => Ok(ProposalGateStatus::Stale),
            "invalid" => Ok(ProposalGateStatus::Invalid),
            "unauthorized" => Ok(ProposalGateStatus::Unauthorized),
            "superseded" => Ok(ProposalGateStatus::Superseded),
            other => Err(format!("unknown ProposalGateStatus: {other}")),
        }
    }
}

/// Failure classification for failed proposal gate results.
/// Drives routing in the closeout synthesizer per R14 §architecture.gate_cause_routing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGateFailureClassification {
    /// Code-owned failures with implementation budget remaining → return_to_code_refine.
    CodeOwnedBudgetRemaining,
    /// Non-code-owned failures or unclear failures → await_operator_decision.
    UnclearOrNonCodeOwned,
    /// Budget exhausted regardless of failure ownership → await_operator_decision.
    BudgetExhausted,
}

impl ProposalGateFailureClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalGateFailureClassification::CodeOwnedBudgetRemaining => {
                "code_owned_budget_remaining"
            }
            ProposalGateFailureClassification::UnclearOrNonCodeOwned => "unclear_or_non_code_owned",
            ProposalGateFailureClassification::BudgetExhausted => "budget_exhausted",
        }
    }
}

impl std::str::FromStr for ProposalGateFailureClassification {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "code_owned_budget_remaining" => {
                Ok(ProposalGateFailureClassification::CodeOwnedBudgetRemaining)
            }
            "unclear_or_non_code_owned" => {
                Ok(ProposalGateFailureClassification::UnclearOrNonCodeOwned)
            }
            "budget_exhausted" => Ok(ProposalGateFailureClassification::BudgetExhausted),
            other => Err(format!(
                "unknown ProposalGateFailureClassification: {other}"
            )),
        }
    }
}

/// Required lineage fields for a governed gate-settlement command.
/// All fields must be present; unmanaged receipts (missing lineage) are rejected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalGateLineage {
    pub principal: String,
    pub capability: String,
    pub journal_id: String,
    pub authority: String,
    pub reason: String,
    pub source_artifacts: Vec<String>,
    pub run_id: String,
    pub proposal_id: String,
    pub stage_id: String,
    pub workflow_digest: String,
    pub worktree_head: String,
    pub dirty_or_changed_file_digest: String,
    pub source_generation_ids: Vec<String>,
    pub current_fingerprint: String,
}

/// Active generation record for proposal_gate_result_v1.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalGateResult {
    pub gate_id: String,
    pub proposal_id: String,
    pub run_id: String,
    pub stage_id: String,
    pub status: ProposalGateStatus,
    pub generation_id: String,
    pub diagnostic_reason: Option<String>,
    pub executor_version: Option<String>,
    pub evidence_digest: Option<String>,
    pub exit_code: Option<i32>,
    pub elapsed_ms: Option<u64>,
    pub settled_at: DateTime<Utc>,
    pub authorization_lineage: Option<ProposalGateLineage>,
    /// Present on Failed results; drives routing between return_to_code_refine
    /// and await_operator_decision per R14 §architecture.gate_cause_routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_classification: Option<ProposalGateFailureClassification>,
}

impl ProposalGateResult {
    /// Construct a fail-closed missing-definition result — valid active generation.
    pub fn missing_definition(
        generation_id: impl Into<String>,
        run_id: impl Into<String>,
        proposal_id: impl Into<String>,
        stage_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let proposal_id_str = proposal_id.into();
        ProposalGateResult {
            gate_id: format!("p{}:{}", proposal_id_str, proposal_id_str),
            proposal_id: proposal_id_str,
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            status: ProposalGateStatus::MissingDefinition,
            generation_id: generation_id.into(),
            diagnostic_reason: Some(reason.into()),
            executor_version: None,
            evidence_digest: None,
            exit_code: None,
            elapsed_ms: None,
            settled_at: Utc::now(),
            authorization_lineage: None,
            failure_classification: None,
        }
    }

    /// True iff this result was produced by a valid schema parse (even if the
    /// domain status itself is fail-closed).
    pub fn is_schema_valid(&self) -> bool {
        !matches!(self.status, ProposalGateStatus::Invalid)
    }
}

/// Parse a JSON value into a ProposalGateResult. Returns Err if the payload is
/// schema-invalid — schema-invalid payloads must NOT become active generations.
pub fn parse_proposal_gate_result(
    raw: &serde_json::Value,
    generation_id: impl Into<String>,
) -> Result<ProposalGateResult, String> {
    let obj = raw
        .as_object()
        .ok_or_else(|| "proposal_gate_result_v1: expected JSON object".to_string())?;

    let status_str = obj
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "proposal_gate_result_v1: missing required field 'status'".to_string())?;

    let status: ProposalGateStatus = status_str
        .parse()
        .map_err(|e| format!("proposal_gate_result_v1: invalid status: {e}"))?;

    let gate_id = obj
        .get("gate_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let proposal_id = obj
        .get("proposal_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let run_id = obj
        .get("run_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let stage_id = obj
        .get("stage_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let diagnostic_reason = obj
        .get("diagnostic_reason")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let executor_version = obj
        .get("executor_version")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let evidence_digest = obj
        .get("evidence_digest")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let exit_code = obj
        .get("exit_code")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let elapsed_ms = obj.get("elapsed_ms").and_then(|v| v.as_u64());

    let settled_at = obj
        .get("settled_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    let failure_classification = obj
        .get("failure_classification")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<ProposalGateFailureClassification>().ok());

    Ok(ProposalGateResult {
        gate_id,
        proposal_id,
        run_id,
        stage_id,
        status,
        generation_id: generation_id.into(),
        diagnostic_reason,
        executor_version,
        evidence_digest,
        exit_code,
        elapsed_ms,
        settled_at,
        authorization_lineage: None,
        failure_classification,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_statuses_round_trip_through_str() {
        let statuses = [
            ("passed", ProposalGateStatus::Passed),
            ("failed", ProposalGateStatus::Failed),
            ("waived", ProposalGateStatus::Waived),
            ("missing_definition", ProposalGateStatus::MissingDefinition),
            ("stale", ProposalGateStatus::Stale),
            ("invalid", ProposalGateStatus::Invalid),
            ("unauthorized", ProposalGateStatus::Unauthorized),
            ("superseded", ProposalGateStatus::Superseded),
        ];
        for (s, expected) in &statuses {
            let parsed: ProposalGateStatus = s.parse().unwrap();
            assert_eq!(&parsed, expected, "from_str mismatch for {s}");
            assert_eq!(parsed.as_str(), *s, "as_str mismatch for {s}");
        }
    }

    #[test]
    fn fail_closed_domain_statuses_are_valid_active_generations() {
        assert!(ProposalGateStatus::Failed.is_fail_closed_domain_status());
        assert!(ProposalGateStatus::MissingDefinition.is_fail_closed_domain_status());
        assert!(ProposalGateStatus::Stale.is_fail_closed_domain_status());
        assert!(ProposalGateStatus::Unauthorized.is_fail_closed_domain_status());
        assert!(!ProposalGateStatus::Passed.is_fail_closed_domain_status());
        assert!(!ProposalGateStatus::Waived.is_fail_closed_domain_status());
        assert!(!ProposalGateStatus::Invalid.is_fail_closed_domain_status());
    }

    #[test]
    fn passed_and_waived_allow_entry() {
        assert!(ProposalGateStatus::Passed.allows_entry());
        assert!(ProposalGateStatus::Waived.allows_entry());
        assert!(!ProposalGateStatus::Failed.allows_entry());
        assert!(!ProposalGateStatus::MissingDefinition.allows_entry());
    }

    #[test]
    fn schema_invalid_payload_returns_err_not_active() {
        let raw = json!("not an object");
        assert!(parse_proposal_gate_result(&raw, "gen-1").is_err());
    }

    #[test]
    fn missing_status_field_returns_err() {
        let raw = json!({"gate_id": "p077:077", "run_id": "run-1"});
        assert!(parse_proposal_gate_result(&raw, "gen-1").is_err());
    }

    #[test]
    fn unknown_status_value_returns_err() {
        let raw = json!({"status": "invented_status"});
        assert!(parse_proposal_gate_result(&raw, "gen-1").is_err());
    }

    #[test]
    fn valid_passed_payload_parses() {
        let raw = json!({
            "status": "passed",
            "gate_id": "p077:077",
            "proposal_id": "077",
            "run_id": "run-abc",
            "stage_id": "state_9",
        });
        let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
        assert_eq!(result.status, ProposalGateStatus::Passed);
        assert!(result.is_schema_valid());
    }

    #[test]
    fn well_formed_fail_closed_statuses_parse_as_valid_active_generations() {
        for status in ["failed", "missing_definition", "stale", "unauthorized"] {
            let raw = json!({"status": status});
            let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
            assert!(result.is_schema_valid(), "{status} should be schema-valid");
            assert!(
                result.status.is_fail_closed_domain_status(),
                "{status} should be fail-closed"
            );
        }
    }

    #[test]
    fn missing_definition_constructor_produces_valid_generation() {
        let r = ProposalGateResult::missing_definition(
            "gen-x",
            "run-1",
            "077",
            "state_9",
            "gate script not registered",
        );
        assert_eq!(r.status, ProposalGateStatus::MissingDefinition);
        assert!(r.is_schema_valid());
        assert!(r.diagnostic_reason.is_some());
        assert_eq!(r.gate_id, "p077:077");
        assert_eq!(r.proposal_id, "077");
    }

    #[test]
    fn missing_definition_constructor_uses_proposal_id_parameter_not_hardcoded() {
        let r = ProposalGateResult::missing_definition(
            "gen-y",
            "run-2",
            "123",
            "state_5",
            "gate not registered",
        );
        assert_eq!(
            r.gate_id, "p123:123",
            "gate_id must use proposal_id parameter"
        );
        assert_eq!(r.proposal_id, "123", "proposal_id must use parameter");
        assert_eq!(r.status, ProposalGateStatus::MissingDefinition);
    }

    #[test]
    fn failure_classification_round_trips_through_parse() {
        let raw = json!({
            "status": "failed",
            "failure_classification": "code_owned_budget_remaining",
        });
        let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
        assert_eq!(
            result.failure_classification,
            Some(ProposalGateFailureClassification::CodeOwnedBudgetRemaining)
        );
    }

    #[test]
    fn failure_classification_budget_exhausted_parses() {
        let raw = json!({"status": "failed", "failure_classification": "budget_exhausted"});
        let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
        assert_eq!(
            result.failure_classification,
            Some(ProposalGateFailureClassification::BudgetExhausted)
        );
    }

    #[test]
    fn missing_failure_classification_parses_as_none() {
        let raw = json!({"status": "failed"});
        let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
        assert!(result.failure_classification.is_none());
    }

    #[test]
    fn unknown_failure_classification_value_parses_as_none_not_err() {
        let raw = json!({"status": "failed", "failure_classification": "invented_value"});
        let result = parse_proposal_gate_result(&raw, "gen-1").unwrap();
        assert!(result.failure_classification.is_none());
    }
}
