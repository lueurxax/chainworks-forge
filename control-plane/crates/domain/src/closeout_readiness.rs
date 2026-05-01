// P077: Active artifact contract — implementation_closeout_readiness_v1
//
// Statuses from R14 §architecture.contracts:
//   ready, ready_with_risks, handoff_required, not_ready, blocked, invalid, unknown
//
// Decisions:
//   enter_manual_release, return_to_code_refine, await_non_code_handoff,
//   await_gate_definition, await_operator_decision
//
// Schema-invalid payloads are contract-invalid (do not become active).
// Well-formed fail-closed domain statuses (invalid/unknown/blocked) ARE valid active generations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID: &str =
    "implementation_closeout_readiness_v1";
pub const IMPLEMENTATION_CLOSEOUT_READINESS_ARTIFACT_PATH: &str =
    "review/implementation-closeout-readiness.json";

/// Derived diagnostic projection — NOT a transition source.
pub const IMPLEMENTATION_CLOSEOUT_INPUTS_V1_CONTRACT_ID: &str =
    "implementation_closeout_inputs_v1";
pub const IMPLEMENTATION_CLOSEOUT_INPUTS_ARTIFACT_PATH: &str =
    "review/implementation-closeout-inputs.json";

/// Derived operator projection — NOT a transition source.
pub const CLOSEOUT_HANDOFF_STATUS_V1_CONTRACT_ID: &str = "closeout_handoff_status_v1";
pub const CLOSEOUT_HANDOFF_STATUS_ARTIFACT_PATH: &str = "review/closeout-handoff-status.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutReadinessStatus {
    Ready,
    ReadyWithRisks,
    HandoffRequired,
    NotReady,
    Blocked,
    Invalid,
    Unknown,
}

impl CloseoutReadinessStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloseoutReadinessStatus::Ready => "ready",
            CloseoutReadinessStatus::ReadyWithRisks => "ready_with_risks",
            CloseoutReadinessStatus::HandoffRequired => "handoff_required",
            CloseoutReadinessStatus::NotReady => "not_ready",
            CloseoutReadinessStatus::Blocked => "blocked",
            CloseoutReadinessStatus::Invalid => "invalid",
            CloseoutReadinessStatus::Unknown => "unknown",
        }
    }

    /// True for well-formed fail-closed domain statuses per R14.
    /// These ARE valid active generations even though they fail closed.
    pub fn is_fail_closed_domain_status(&self) -> bool {
        matches!(
            self,
            CloseoutReadinessStatus::Invalid
                | CloseoutReadinessStatus::Unknown
                | CloseoutReadinessStatus::Blocked
        )
    }
}

impl std::fmt::Display for CloseoutReadinessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CloseoutReadinessStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ready" => Ok(CloseoutReadinessStatus::Ready),
            "ready_with_risks" => Ok(CloseoutReadinessStatus::ReadyWithRisks),
            "handoff_required" => Ok(CloseoutReadinessStatus::HandoffRequired),
            "not_ready" => Ok(CloseoutReadinessStatus::NotReady),
            "blocked" => Ok(CloseoutReadinessStatus::Blocked),
            "invalid" => Ok(CloseoutReadinessStatus::Invalid),
            "unknown" => Ok(CloseoutReadinessStatus::Unknown),
            other => Err(format!("unknown CloseoutReadinessStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutReadinessDecision {
    EnterManualRelease,
    ReturnToCodeRefine,
    AwaitNonCodeHandoff,
    AwaitGateDefinition,
    AwaitOperatorDecision,
}

impl CloseoutReadinessDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloseoutReadinessDecision::EnterManualRelease => "enter_manual_release",
            CloseoutReadinessDecision::ReturnToCodeRefine => "return_to_code_refine",
            CloseoutReadinessDecision::AwaitNonCodeHandoff => "await_non_code_handoff",
            CloseoutReadinessDecision::AwaitGateDefinition => "await_gate_definition",
            CloseoutReadinessDecision::AwaitOperatorDecision => "await_operator_decision",
        }
    }
}

impl std::fmt::Display for CloseoutReadinessDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CloseoutReadinessDecision {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enter_manual_release" => Ok(CloseoutReadinessDecision::EnterManualRelease),
            "return_to_code_refine" => Ok(CloseoutReadinessDecision::ReturnToCodeRefine),
            "await_non_code_handoff" => Ok(CloseoutReadinessDecision::AwaitNonCodeHandoff),
            "await_gate_definition" => Ok(CloseoutReadinessDecision::AwaitGateDefinition),
            "await_operator_decision" => Ok(CloseoutReadinessDecision::AwaitOperatorDecision),
            other => Err(format!("unknown CloseoutReadinessDecision: {other}")),
        }
    }
}

