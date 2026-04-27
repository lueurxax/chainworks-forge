//! P060: Deterministic reviewer routing domain types.
//!
//! Defines SystemExecution, RoutingReceipt, AgentSelectionPlanV1,
//! CompiledDynamicAgentBinding, DynamicMaterializationRecord,
//! and ReviewRoutingOptions — the data contracts specified in
//! Proposal 060 §6.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{
    DynamicMaterializationId, RoutingReceiptId, RunId, StageExecutionId, SystemExecutionId,
};

// ── SystemExecution ──────────────────────────────────────────────────

/// Status of a system-executed task (no provider invocation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemExecutionStatus {
    Queued,
    Running,
    Succeeded,
    Blocked,
    Failed,
}

impl std::fmt::Display for SystemExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Blocked => write!(f, "blocked"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for SystemExecutionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            other => Err(format!("Unknown SystemExecutionStatus: {other}")),
        }
    }
}

/// A system-executed task record (P060 §6). Owns the task lifecycle for
/// `system.routing` — no AgentExecution is created for system tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemExecution {
    pub id: SystemExecutionId,
    pub run_id: RunId,
    pub stage_id: String,
    pub attempt_id: i64,
    pub task_id: String,
    pub task_type: String,
    pub status: SystemExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub receipt_id: Option<RoutingReceiptId>,
    pub plan_hash: Option<String>,
    pub failure_kind: Option<String>,
}

// ── RoutingReceipt ───────────────────────────────────────────────────

/// Terminal routing outcome status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingReceiptStatus {
    Succeeded,
    Failed,
    Blocked,
}

impl std::fmt::Display for RoutingReceiptStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

impl std::str::FromStr for RoutingReceiptStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            other => Err(format!("Unknown RoutingReceiptStatus: {other}")),
        }
    }
}

/// A RoutingReceipt is created for every terminal routing outcome.
/// For failed routing, `receipt_id` and `failure_kind` are mirrored
/// into `StageExecution.validation_failure_json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingReceipt {
    pub receipt_id: RoutingReceiptId,
    pub run_id: RunId,
    pub stage_id: String,
    pub attempt_id: i64,
    pub system_execution_id: SystemExecutionId,
    pub status: RoutingReceiptStatus,
    pub failure_kind: Option<String>,
    pub plan_hash: Option<String>,
    /// JSON-encoded `InputSnapshotHashes`.
    pub input_snapshot_hashes_json: Option<String>,
    /// JSON-encoded operator action references.
    pub operator_actions_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── AgentSelectionPlanV1 ─────────────────────────────────────────────

/// The routing plan artifact: emitted only when routing succeeds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSelectionPlanV1 {
    pub schema_version: String,
    pub routing_rules_version: String,
    pub proposal_md5: String,
    pub plan_hash: String,
    pub mode: ReviewRoutingMode,
    pub fingerprint: Vec<String>,
    pub selected_agents: Vec<SelectedAgent>,
    pub rejected_alternatives: Vec<RejectedAlternative>,
    pub ineligible_candidates: Vec<IneligibleCandidate>,
    pub warnings: Vec<String>,
    pub input_snapshot_hashes: InputSnapshotHashes,
}

/// A reviewer selected by the routing algorithm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedAgent {
    pub agent_id: String,
    pub routing_id: String,
    pub score: i64,
    pub mandatory: bool,
    pub override_source: Option<String>,
    pub score_terms: ScoreTerms,
    pub rationale: String,
    pub evidence_refs: Vec<RoutingEvidenceRef>,
    pub materialization_binding_id: String,
}

/// Score breakdown for a selected or rejected candidate.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScoreTerms {
    pub force_include: i64,
    pub stack_matches: i64,
    pub surface_matches: i64,
    pub risk_matches: i64,
    pub strong_keyword_matches: i64,
    pub repo_signal_matches: i64,
    pub cross_stack_dependency_matches: i64,
    pub baseline_gap_matches: i64,
    pub overlap_penalty: i64,
}

impl ScoreTerms {
    /// Compute the total score from the formula in P060 §5.
    pub fn total(&self) -> i64 {
        self.force_include * 5
            + self.stack_matches * 4
            + self.surface_matches * 3
            + self.risk_matches * 3
            + self.strong_keyword_matches * 2
            + self.repo_signal_matches * 2
            + self.cross_stack_dependency_matches * 2
            + self.baseline_gap_matches * 1
            - self.overlap_penalty * 3
    }
}

