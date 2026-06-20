use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level status enum (api-contract-r2-001)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputContractRepairStatus {
    NotAttempted,
    InProgress,
    Recovered,
    Blocked,
    Skipped,
    Cancelled,
    Failed,
}

impl std::fmt::Display for OutputContractRepairStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NotAttempted => "not_attempted",
            Self::InProgress => "in_progress",
            Self::Recovered => "recovered",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for OutputContractRepairStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "not_attempted" => Ok(Self::NotAttempted),
            "in_progress" => Ok(Self::InProgress),
            "recovered" => Ok(Self::Recovered),
            "blocked" => Ok(Self::Blocked),
            "skipped" => Ok(Self::Skipped),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            other => Err(format!("Unknown OutputContractRepairStatus: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Presentation category (macos-r3-001)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationCategory {
    Informational,
    Recovered,
    Blocked,
    Skipped,
    Failed,
    Cancelled,
}

impl PresentationCategory {
    /// Normative projection from status and final settlement.
    pub fn from_status(status: &OutputContractRepairStatus) -> Self {
        match status {
            OutputContractRepairStatus::NotAttempted => Self::Informational,
            OutputContractRepairStatus::InProgress => Self::Informational,
            OutputContractRepairStatus::Recovered => Self::Recovered,
            OutputContractRepairStatus::Blocked => Self::Blocked,
            OutputContractRepairStatus::Skipped => Self::Skipped,
            OutputContractRepairStatus::Cancelled => Self::Cancelled,
            OutputContractRepairStatus::Failed => Self::Failed,
        }
    }
}

impl std::fmt::Display for PresentationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Informational => "informational",
            Self::Recovered => "recovered",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Initial failure classification
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialFailureClass {
    NoOutputProduced,
    EmptyOutput,
    MissingRequiredOutputs,
    InvalidRequiredOutputs,
    OutputContractMismatch,
    ProviderModeMismatch,
}

impl std::fmt::Display for InitialFailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NoOutputProduced => "no_output_produced",
            Self::EmptyOutput => "empty_output",
            Self::MissingRequiredOutputs => "missing_required_outputs",
            Self::InvalidRequiredOutputs => "invalid_required_outputs",
            Self::OutputContractMismatch => "output_contract_mismatch",
            Self::ProviderModeMismatch => "provider_mode_mismatch",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for InitialFailureClass {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "no_output_produced" => Ok(Self::NoOutputProduced),
            "empty_output" => Ok(Self::EmptyOutput),
            "missing_required_outputs" => Ok(Self::MissingRequiredOutputs),
            "invalid_required_outputs" => Ok(Self::InvalidRequiredOutputs),
            "output_contract_mismatch" => Ok(Self::OutputContractMismatch),
            "provider_mode_mismatch" => Ok(Self::ProviderModeMismatch),
            other => Err(format!("Unknown InitialFailureClass: {other}")),
        }
    }
}

/// Provider-mode mismatch subtypes (closed for v1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModeMismatchSubtype {
    PlanEventInsteadOfOutput,
    EmptySubmitAfterPlan,
    FilePlanWrittenInsteadOfPayload,
    RepairRepeatedPlanBehavior,
}

// ---------------------------------------------------------------------------
// Adapter and provider family
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterFamily {
    Codex,
    Claude,
    Gemini,
    Junie,
    Auggie,
    Fixture,
}

impl std::fmt::Display for AdapterFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Junie => "junie",
            Self::Auggie => "auggie",
            Self::Fixture => "fixture",
        };
        write!(f, "{s}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFamily {
    Codex,
    Claude,
    Gemini,
    Junie,
    Auggie,
    Fixture,
}

// ---------------------------------------------------------------------------
// Lease state machine
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    Reserved,
    PromptSent,
    Settled,
}