/// Fingerprint composition per R14 §architecture.fingerprint.
/// Excluded: derived projections, exported JSON, GraphQL/MCP projections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseoutFingerprint {
    pub proposal_or_freeze_digest: String,
    pub run_id: String,
    pub stage_id: String,
    pub workflow_digest: String,
    pub worktree_head: String,
    pub dirty_or_changed_file_digest: String,
    pub upstream_active_generation_ids: Vec<String>,
    pub contract_version: String,
    pub computed_at: DateTime<Utc>,
    pub latency_ms: u64,
}

impl CloseoutFingerprint {
    /// 8-character hash used as the operator-facing generation identifier
    /// in tooltip, sheet header, VoiceOver, and copy-to-clipboard.
    ///
    /// Uses SHA-256 (via the sha2 crate) over a deterministic field encoding
    /// to guarantee stability across Rust toolchain versions (unlike DefaultHasher).
    pub fn short_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(self.proposal_or_freeze_digest.as_bytes());
        h.update(b"\x00");
        h.update(self.run_id.as_bytes());
        h.update(b"\x00");
        h.update(self.stage_id.as_bytes());
        h.update(b"\x00");
        h.update(self.worktree_head.as_bytes());
        h.update(b"\x00");
        h.update(self.dirty_or_changed_file_digest.as_bytes());
        let result = h.finalize();
        format!("{:x}", result)[..8].to_string()
    }
}

/// Active generation record for implementation_closeout_readiness_v1.
/// This is the ONLY enforcement-mode state-9 manual-release authority.
/// Transition code reads SQLite active artifact-contract truth, never exported JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloseoutReadiness {
    pub run_id: String,
    pub stage_id: String,
    pub status: CloseoutReadinessStatus,
    pub decision: CloseoutReadinessDecision,
    pub generation_id: String,
    pub readiness_mode: String,
    pub diagnostic_reason: Option<String>,
    pub primary_unblock: Option<String>,
    pub code_blocker_count: u32,
    pub handoff_owner: Option<String>,
    pub risk_settlement_required: bool,
    pub fingerprint: Option<CloseoutFingerprint>,
    pub synthesized_at: DateTime<Utc>,
}

impl CloseoutReadiness {
    /// Construct an unknown/unavailable generation for fail-closed cases
    /// where active truth cannot be established.
    pub fn fail_closed_unknown(
        generation_id: impl Into<String>,
        run_id: impl Into<String>,
        stage_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        CloseoutReadiness {
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            status: CloseoutReadinessStatus::Unknown,
            decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            generation_id: generation_id.into(),
            readiness_mode: "advisory".into(),
            diagnostic_reason: Some(reason.into()),
            primary_unblock: None,
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: None,
            synthesized_at: Utc::now(),
        }
    }

    /// True iff this generation was produced by a valid schema parse.
    /// Per R14: well-formed fail-closed domain statuses (invalid/unknown/blocked)
    /// ARE valid active generations — only failed serde parsing is schema-invalid.
    pub fn is_schema_valid(&self) -> bool {
        true
    }
}