/// A candidate that scored but was not selected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectedAlternative {
    pub agent_id: String,
    pub routing_id: String,
    pub score: i64,
    pub reason: String,
    pub score_terms: ScoreTerms,
}

/// A candidate that was ineligible (disabled, wrong rollout wave, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IneligibleCandidate {
    pub agent_id: String,
    pub routing_id: String,
    pub reason: String,
}

/// Traceable evidence reference. Raw fields require
/// `operator_debug_routing_evidence` capability for readback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingEvidenceRef {
    pub evidence_id: String,
    pub evidence_type: String,
    pub hash: String,
    /// Raw value (redacted to None for unauthorized readers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<String>,
}

impl RoutingEvidenceRef {
    /// Return a hash-only projection (redact raw fields).
    pub fn redacted(&self) -> Self {
        Self {
            evidence_id: self.evidence_id.clone(),
            evidence_type: self.evidence_type.clone(),
            hash: self.hash.clone(),
            normalized_value: None,
            path: None,
            symbol: None,
            span: None,
        }
    }
}

/// Frozen hashes of all routing inputs for determinism verification.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InputSnapshotHashes {
    pub workflow_snapshot_hash: String,
    pub catalog_snapshot_hash: String,
    pub routing_metadata_hash: String,
    pub candidate_binding_hash: String,
    pub evidence_hash: String,
    pub override_hash: Option<String>,
}

// ── ReviewRoutingMode ────────────────────────────────────────────────

/// Routing mode for proposal review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRoutingMode {
    Dynamic,
    LegacyFixed,
    ShadowDynamic,
}

impl Default for ReviewRoutingMode {
    fn default() -> Self {
        Self::LegacyFixed
    }
}

impl std::fmt::Display for ReviewRoutingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dynamic => write!(f, "dynamic"),
            Self::LegacyFixed => write!(f, "legacy_fixed"),
            Self::ShadowDynamic => write!(f, "shadow_dynamic"),
        }
    }
}

impl std::str::FromStr for ReviewRoutingMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dynamic" => Ok(Self::Dynamic),
            "legacy_fixed" => Ok(Self::LegacyFixed),
            "shadow_dynamic" => Ok(Self::ShadowDynamic),
            other => Err(format!("Unknown ReviewRoutingMode: {other}")),
        }
    }
}

// ── ReviewRoutingOptions (RunStartOptionsV2.review_routing) ──────────

/// Plan-time routing overrides, frozen at run start.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReviewRoutingOptions {
    #[serde(default)]
    pub mode: ReviewRoutingMode,
    #[serde(default)]
    pub force_include: Vec<String>,
    #[serde(default)]
    pub force_exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

// ── CompiledDynamicAgentBinding ──────────────────────────────────────

/// Routing metadata for an agent catalog entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingMetadata {
    pub routing_id: String,
    pub family: String,
    pub capabilities: Vec<String>,
    pub stacks: Vec<String>,
    pub surfaces: Vec<String>,
    pub risks: Vec<String>,
    pub enabled_for_proposal_review: bool,
    pub rollout_wave: String,
    #[serde(default)]
    pub mandatory_when: Vec<String>,
    #[serde(default)]
    pub usually_pair_with: Vec<String>,
    #[serde(default)]
    pub close_alternatives: Vec<String>,
}

/// A compiled candidate binding for dynamic reviewer materialization.
/// Frozen at compilation time with the full resolved agent truth.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledDynamicAgentBinding {
    pub binding_id: String,
    pub agent_id: String,
    /// JSON-encoded `ResolvedAgent` snapshot.
    pub resolved_agent_snapshot_json: String,
    pub output_contracts: Vec<String>,
    pub permission_hash: String,
    pub worktree_hash: String,
    pub mcp_profile_hash: String,
    pub skill_hash: String,
    pub routing_metadata_hash: String,
    pub enabled_for_proposal_review: bool,
    pub rollout_wave: String,
    pub catalog_snapshot_hash: String,
    pub routing_metadata: RoutingMetadata,
}

// ── DynamicMaterializationRecord ─────────────────────────────────────

/// Tracks materialized reviewer executions for idempotency.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DynamicMaterializationRecord {
    pub id: DynamicMaterializationId,
    pub run_id: RunId,
    pub stage_id: String,
    pub attempt_id: i64,
    pub phase_id: String,
    pub plan_hash: String,
    pub binding_id: String,
    pub agent_execution_id: String,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