impl std::fmt::Display for LeaseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Reserved => "reserved",
            Self::PromptSent => "prompt_sent",
            Self::Settled => "settled",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for LeaseState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "reserved" => Ok(Self::Reserved),
            "prompt_sent" => Ok(Self::PromptSent),
            "settled" => Ok(Self::Settled),
            other => Err(format!("Unknown LeaseState: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseKind {
    Repair,
    Fallback,
}

impl std::fmt::Display for LeaseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repair => write!(f, "repair"),
            Self::Fallback => write!(f, "fallback"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseSettledResult {
    Accepted,
    RejectedInvalid,
    SkippedIneligible,
    Unavailable,
    FailedTransport,
    DeadlineExceeded,
    Cancelled,
    SupersededIgnored,
    LeaseContended,
    BudgetExhausted,
}

/// Full reclamation reason vocabulary per approved proposal.
/// Matches the CHECK constraint in migrations/079_p079_output_contract_repair.sql.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseReclamationReason {
    TtlExpiredReserved,
    TtlExpiredPromptSent,
    Cancellation,
    Supersession,
    PrincipalRevoked,
}

impl std::fmt::Display for LeaseReclamationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::TtlExpiredReserved => "ttl_expired_reserved",
            Self::TtlExpiredPromptSent => "ttl_expired_prompt_sent",
            Self::Cancellation => "cancellation",
            Self::Supersession => "supersession",
            Self::PrincipalRevoked => "principal_revoked",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for LeaseReclamationReason {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ttl_expired_reserved" => Ok(Self::TtlExpiredReserved),
            "ttl_expired_prompt_sent" => Ok(Self::TtlExpiredPromptSent),
            "cancellation" => Ok(Self::Cancellation),
            "supersession" => Ok(Self::Supersession),
            "principal_revoked" => Ok(Self::PrincipalRevoked),
            other => Err(format!("Unknown LeaseReclamationReason: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Same-session repair
// ---------------------------------------------------------------------------

/// Closed result enum for same-session repair (v1).
/// `Attempted` is not a closed-schema value; use the `result` field to distinguish.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SameSessionRepairResult {
    NotNeeded,
    Accepted,
    RejectedInvalid,
    Unavailable,
    SkippedIneligible,
    FailedTransport,
    BudgetExhausted,
    DeadlineExceeded,
    Cancelled,
    SupersededIgnored,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SameSessionRepair {
    pub result: SameSessionRepairResult,
    pub turn_count: u32,
    pub deadline_seconds: Option<u32>,
    pub reason: Option<String>,
    pub repair_attempt_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Transcript recovery
// ---------------------------------------------------------------------------

/// Closed result enum for transcript/envelope recovery (v1).
/// `oversized_payload` and `unattributable_envelope` are stored as subtypes
/// under `result = unavailable` per proposal sec-002.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptRecoveryResult {
    NotNeeded,
    Accepted,
    RejectedInvalid,
    SkippedIneligible,
    FailedTransport,
    Cancelled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverySource {
    Transcript,
    ProviderEnvelope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptRecovery {
    pub result: TranscriptRecoveryResult,
    /// Subtype for `unavailable` result: `oversized_payload` or `unattributable_envelope`.
    pub result_subtype: Option<String>,
    pub recovery_source: Option<RecoverySource>,
    pub bytes_examined: Option<u64>,
    pub max_recovery_payload_bytes: u64,
    pub max_json_depth: u32,
    pub max_chunks_examined: u32,
    pub recovery_parser_version: String,
}

impl Default for TranscriptRecovery {
    fn default() -> Self {
        Self {
            result: TranscriptRecoveryResult::NotNeeded,
            result_subtype: None,
            recovery_source: None,
            bytes_examined: None,
            max_recovery_payload_bytes: 262144, // 256 KiB
            max_json_depth: 32,
            max_chunks_examined: 64,
            recovery_parser_version: "p079_recovery_v1".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider fallback
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFallbackResult {
    NotNeeded,
    Scheduled,
    Accepted,
    RejectedInvalid,
    Unavailable,
    SkippedIneligible,
    FailedTransport,
    DeadlineExceeded,
    Cancelled,
    BudgetExhausted,
    LeaseContended,
    SupersededIgnored,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderFallback {
    pub result: ProviderFallbackResult,
    pub fallback_profile: Option<String>,
    pub fallback_agent_execution_id: Option<String>,
    pub parent_failed_agent_execution_id: Option<String>,
    pub fallback_packet_hash: Option<String>,
    pub fallback_principal_id: Option<String>,
    pub fallback_principal_capability_hash: Option<String>,
    pub deadline_seconds: Option<u32>,
}

// ---------------------------------------------------------------------------
// Provider plan evidence
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderPlanEvidence {
    /// Run-meta-root-relative paths only.
    pub paths: Vec<String>,
    /// Redaction markers applied, e.g. `["[redacted:abs_path]", "[redacted:credential]"]`.
    pub redactions_applied: Vec<String>,
    pub truncated_at_cap: bool,
    pub accepted_as_output: bool,
}

// ---------------------------------------------------------------------------
// Required output binding (closed v1 schema: name, contract_id, canonical_path)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequiredOutputBinding {
    /// Logical output name as declared in the agent contract.
    pub name: String,
    pub contract_id: String,
    /// Absolute frozen canonical path from RunPlanSnapshot.
    pub canonical_path: String,
}

// ---------------------------------------------------------------------------
// Permission decisions
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecisionValue {
    Allowed,
    Denied,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionResourceKind {
    FsWriteCanonicalOutputPath,
    FsWriteOther,
    FsRead,
    Shell,
    Network,
    ToolCustom,
    ToolMcp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub method: String,
    pub resource_kind: PermissionResourceKind,
    pub decision: PermissionDecisionValue,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Lease value object (for readback)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputContractRepairLease {
    pub key: String,
    pub kind: LeaseKind,
    pub state: LeaseState,
    pub settled_result: Option<LeaseSettledResult>,
    pub reclamation_reason: Option<LeaseReclamationReason>,
    pub idempotency_token: String,
    pub owner_principal_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub lease_seconds: u32,
    /// Set when the lease transitions to prompt_sent (REL-r2-1 durable-commit invariant).
    pub dispatch_committed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Budget (v1: max 1 repair + 1 fallback per invocation)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputContractBudget {
    pub repair_consumed: bool,
    pub fallback_consumed: bool,
    /// Maximum repair attempts per invocation (always 1 in v1).
    pub repair_max_per_invocation: u32,
    /// Maximum fallback attempts per invocation (always 1 in v1).
    pub fallback_max_per_invocation: u32,
}

impl Default for OutputContractBudget {
    fn default() -> Self {
        Self {
            repair_consumed: false,
            fallback_consumed: false,
            repair_max_per_invocation: 1,
            fallback_max_per_invocation: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level evidence record (output_contract_repair.v1)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputContractRepairEvidence {
    pub schema_version: String,
    pub repair_attempt_id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub session_generation_id: String,
    pub role: String,
    pub provider_family: ProviderFamily,
    pub adapter_family: AdapterFamily,
    pub required_output_mode: String,
    pub initial_failure_class: InitialFailureClass,
    pub initial_failure_subtype: Option<String>,
    pub status: OutputContractRepairStatus,
    pub presentation_category: PresentationCategory,
    /// Required on every v1 row; closed vocabulary from the proposal.
    pub recommended_next_action: String,
    pub required_outputs: Vec<RequiredOutputBinding>,
    pub same_session_repair: Option<SameSessionRepair>,
    pub transcript_recovery: Option<TranscriptRecovery>,
    pub provider_fallback: Option<ProviderFallback>,
    pub provider_plan_evidence: Option<ProviderPlanEvidence>,
    pub permission_decisions: Vec<PermissionDecision>,
    pub budget: OutputContractBudget,
    pub lease: Option<OutputContractRepairLease>,
    pub policy_feature_flags: Vec<String>,
    pub repair_prompt_template_version: Option<String>,
    pub final_output_settlement: Option<String>,
    pub evidence_artifact_path: Option<String>,
    pub evidence_version: u64,
    pub projection_integrity: String,
    pub projection_stale_since: Option<DateTime<Utc>>,
    /// When the repair evidence was first recorded (RFC3339). Required by the approved v1 schema.
    pub recorded_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OutputContractRepairEvidence {
    pub const SCHEMA_VERSION: &'static str = "output_contract_repair.v1";
    pub const REPAIR_PROMPT_TEMPLATE_VERSION: &'static str = "p079_repair_v1";
    pub const RECOVERY_PARSER_VERSION: &'static str = "p079_recovery_v1";
}

// ---------------------------------------------------------------------------
// Fallback context packet (sec-001)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputContractFallbackPacket {
    pub schema_version: String,
    pub validation_failure_class: String,
    pub validation_failure_subtype: Option<String>,
    pub required_output_names: Vec<String>,
    pub required_output_contract_ids: Vec<String>,
    /// Run-meta-root-relative paths only.
    pub required_output_canonical_paths_relative: Vec<String>,
    pub prior_attempt_summary: String,
    pub repair_prompt_template_version: String,
    pub recovery_parser_version: String,
}

impl OutputContractFallbackPacket {
    pub const SCHEMA_VERSION: &'static str = "output_contract_repair_fallback_packet.v1";
    /// Total serialized packet size cap: 32 KiB.
    pub const MAX_PACKET_BYTES: usize = 32 * 1024;
    /// Per-string cap: 4 KiB.
    pub const MAX_STRING_BYTES: usize = 4 * 1024;
}

// ---------------------------------------------------------------------------
// Lease record for DB layer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OutputContractRepairLeaseRow {
    pub lease_key: String,
    pub schema_version: String,
    pub repair_event_id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub parent_agent_execution_id: String,
    pub lease_kind: LeaseKind,
    pub lease_state: LeaseState,
    pub settled_result: Option<String>,
    pub reclamation_reason: Option<String>,
    pub frozen_fallback_policy_hash: Option<String>,
    pub idempotency_token: String,
    pub lease_owner_principal_id: String,
    pub lease_acquired_at: String,
    pub lease_expires_at: String,
    pub lease_seconds: i64,
    /// Set when reserved->prompt_sent commits (REL-r2-1 invariant).
    pub dispatch_committed_at: Option<String>,
    /// Optimistic-locking version for CAS reclamation.
    pub version: i64,
    pub infra_retry_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Fallback parent link record for DB layer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct FallbackParentLink {
    /// PK: the fallback child agent execution id (approved sqlite_migration_appendix naming).
    pub fallback_agent_execution_id: String,
    /// The parent agent execution that failed output validation and triggered fallback.
    pub parent_failed_agent_execution_id: String,
    pub repair_event_id: String,
    pub lease_key: String,
    pub fallback_packet_hash: String,
    pub fallback_principal_id: String,
    pub fallback_principal_capability_hash: String,
    pub fallback_result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Repair event row for DB layer (PK = repair_attempt_id per sqlite appendix)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OutputContractRepairEventRow {
    /// Primary key. Random v4 UUID minted at event creation (approved appendix PK).
    pub repair_attempt_id: String,
    pub schema_version: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub session_generation_id: String,
    pub role: String,
    pub provider_family: String,
    pub adapter_family: String,
    pub required_output_mode: String,
    pub initial_failure_class: String,
    pub initial_failure_subtype: Option<String>,
    pub status: String,
    pub presentation_category: String,
    pub recommended_next_action: String,
    pub final_output_settlement: Option<String>,
    pub same_session_repair_json: Option<String>,
    pub transcript_recovery_json: Option<String>,
    pub provider_fallback_json: Option<String>,
    pub provider_plan_evidence_json: Option<String>,
    pub required_outputs_json: String,
    pub permission_decisions_json: String,
    pub repair_budget_consumed: bool,
    pub fallback_budget_consumed: bool,
    pub repair_prompt_template_version: Option<String>,
    pub recovery_parser_version: Option<String>,
    pub policy_feature_flags_json: String,
    pub evidence_artifact_path: Option<String>,
    pub lease_id: Option<String>,
    pub evidence_version: i64,
    pub projection_integrity: String,
    pub projection_stale_since: Option<String>,
    pub projection_schema_version: String,
    pub projection_rebuild_attempts: i64,
    pub recorded_at: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Recommended next actions (approved closed vocabulary)
// ---------------------------------------------------------------------------

pub const RECOMMENDED_NEXT_ACTION_CONTINUE: &str = "continue";
pub const RECOMMENDED_NEXT_ACTION_INSPECT_REPAIR_EVIDENCE: &str = "inspect_repair_evidence";
pub const RECOMMENDED_NEXT_ACTION_CONFIGURE_FALLBACK_POLICY: &str = "configure_fallback_policy";
pub const RECOMMENDED_NEXT_ACTION_OPERATOR_RESOLVE_APPROVAL: &str = "operator_resolve_approval";
pub const RECOMMENDED_NEXT_ACTION_OPERATOR_RESOLVE_WORKFLOW_CONFLICT: &str =
    "operator_resolve_workflow_conflict";
pub const RECOMMENDED_NEXT_ACTION_RETRY_AFTER_TRANSPORT_RESTORED: &str =
    "retry_after_transport_restored";
pub const RECOMMENDED_NEXT_ACTION_CANCEL_ACKNOWLEDGED: &str = "cancel_acknowledged";
pub const RECOMMENDED_NEXT_ACTION_MANUAL_INVESTIGATION: &str = "manual_investigation";

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// System principal for engine-initiated repairs (SEC-HIGH-003)
// ---------------------------------------------------------------------------

/// Named system principal used when the repair lane is triggered internally by the engine
/// executor without an authenticated user caller_principal_id in the work-item payload.
/// This sentinel is intentionally distinct from any user principal — it is auditable,
/// revocable (by disabling the P079 feature flag), and never confused with a real user.
/// Do NOT use `unknown_principal` in place of this constant.
pub const ENGINE_REPAIR_SYSTEM_PRINCIPAL: &str = "system:engine:p079_repair_v1";

// Feature flag names
// ---------------------------------------------------------------------------

pub const FLAG_OUTPUT_REPAIR_ENABLED: &str = "CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED";
pub const FLAG_TRANSCRIPT_RECOVERY_ENABLED: &str = "CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED";
pub const FLAG_PROVIDER_FALLBACK_ENABLED: &str = "CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED";

// ---------------------------------------------------------------------------
// Budget constants
// ---------------------------------------------------------------------------

/// Default repair lease TTL: 180s (1.5× repair deadline).
pub const DEFAULT_REPAIR_LEASE_SECONDS: i64 = 180;
/// Default fallback lease TTL: 1200s (1.33× fallback deadline cap).
pub const DEFAULT_FALLBACK_LEASE_SECONDS: i64 = 1200;
/// Fallback deadline cap.
pub const FALLBACK_POLICY_MAX_CAP_SECONDS: u32 = 900;
/// Infrastructure retry max (pre-ACK).
pub const INFRA_RETRY_MAX: i64 = 2;
/// Infrastructure retry backoff ms.
pub const INFRA_RETRY_BACKOFF_MS: u64 = 250;
/// Max consecutive projection rebuild attempts before permanently_stale.
pub const PROJECTION_REBUILD_MAX_ATTEMPTS: u32 = 12;
/// Transcript recovery byte limit.
pub const MAX_RECOVERY_PAYLOAD_BYTES: usize = 256 * 1024;
/// Transcript recovery JSON depth.
pub const MAX_JSON_DEPTH: u32 = 32;
/// Transcript recovery chunk scan limit.
pub const MAX_CHUNKS_EXAMINED: u32 = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_079_presentation_category_projection() {
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::NotAttempted),
            PresentationCategory::Informational
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::InProgress),
            PresentationCategory::Informational
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::Recovered),
            PresentationCategory::Recovered
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::Blocked),
            PresentationCategory::Blocked
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::Skipped),
            PresentationCategory::Skipped
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::Cancelled),
            PresentationCategory::Cancelled
        );
        assert_eq!(
            PresentationCategory::from_status(&OutputContractRepairStatus::Failed),
            PresentationCategory::Failed
        );
    }

    #[test]
    fn proposal_079_status_roundtrip() {
        for s in &[
            "not_attempted",
            "in_progress",
            "recovered",
            "blocked",
            "skipped",
            "cancelled",
            "failed",
        ] {
            let parsed: OutputContractRepairStatus = s.parse().unwrap();
            assert_eq!(parsed.to_string(), *s);
        }
    }

    #[test]
    fn proposal_079_failure_class_roundtrip() {
        for s in &[
            "no_output_produced",
            "empty_output",
            "missing_required_outputs",
            "invalid_required_outputs",
            "output_contract_mismatch",
            "provider_mode_mismatch",
        ] {
            let parsed: InitialFailureClass = s.parse().unwrap();
            assert_eq!(parsed.to_string(), *s);
        }
    }

    #[test]
    fn proposal_079_schema_version_constants() {
        assert_eq!(
            OutputContractRepairEvidence::SCHEMA_VERSION,
            "output_contract_repair.v1"
        );
        assert_eq!(
            OutputContractFallbackPacket::SCHEMA_VERSION,
            "output_contract_repair_fallback_packet.v1"
        );
        assert!(OutputContractFallbackPacket::MAX_PACKET_BYTES == 32 * 1024);
    }

    #[test]
    fn proposal_079_required_output_binding_uses_name_field() {
        let b = RequiredOutputBinding {
            name: "my_output".into(),
            contract_id: "contract-001".into(),
            canonical_path: "/tmp/out.json".into(),
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(
            json.contains("\"name\""),
            "RequiredOutputBinding must use 'name' field"
        );
        assert!(
            !json.contains("\"output_name\""),
            "must not use 'output_name'"
        );
    }

    #[test]
    fn proposal_079_budget_has_max_fields() {
        let b = OutputContractBudget::default();
        assert_eq!(b.repair_max_per_invocation, 1);
        assert_eq!(b.fallback_max_per_invocation, 1);
    }

    #[test]
    fn proposal_079_transcript_recovery_result_not_needed() {
        let r = TranscriptRecoveryResult::NotNeeded;
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"not_needed\"");
    }

    #[test]
    fn proposal_079_same_session_repair_result_no_attempted() {
        // SameSessionRepairResult must not contain 'Attempted' (not in closed v1 schema).
        // Verify that valid values serialize correctly.
        let r = SameSessionRepairResult::NotNeeded;
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"not_needed\"");
        let r2 = SameSessionRepairResult::SupersededIgnored;
        let json2 = serde_json::to_string(&r2).unwrap();
        assert_eq!(json2, "\"superseded_ignored\"");
    }

    #[test]
    fn proposal_079_event_row_uses_repair_attempt_id() {
        // Verify that the DB row struct uses repair_attempt_id (not id) as the PK field.
        let now = "2026-05-30T10:00:00Z";
        let row = OutputContractRepairEventRow {
            repair_attempt_id: "rep-001".into(),
            schema_version: "output_contract_repair.v1".into(),
            run_id: "run-001".into(),
            stage_execution_id: "stage-001".into(),
            agent_execution_id: "agent-001".into(),
            session_generation_id: "session-001".into(),
            role: "lead_orchestrator".into(),
            provider_family: "claude".into(),
            adapter_family: "claude".into(),
            required_output_mode: "strict_structured".into(),
            initial_failure_class: "missing_required_outputs".into(),
            initial_failure_subtype: None,
            status: "in_progress".into(),
            presentation_category: "informational".into(),
            recommended_next_action: "continue".into(),
            final_output_settlement: None,
            same_session_repair_json: None,
            transcript_recovery_json: None,
            provider_fallback_json: None,
            provider_plan_evidence_json: None,
            required_outputs_json: "[]".into(),
            permission_decisions_json: "[]".into(),
            repair_budget_consumed: false,
            fallback_budget_consumed: false,
            repair_prompt_template_version: Some("p079_repair_v1".into()),
            recovery_parser_version: Some("p079_recovery_v1".into()),
            policy_feature_flags_json: "[]".into(),
            evidence_artifact_path: None,
            lease_id: None,
            evidence_version: 1,
            projection_integrity: "fresh".into(),
            projection_stale_since: None,
            projection_schema_version: "output_contract_repair_events_v1".into(),
            projection_rebuild_attempts: 0,
            recorded_at: now.into(),
            created_at: now.into(),
            updated_at: now.into(),
        };
        assert_eq!(row.repair_attempt_id, "rep-001");
        assert_eq!(row.recommended_next_action, "continue");
        assert_eq!(row.presentation_category, "informational");
    }
}