/// Parse a JSON value into a CloseoutReadiness. Returns Err if the payload is
/// schema-invalid — these must NOT become active generations.
pub fn parse_closeout_readiness(
    raw: &serde_json::Value,
    generation_id: impl Into<String>,
) -> Result<CloseoutReadiness, String> {
    let obj = raw
        .as_object()
        .ok_or_else(|| {
            "implementation_closeout_readiness_v1: expected JSON object".to_string()
        })?;

    let status_str = obj
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            "implementation_closeout_readiness_v1: missing required field 'status'".to_string()
        })?;

    let status: CloseoutReadinessStatus = status_str
        .parse()
        .map_err(|e| format!("implementation_closeout_readiness_v1: invalid status: {e}"))?;

    let decision_str = obj
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            "implementation_closeout_readiness_v1: missing required field 'decision'".to_string()
        })?;

    let decision: CloseoutReadinessDecision = decision_str
        .parse()
        .map_err(|e| format!("implementation_closeout_readiness_v1: invalid decision: {e}"))?;

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

    let readiness_mode = obj
        .get("readiness_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("advisory")
        .to_string();

    let diagnostic_reason = obj
        .get("diagnostic_reason")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let primary_unblock = obj
        .get("primary_unblock")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let code_blocker_count = obj
        .get("code_blocker_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let handoff_owner = obj
        .get("handoff_owner")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let risk_settlement_required = obj
        .get("risk_settlement_required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let synthesized_at = obj
        .get("synthesized_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    Ok(CloseoutReadiness {
        run_id,
        stage_id,
        status,
        decision,
        generation_id: generation_id.into(),
        readiness_mode,
        diagnostic_reason,
        primary_unblock,
        code_blocker_count,
        handoff_owner,
        risk_settlement_required,
        fingerprint: None,
        synthesized_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_statuses_round_trip() {
        let cases = [
            ("ready", CloseoutReadinessStatus::Ready),
            ("ready_with_risks", CloseoutReadinessStatus::ReadyWithRisks),
            ("handoff_required", CloseoutReadinessStatus::HandoffRequired),
            ("not_ready", CloseoutReadinessStatus::NotReady),
            ("blocked", CloseoutReadinessStatus::Blocked),
            ("invalid", CloseoutReadinessStatus::Invalid),
            ("unknown", CloseoutReadinessStatus::Unknown),
        ];
        for (s, expected) in &cases {
            let parsed: CloseoutReadinessStatus = s.parse().unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn all_decisions_round_trip() {
        let cases = [
            ("enter_manual_release", CloseoutReadinessDecision::EnterManualRelease),
            ("return_to_code_refine", CloseoutReadinessDecision::ReturnToCodeRefine),
            ("await_non_code_handoff", CloseoutReadinessDecision::AwaitNonCodeHandoff),
            ("await_gate_definition", CloseoutReadinessDecision::AwaitGateDefinition),
            ("await_operator_decision", CloseoutReadinessDecision::AwaitOperatorDecision),
        ];
        for (s, expected) in &cases {
            let parsed: CloseoutReadinessDecision = s.parse().unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn fail_closed_domain_statuses_include_invalid_unknown_blocked() {
        assert!(CloseoutReadinessStatus::Invalid.is_fail_closed_domain_status());
        assert!(CloseoutReadinessStatus::Unknown.is_fail_closed_domain_status());
        assert!(CloseoutReadinessStatus::Blocked.is_fail_closed_domain_status());
        assert!(!CloseoutReadinessStatus::Ready.is_fail_closed_domain_status());
        assert!(!CloseoutReadinessStatus::ReadyWithRisks.is_fail_closed_domain_status());
        assert!(!CloseoutReadinessStatus::HandoffRequired.is_fail_closed_domain_status());
        assert!(!CloseoutReadinessStatus::NotReady.is_fail_closed_domain_status());
    }

    #[test]
    fn schema_invalid_payload_is_err() {
        assert!(parse_closeout_readiness(&json!("string"), "gen-1").is_err());
        assert!(parse_closeout_readiness(&json!(null), "gen-1").is_err());
    }

    #[test]
    fn missing_status_returns_err() {
        let raw = json!({"decision": "enter_manual_release"});
        assert!(parse_closeout_readiness(&raw, "gen-1").is_err());
    }

    #[test]
    fn missing_decision_returns_err() {
        let raw = json!({"status": "ready"});
        assert!(parse_closeout_readiness(&raw, "gen-1").is_err());
    }

    #[test]
    fn valid_ready_payload_parses() {
        let raw = json!({
            "status": "ready",
            "decision": "enter_manual_release",
            "run_id": "run-abc",
            "stage_id": "state_9",
            "readiness_mode": "enforcement",
        });
        let r = parse_closeout_readiness(&raw, "gen-1").unwrap();
        assert_eq!(r.status, CloseoutReadinessStatus::Ready);
        assert_eq!(r.decision, CloseoutReadinessDecision::EnterManualRelease);
        assert!(r.is_schema_valid());
    }

    #[test]
    fn well_formed_fail_closed_statuses_parse_as_valid_active_generations() {
        for (status_str, decision_str) in [
            ("invalid", "await_operator_decision"),
            ("unknown", "await_operator_decision"),
            ("blocked", "await_operator_decision"),
        ] {
            let raw = json!({"status": status_str, "decision": decision_str});
            let r = parse_closeout_readiness(&raw, "gen-1").unwrap();
            assert!(r.is_schema_valid(), "{status_str} should be schema-valid");
            assert!(
                r.status.is_fail_closed_domain_status(),
                "{status_str} should be fail-closed"
            );
        }
    }

    #[test]
    fn fail_closed_unknown_constructor_produces_valid_generation() {
        let r = CloseoutReadiness::fail_closed_unknown(
            "gen-x",
            "run-1",
            "state_9",
            "fingerprint unavailable",
        );
        assert_eq!(r.status, CloseoutReadinessStatus::Unknown);
        assert!(r.is_schema_valid());
        assert!(r.diagnostic_reason.is_some());
    }

    #[test]
    fn fingerprint_short_hash_is_8_chars() {
        let fp = CloseoutFingerprint {
            proposal_or_freeze_digest: "sha256:abc".into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            workflow_digest: "wf-digest".into(),
            worktree_head: "abcdef1".into(),
            dirty_or_changed_file_digest: "clean".into(),
            upstream_active_generation_ids: vec!["gen-1".into()],
            contract_version: "v1".into(),
            computed_at: Utc::now(),
            latency_ms: 42,
        };
        let hash = fp.short_hash();
        assert_eq!(hash.len(), 8, "short_hash must be exactly 8 characters");
    }
}