// ── ReviewCorpusBundleV2 ────────────────────────────────────────────

/// Additive fields for ReviewCorpusBundleV2 (P060 §6).
/// Extends the review corpus with selection-aware metadata so aggregation
/// consumers know which reviewer artifacts were produced by dynamically
/// selected reviewers vs. the legacy fixed quartet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewCorpusBundleV2 {
    /// Artifact paths/IDs for selected reviewer outputs.
    pub selected_review_artifacts: Vec<String>,
    /// Agent IDs of the selected reviewers (ordered by selection).
    pub selected_reviewer_ids: Vec<String>,
    /// Number of selected reviewers.
    pub reviewer_count: usize,
    /// plan_hash from the AgentSelectionPlanV1 that drove selection.
    pub selection_plan_hash: String,
    /// True when the run used legacy fixed mode (no dynamic routing).
    pub legacy_fixed_mode: bool,
}

// ── Routing validation errors ────────────────────────────────────────

/// Typed routing validation failure kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingFailureKind {
    OverrideConflict,
    MandatoryOverflow,
    DisabledRolloutWave,
    UnknownAgent,
    PlaceholderResolvedAgent,
    MalformedRoutingMetadata,
    MixedVersionSnapshot,
    MissingOutputContract,
}

impl std::fmt::Display for RoutingFailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OverrideConflict => write!(f, "override_conflict"),
            Self::MandatoryOverflow => write!(f, "mandatory_overflow"),
            Self::DisabledRolloutWave => write!(f, "disabled_rollout_wave"),
            Self::UnknownAgent => write!(f, "unknown_agent"),
            Self::PlaceholderResolvedAgent => write!(f, "placeholder_resolved_agent"),
            Self::MalformedRoutingMetadata => write!(f, "malformed_routing_metadata"),
            Self::MixedVersionSnapshot => write!(f, "mixed_version_snapshot"),
            Self::MissingOutputContract => write!(f, "missing_output_contract"),
        }
    }
}

impl std::str::FromStr for RoutingFailureKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "override_conflict" => Ok(Self::OverrideConflict),
            "mandatory_overflow" => Ok(Self::MandatoryOverflow),
            "disabled_rollout_wave" => Ok(Self::DisabledRolloutWave),
            "unknown_agent" => Ok(Self::UnknownAgent),
            "placeholder_resolved_agent" => Ok(Self::PlaceholderResolvedAgent),
            "malformed_routing_metadata" => Ok(Self::MalformedRoutingMetadata),
            "mixed_version_snapshot" => Ok(Self::MixedVersionSnapshot),
            "missing_output_contract" => Ok(Self::MissingOutputContract),
            other => Err(format!("Unknown RoutingFailureKind: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_terms_formula_matches_proposal() {
        let terms = ScoreTerms {
            force_include: 1,
            stack_matches: 2,
            surface_matches: 1,
            risk_matches: 1,
            strong_keyword_matches: 3,
            repo_signal_matches: 1,
            cross_stack_dependency_matches: 0,
            baseline_gap_matches: 1,
            overlap_penalty: 1,
        };
        // 1*5 + 2*4 + 1*3 + 1*3 + 3*2 + 1*2 + 0*2 + 1*1 - 1*3 = 5+8+3+3+6+2+0+1-3 = 25
        assert_eq!(terms.total(), 25);
    }

    #[test]
    fn routing_evidence_redacted_drops_raw_fields() {
        let evidence = RoutingEvidenceRef {
            evidence_id: "e1".into(),
            evidence_type: "keyword".into(),
            hash: "abc123".into(),
            normalized_value: Some("security".into()),
            path: Some("src/auth.rs".into()),
            symbol: Some("validate_token".into()),
            span: Some("10:15".into()),
        };
        let redacted = evidence.redacted();
        assert_eq!(redacted.evidence_id, "e1");
        assert_eq!(redacted.hash, "abc123");
        assert!(redacted.normalized_value.is_none());
        assert!(redacted.path.is_none());
        assert!(redacted.symbol.is_none());
        assert!(redacted.span.is_none());
    }

    #[test]
    fn system_execution_status_roundtrip() {
        for s in &[
            SystemExecutionStatus::Queued,
            SystemExecutionStatus::Running,
            SystemExecutionStatus::Succeeded,
            SystemExecutionStatus::Blocked,
            SystemExecutionStatus::Failed,
        ] {
            let s2: SystemExecutionStatus = s.to_string().parse().unwrap();
            assert_eq!(s, &s2);
        }
    }

    #[test]
    fn routing_mode_roundtrip() {
        for m in &[
            ReviewRoutingMode::Dynamic,
            ReviewRoutingMode::LegacyFixed,
            ReviewRoutingMode::ShadowDynamic,
        ] {
            let m2: ReviewRoutingMode = m.to_string().parse().unwrap();
            assert_eq!(m, &m2);
        }
    }

    #[test]
    fn routing_failure_kind_roundtrip() {
        for k in &[
            RoutingFailureKind::OverrideConflict,
            RoutingFailureKind::MandatoryOverflow,
            RoutingFailureKind::DisabledRolloutWave,
            RoutingFailureKind::UnknownAgent,
            RoutingFailureKind::PlaceholderResolvedAgent,
            RoutingFailureKind::MalformedRoutingMetadata,
            RoutingFailureKind::MixedVersionSnapshot,
            RoutingFailureKind::MissingOutputContract,
        ] {
            let k2: RoutingFailureKind = k.to_string().parse().unwrap();
            assert_eq!(k, &k2);
        }
    }

    #[test]
    fn review_routing_options_defaults() {
        let opts = ReviewRoutingOptions::default();
        assert_eq!(opts.mode, ReviewRoutingMode::LegacyFixed);
        assert!(opts.force_include.is_empty());
        assert!(opts.force_exclude.is_empty());
    }

    #[test]
    fn agent_selection_plan_serializes() {
        let plan = AgentSelectionPlanV1 {
            schema_version: "1".into(),
            routing_rules_version: "1".into(),
            proposal_md5: "abc".into(),
            plan_hash: "hash".into(),
            mode: ReviewRoutingMode::Dynamic,
            fingerprint: vec!["rust".into(), "security".into()],
            selected_agents: vec![SelectedAgent {
                agent_id: "proposal_reviewer_security".into(),
                routing_id: "security".into(),
                score: 15,
                mandatory: false,
                override_source: None,
                score_terms: ScoreTerms::default(),
                rationale: "security risk match".into(),
                evidence_refs: vec![],
                materialization_binding_id: "bind-1".into(),
            }],
            rejected_alternatives: vec![],
            ineligible_candidates: vec![],
            warnings: vec![],
            input_snapshot_hashes: InputSnapshotHashes::default(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        let round: AgentSelectionPlanV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(round.plan_hash, "hash");
        assert_eq!(round.selected_agents.len(), 1);
    }

    #[test]
    fn review_corpus_bundle_v2_serialization_roundtrip() {
        let bundle = ReviewCorpusBundleV2 {
            selected_review_artifacts: vec!["a1".into(), "a2".into(), "a3".into()],
            selected_reviewer_ids: vec![
                "proposal_reviewer_security".into(),
                "proposal_reviewer_rust_architect".into(),
                "proposal_reviewer_reliability".into(),
            ],
            reviewer_count: 3,
            selection_plan_hash: "abc123hash".into(),
            legacy_fixed_mode: false,
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let round: ReviewCorpusBundleV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(round.reviewer_count, 3);
        assert_eq!(round.selected_reviewer_ids.len(), 3);
        assert_eq!(round.selection_plan_hash, "abc123hash");
        assert!(!round.legacy_fixed_mode);
        assert_eq!(round.selected_review_artifacts.len(), 3);
    }

    #[test]
    fn review_corpus_bundle_v2_legacy_fixed_mode() {
        let bundle = ReviewCorpusBundleV2 {
            selected_review_artifacts: vec!["a1".into(), "a2".into(), "a3".into(), "a4".into()],
            selected_reviewer_ids: vec![
                "proposal_reviewer_product_owner".into(),
                "proposal_reviewer_ux".into(),
                "proposal_reviewer_ui".into(),
                "proposal_reviewer_architect".into(),
            ],
            reviewer_count: 4,
            selection_plan_hash: "legacy_hash".into(),
            legacy_fixed_mode: true,
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let round: ReviewCorpusBundleV2 = serde_json::from_str(&json).unwrap();
        assert!(round.legacy_fixed_mode);
        assert_eq!(round.reviewer_count, 4);
    }
}
