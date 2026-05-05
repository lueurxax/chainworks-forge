// P066: Full typed struct for ToolchainMappingDiagnosticsV1 schema (payload_schema_v1).
//
// Two distinct failure kinds:
// - DiagMappingState::SetupFailed: directory preparation failed within 2000 ms deadline.
// - DiagMappingState::QueueTimeout: per-run Xcode lease wait exceeded deadline.
//
// Seven sentinel constructors map to all 7 documented mapping states.

use serde::{Deserialize, Serialize};

pub const TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION: i64 = 1;
pub const TOOLCHAIN_SETUP_DEADLINE_MS: i64 = 2_000;
pub const TOOLCHAIN_XCODE_QUEUE_DEADLINE_MS: i64 = 300_000;

const CLEANUP_SURFACES: &[&str] = &[
    "startupRecoverySummary.toolchainCache",
    "toolchainCacheHousekeepingSummary",
];

/// P066: Schema v1 diagnostics document for toolchain cache mapping.
/// Stored as actual_toolchain_mapping_diagnostics_json on agent_executions.
/// All seven mapping states are representable; absolute paths are never included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolchainMappingDiagnosticsV1 {
    pub version: i64,
    pub mapping_state: DiagMappingState,
    pub mapping_enabled: bool,
    #[serde(default)]
    pub mapping_families: Vec<String>,
    pub inactive_reason: Option<DiagInactiveReason>,
    pub policy_source: DiagPolicySource,
    pub policy_version: Option<i64>,
    pub provider_family: String,
    pub redaction_mode: DiagRedactionMode,
    pub setup: DiagSetupState,
    pub concurrency: DiagConcurrencyState,
    pub cleanup: DiagCleanupState,
    #[serde(default)]
    pub family_results: Vec<DiagFamilyResult>,
}

impl ToolchainMappingDiagnosticsV1 {
    /// Synthesize a legacy_row_unavailable sentinel for pre-migration NULL rows.
    /// policy_source=synthesized_legacy; inactive_reason=legacy_row.
    pub fn legacy_row_unavailable() -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::LegacyRowUnavailable,
            mapping_enabled: false,
            mapping_families: vec![],
            inactive_reason: Some(DiagInactiveReason::LegacyRow),
            policy_source: DiagPolicySource::SynthesizedLegacy,
            policy_version: None,
            provider_family: "unknown".to_string(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::not_evaluated(),
            concurrency: DiagConcurrencyState::not_applicable_legacy(),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// disabled_by_policy: policy.enabled=false.
    /// policy_source=runplan_snapshot; inactive_reason=policy_disabled.
    pub fn disabled_by_policy(provider_family: impl Into<String>, policy_version: i64) -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::DisabledByPolicy,
            mapping_enabled: false,
            mapping_families: vec![],
            inactive_reason: Some(DiagInactiveReason::PolicyDisabled),
            policy_source: DiagPolicySource::RunplanSnapshot,
            policy_version: Some(policy_version),
            provider_family: provider_family.into(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::not_evaluated(),
            concurrency: DiagConcurrencyState::not_applicable(),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// policy_absent: toolchain_cache_policy block missing from the agent's run-plan snapshot.
    /// policy_source=runplan_snapshot; inactive_reason=policy_absent.
    pub fn policy_absent(provider_family: impl Into<String>) -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::PolicyAbsent,
            mapping_enabled: false,
            mapping_families: vec![],
            inactive_reason: Some(DiagInactiveReason::PolicyAbsent),
            policy_source: DiagPolicySource::RunplanSnapshot,
            policy_version: None,
            provider_family: provider_family.into(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::not_evaluated(),
            concurrency: DiagConcurrencyState::not_applicable(),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// unsupported_family: provider family has no P066 mapping support.
    /// policy_source=runplan_snapshot; inactive_reason=unsupported_family.
    pub fn unsupported_family(provider_family: impl Into<String>) -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::UnsupportedFamily,
            mapping_enabled: false,
            mapping_families: vec![],
            inactive_reason: Some(DiagInactiveReason::UnsupportedFamily),
            policy_source: DiagPolicySource::RunplanSnapshot,
            policy_version: None,
            provider_family: provider_family.into(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::not_evaluated(),
            concurrency: DiagConcurrencyState::not_applicable(),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// setup_failed: directory preparation or validation failed within 2000 ms deadline.
    /// Diagnostics MUST be emitted even on setup failure (proposal §failure_contract).
    pub fn setup_failed(
        provider_family: impl Into<String>,
        policy_version: i64,
        failure_kind: impl Into<String>,
        failure_reason: impl Into<String>,
    ) -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::SetupFailed,
            mapping_enabled: true,
            mapping_families: vec![],
            inactive_reason: None,
            policy_source: DiagPolicySource::RunplanSnapshot,
            policy_version: Some(policy_version),
            provider_family: provider_family.into(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::failed(failure_kind, failure_reason),
            concurrency: DiagConcurrencyState::not_applicable(),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// queue_timeout: xcode_run_scope_queue_timeout (per-run Xcode lease wait exceeded).
    /// MUST NOT increment mapping_setup_latency_p95_ms (separate from setup failure).
    pub fn queue_timeout(
        provider_family: impl Into<String>,
        policy_version: i64,
        wait_ms: i64,
        deadline_ms: i64,
    ) -> Self {
        Self {
            version: TOOLCHAIN_DIAGNOSTICS_SCHEMA_VERSION,
            mapping_state: DiagMappingState::QueueTimeout,
            mapping_enabled: true,
            mapping_families: vec!["xcode".to_string()],
            inactive_reason: None,
            policy_source: DiagPolicySource::RunplanSnapshot,
            policy_version: Some(policy_version),
            provider_family: provider_family.into(),
            redaction_mode: DiagRedactionMode::ToolchainHomeRelative,
            setup: DiagSetupState::not_evaluated(),
            concurrency: DiagConcurrencyState::queue_timeout(wait_ms, deadline_ms),
            cleanup: DiagCleanupState::none(),
            family_results: vec![],
        }
    }

    /// Serialize to JSON for storage in actual_toolchain_mapping_diagnostics_json.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from stored JSON; returns legacy_row_unavailable sentinel on NULL or parse error.
    pub fn from_json_or_legacy(json: Option<&str>) -> Self {
        match json {
            None => Self::legacy_row_unavailable(),
            Some(s) => serde_json::from_str(s).unwrap_or_else(|_| Self::legacy_row_unavailable()),
        }
    }

    /// DEC-003 invariant: policy_source is always runplan_snapshot or synthesized_legacy.
    pub fn has_valid_policy_source(&self) -> bool {
        matches!(
            self.policy_source,
            DiagPolicySource::RunplanSnapshot | DiagPolicySource::SynthesizedLegacy
        )
    }
}

// ── Enums ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagMappingState {
    Active,
    DisabledByPolicy,
    PolicyAbsent,
    UnsupportedFamily,
    SetupFailed,
    QueueTimeout,
    LegacyRowUnavailable,
}

impl DiagMappingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::DisabledByPolicy => "disabled_by_policy",
            Self::PolicyAbsent => "policy_absent",
            Self::UnsupportedFamily => "unsupported_family",
            Self::SetupFailed => "setup_failed",
            Self::QueueTimeout => "queue_timeout",
            Self::LegacyRowUnavailable => "legacy_row_unavailable",
        }
    }
}

impl std::fmt::Display for DiagMappingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagInactiveReason {
    PolicyDisabled,
    PolicyAbsent,
    UnsupportedFamily,
    LegacyRow,
}

impl DiagInactiveReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PolicyDisabled => "policy_disabled",
            Self::PolicyAbsent => "policy_absent",
            Self::UnsupportedFamily => "unsupported_family",
            Self::LegacyRow => "legacy_row",
        }
    }
}

/// DEC-003: policy_source is always runplan_snapshot or synthesized_legacy.
/// Live agent_catalog is never an authoritative policy source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagPolicySource {
    RunplanSnapshot,
    SynthesizedLegacy,
}

impl DiagPolicySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RunplanSnapshot => "runplan_snapshot",
            Self::SynthesizedLegacy => "synthesized_legacy",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagRedactionMode {
    ToolchainHomeRelative,
}

impl Default for DiagRedactionMode {
    fn default() -> Self {
        Self::ToolchainHomeRelative
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagSetupState {
    pub deadline_ms: i64,
    pub duration_ms: Option<i64>,
    pub status: DiagSetupStatus,
    pub failure_kind: Option<String>,
    pub failure_reason: Option<String>,
}

impl DiagSetupState {
    pub fn not_evaluated() -> Self {
        Self {
            deadline_ms: TOOLCHAIN_SETUP_DEADLINE_MS,
            duration_ms: None,
            status: DiagSetupStatus::NotEvaluated,
            failure_kind: None,
            failure_reason: None,
        }
    }

    pub fn prepared(duration_ms: i64) -> Self {
        Self {
            deadline_ms: TOOLCHAIN_SETUP_DEADLINE_MS,
            duration_ms: Some(duration_ms),
            status: DiagSetupStatus::Prepared,
            failure_kind: None,
            failure_reason: None,
        }
    }

    pub fn failed(failure_kind: impl Into<String>, failure_reason: impl Into<String>) -> Self {
        Self {
            deadline_ms: TOOLCHAIN_SETUP_DEADLINE_MS,
            duration_ms: None,
            status: DiagSetupStatus::Failed,
            failure_kind: Some(failure_kind.into()),
            failure_reason: Some(failure_reason.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagSetupStatus {
    NotEvaluated,
    Prepared,
    Failed,
}

// ── Concurrency ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagConcurrencyState {
    pub mode: DiagConcurrencyMode,
    pub status: DiagConcurrencyStatus,
    pub wait_ms: Option<i64>,
    pub wait_deadline_ms: Option<i64>,
    pub lease_key_kind: Option<String>,
    pub diagnostic_note: Option<String>,
}

impl DiagConcurrencyState {
    /// No concurrency context — used for non-Xcode families.
    pub fn not_applicable() -> Self {
        Self {
            mode: DiagConcurrencyMode::None,
            status: DiagConcurrencyStatus::NotApplicable,
            wait_ms: None,
            wait_deadline_ms: None,
            lease_key_kind: None,
            diagnostic_note: None,
        }
    }

    /// Legacy sentinel — historical row predates the diagnostics column.
    pub fn not_applicable_legacy() -> Self {
        Self {
            mode: DiagConcurrencyMode::None,
            status: DiagConcurrencyStatus::NotApplicable,
            wait_ms: None,
            wait_deadline_ms: None,
            lease_key_kind: None,
            diagnostic_note: Some(
                "Historical row predates actual_toolchain_mapping_diagnostics_json.".to_string(),
            ),
        }
    }

    pub fn acquired_immediately(wait_ms: i64) -> Self {
        Self {
            mode: DiagConcurrencyMode::SerializedPerRun,
            status: DiagConcurrencyStatus::AcquiredImmediately,
            wait_ms: Some(wait_ms),
            wait_deadline_ms: Some(TOOLCHAIN_XCODE_QUEUE_DEADLINE_MS),
            lease_key_kind: Some("run_id".to_string()),
            diagnostic_note: None,
        }
    }

    pub fn queued(wait_ms: i64, wait_deadline_ms: i64) -> Self {
        Self {
            mode: DiagConcurrencyMode::SerializedPerRun,
            status: DiagConcurrencyStatus::Queued,
            wait_ms: Some(wait_ms),
            wait_deadline_ms: Some(wait_deadline_ms),
            lease_key_kind: Some("run_id".to_string()),
            diagnostic_note: Some(
                "Queued behind same-run Xcode work; build time is reported separately.".to_string(),
            ),
        }
    }

    pub fn queue_timeout(wait_ms: i64, deadline_ms: i64) -> Self {
        Self {
            mode: DiagConcurrencyMode::SerializedPerRun,
            status: DiagConcurrencyStatus::QueueTimeout,
            wait_ms: Some(wait_ms),
            wait_deadline_ms: Some(deadline_ms),
            lease_key_kind: Some("run_id".to_string()),
            diagnostic_note: None,
        }
    }

    pub fn cancelled_before_acquire() -> Self {
        Self {
            mode: DiagConcurrencyMode::SerializedPerRun,
            status: DiagConcurrencyStatus::CancelledBeforeAcquire,
            wait_ms: None,
            wait_deadline_ms: Some(TOOLCHAIN_XCODE_QUEUE_DEADLINE_MS),
            lease_key_kind: Some("run_id".to_string()),
            diagnostic_note: None,
        }
    }

    pub fn root_quarantined_before_reuse() -> Self {
        Self {
            mode: DiagConcurrencyMode::SerializedPerRun,
            status: DiagConcurrencyStatus::RootQuarantinedBeforeReuse,
            wait_ms: None,
            wait_deadline_ms: Some(TOOLCHAIN_XCODE_QUEUE_DEADLINE_MS),
            lease_key_kind: Some("run_id".to_string()),
            diagnostic_note: Some(
                "Crash restart: existing root quarantined; fresh root created for this execution."
                    .to_string(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagConcurrencyMode {
    None,
    SerializedPerRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagConcurrencyStatus {
    NotApplicable,
    AcquiredImmediately,
    Queued,
    QueueTimeout,
    CancelledBeforeAcquire,
    RootQuarantinedBeforeReuse,
}

// ── Cleanup ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagCleanupState {
    pub owner: DiagCleanupOwner,
    pub plan: DiagCleanupPlan,
    #[serde(default)]
    pub aggregate_outcome_surfaces: Vec<String>,
}

impl DiagCleanupState {
    pub fn none() -> Self {
        Self {
            owner: DiagCleanupOwner::None,
            plan: DiagCleanupPlan::None,
            aggregate_outcome_surfaces: vec![],
        }
    }

    pub fn delete_on_close() -> Self {
        Self {
            owner: DiagCleanupOwner::AcpSessionClose,
            plan: DiagCleanupPlan::DeleteOnClose,
            aggregate_outcome_surfaces: CLEANUP_SURFACES.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn defer_until_terminal_prune() -> Self {
        Self {
            owner: DiagCleanupOwner::GeneratedStateHousekeeping,
            plan: DiagCleanupPlan::DeferUntilTerminalPrune,
            aggregate_outcome_surfaces: CLEANUP_SURFACES.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn quarantine_then_recreate() -> Self {
        Self {
            owner: DiagCleanupOwner::StartupRecoverySweep,
            plan: DiagCleanupPlan::QuarantineThenRecreate,
            aggregate_outcome_surfaces: CLEANUP_SURFACES.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagCleanupOwner {
    None,
    AcpSessionClose,
    StartupRecoverySweep,
    GeneratedStateHousekeeping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagCleanupPlan {
    None,
    DeleteOnClose,
    ReclaimAfterRestart,
    QuarantineThenRecreate,
    DeferUntilTerminalPrune,
}

// ── Family results ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagFamilyResult {
    pub family: String,
    pub effective_scope: Option<String>,
    pub requested_scope: Option<String>,
    pub scope_key_kind: Option<String>,
    pub path_mode: DiagPathMode,
    pub relative_root_suffix: String,
    #[serde(default)]
    pub created_directories: Vec<String>,
    #[serde(default)]
    pub unsupported_mappings: Vec<String>,
    #[serde(default)]
    pub validation_failures: Vec<String>,
    pub setup_failure_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagPathMode {
    Disabled,
    ArgumentsAndEnv,
    EnvironmentOnly,
    BestEffort,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p066_diag_legacy_row_unavailable_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::legacy_row_unavailable();
        assert_eq!(d.version, 1);
        assert_eq!(d.mapping_state, DiagMappingState::LegacyRowUnavailable);
        assert!(!d.mapping_enabled);
        assert_eq!(d.inactive_reason, Some(DiagInactiveReason::LegacyRow));
        assert_eq!(d.policy_source, DiagPolicySource::SynthesizedLegacy);
        assert!(d.policy_version.is_none());
        assert_eq!(d.provider_family, "unknown");
        assert!(d.mapping_families.is_empty());
        assert!(d.family_results.is_empty());
        assert!(d.has_valid_policy_source());
    }

    #[test]
    fn p066_diag_disabled_by_policy_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::disabled_by_policy("claude", 1);
        assert_eq!(d.mapping_state, DiagMappingState::DisabledByPolicy);
        assert!(!d.mapping_enabled);
        assert_eq!(d.inactive_reason, Some(DiagInactiveReason::PolicyDisabled));
        assert_eq!(d.policy_source, DiagPolicySource::RunplanSnapshot);
        assert_eq!(d.policy_version, Some(1));
        assert_eq!(d.provider_family, "claude");
        assert!(d.has_valid_policy_source());
    }

    #[test]
    fn p066_diag_policy_absent_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::policy_absent("gemini");
        assert_eq!(d.mapping_state, DiagMappingState::PolicyAbsent);
        assert!(!d.mapping_enabled);
        assert_eq!(d.inactive_reason, Some(DiagInactiveReason::PolicyAbsent));
        assert_eq!(d.policy_source, DiagPolicySource::RunplanSnapshot);
        assert!(d.policy_version.is_none());
        assert_eq!(d.provider_family, "gemini");
    }

    #[test]
    fn p066_diag_unsupported_family_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::unsupported_family("auggie");
        assert_eq!(d.mapping_state, DiagMappingState::UnsupportedFamily);
        assert!(!d.mapping_enabled);
        assert_eq!(
            d.inactive_reason,
            Some(DiagInactiveReason::UnsupportedFamily)
        );
        assert_eq!(d.policy_source, DiagPolicySource::RunplanSnapshot);
    }

    #[test]
    fn p066_diag_setup_failed_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::setup_failed(
            "xcode",
            1,
            "toolchain_mapping_setup_failed",
            "mapping_setup_disk_full",
        );
        assert_eq!(d.mapping_state, DiagMappingState::SetupFailed);
        assert!(
            d.mapping_enabled,
            "setup_failed must have mapping_enabled=true (attempted)"
        );
        assert!(
            d.inactive_reason.is_none(),
            "setup_failed has no inactive_reason"
        );
        assert_eq!(d.setup.status, DiagSetupStatus::Failed);
        assert_eq!(
            d.setup.failure_kind.as_deref(),
            Some("toolchain_mapping_setup_failed")
        );
        assert_eq!(
            d.setup.failure_reason.as_deref(),
            Some("mapping_setup_disk_full")
        );
        assert_eq!(d.policy_source, DiagPolicySource::RunplanSnapshot);
    }

    #[test]
    fn p066_diag_queue_timeout_sentinel_fields() {
        let d = ToolchainMappingDiagnosticsV1::queue_timeout("xcode", 1, 300_001, 300_000);
        assert_eq!(d.mapping_state, DiagMappingState::QueueTimeout);
        assert!(d.mapping_enabled);
        assert!(d.inactive_reason.is_none());
        assert_eq!(d.concurrency.status, DiagConcurrencyStatus::QueueTimeout);
        assert_eq!(d.concurrency.wait_ms, Some(300_001));
        assert_eq!(d.concurrency.wait_deadline_ms, Some(300_000));
        // queue_timeout must NOT be counted as setup_failed
        assert_ne!(d.mapping_state, DiagMappingState::SetupFailed);
        assert_eq!(d.setup.status, DiagSetupStatus::NotEvaluated);
    }

    #[test]
    fn p066_diag_all_states_have_valid_policy_source() {
        let cases: Vec<ToolchainMappingDiagnosticsV1> = vec![
            ToolchainMappingDiagnosticsV1::legacy_row_unavailable(),
            ToolchainMappingDiagnosticsV1::disabled_by_policy("claude", 1),
            ToolchainMappingDiagnosticsV1::policy_absent("claude"),
            ToolchainMappingDiagnosticsV1::unsupported_family("auggie"),
            ToolchainMappingDiagnosticsV1::setup_failed(
                "xcode",
                1,
                "toolchain_mapping_setup_failed",
                "mapping_setup_path_escape",
            ),
            ToolchainMappingDiagnosticsV1::queue_timeout("xcode", 1, 50_000, 300_000),
        ];
        for case in &cases {
            assert!(
                case.has_valid_policy_source(),
                "policy_source must be runplan_snapshot or synthesized_legacy, got {:?} for state {:?}",
                case.policy_source,
                case.mapping_state
            );
        }
    }

    #[test]
    fn p066_diag_setup_and_queue_timeout_are_distinct_states() {
        let setup = ToolchainMappingDiagnosticsV1::setup_failed(
            "xcode",
            1,
            "toolchain_mapping_setup_failed",
            "mapping_setup_timeout",
        );
        let queue = ToolchainMappingDiagnosticsV1::queue_timeout("xcode", 1, 300_000, 300_000);
        assert_ne!(
            setup.mapping_state, queue.mapping_state,
            "setup_failed and queue_timeout must be distinct states"
        );
    }

    #[test]
    fn p066_diag_roundtrip_all_sentinels() {
        let cases: Vec<ToolchainMappingDiagnosticsV1> = vec![
            ToolchainMappingDiagnosticsV1::legacy_row_unavailable(),
            ToolchainMappingDiagnosticsV1::disabled_by_policy("xcode", 1),
            ToolchainMappingDiagnosticsV1::policy_absent("go"),
            ToolchainMappingDiagnosticsV1::unsupported_family("swift_best_effort"),
            ToolchainMappingDiagnosticsV1::setup_failed(
                "xcode",
                1,
                "toolchain_mapping_setup_failed",
                "mapping_setup_invalid_root",
            ),
            ToolchainMappingDiagnosticsV1::queue_timeout("xcode", 1, 12_000, 300_000),
        ];
        for original in &cases {
            let json = original.to_json_string().expect("serialize must succeed");
            let parsed: ToolchainMappingDiagnosticsV1 =
                serde_json::from_str(&json).expect("deserialize must succeed");
            assert_eq!(
                original, &parsed,
                "roundtrip failed for state {:?}",
                original.mapping_state
            );
        }
    }

    #[test]
    fn p066_diag_from_json_or_legacy_null_returns_legacy_sentinel() {
        let d = ToolchainMappingDiagnosticsV1::from_json_or_legacy(None);
        assert_eq!(d.mapping_state, DiagMappingState::LegacyRowUnavailable);
        assert_eq!(d.policy_source, DiagPolicySource::SynthesizedLegacy);
    }

    #[test]
    fn p066_diag_from_json_or_legacy_malformed_returns_legacy_sentinel() {
        let d = ToolchainMappingDiagnosticsV1::from_json_or_legacy(Some("{not valid json}"));
        assert_eq!(d.mapping_state, DiagMappingState::LegacyRowUnavailable);
    }

    #[test]
    fn p066_diag_from_json_or_legacy_valid_json_decoded_correctly() {
        let original = ToolchainMappingDiagnosticsV1::disabled_by_policy("claude", 1);
        let json = original.to_json_string().unwrap();
        let decoded = ToolchainMappingDiagnosticsV1::from_json_or_legacy(Some(&json));
        assert_eq!(decoded.mapping_state, DiagMappingState::DisabledByPolicy);
        assert_eq!(decoded.policy_source, DiagPolicySource::RunplanSnapshot);
        assert_eq!(decoded.provider_family, "claude");
    }

    #[test]
    fn p066_diag_mapping_state_as_str_matches_serde() {
        let states = [
            (DiagMappingState::Active, "active"),
            (DiagMappingState::DisabledByPolicy, "disabled_by_policy"),
            (DiagMappingState::PolicyAbsent, "policy_absent"),
            (DiagMappingState::UnsupportedFamily, "unsupported_family"),
            (DiagMappingState::SetupFailed, "setup_failed"),
            (DiagMappingState::QueueTimeout, "queue_timeout"),
            (
                DiagMappingState::LegacyRowUnavailable,
                "legacy_row_unavailable",
            ),
        ];
        for (state, expected) in &states {
            assert_eq!(state.as_str(), *expected, "as_str mismatch for {:?}", state);
            // Verify serde agrees
            let json = serde_json::to_string(state).unwrap();
            assert_eq!(
                json,
                format!("\"{}\"", expected),
                "serde mismatch for {:?}",
                state
            );
        }
    }

    #[test]
    fn p066_diag_concurrency_constructors_cover_all_statuses() {
        let acquired = DiagConcurrencyState::acquired_immediately(5);
        assert_eq!(acquired.status, DiagConcurrencyStatus::AcquiredImmediately);
        assert_eq!(acquired.mode, DiagConcurrencyMode::SerializedPerRun);

        let queued = DiagConcurrencyState::queued(1800, 300_000);
        assert_eq!(queued.status, DiagConcurrencyStatus::Queued);
        assert_eq!(queued.wait_ms, Some(1800));

        let timed_out = DiagConcurrencyState::queue_timeout(300_001, 300_000);
        assert_eq!(timed_out.status, DiagConcurrencyStatus::QueueTimeout);

        let cancelled = DiagConcurrencyState::cancelled_before_acquire();
        assert_eq!(
            cancelled.status,
            DiagConcurrencyStatus::CancelledBeforeAcquire
        );

        let quarantined = DiagConcurrencyState::root_quarantined_before_reuse();
        assert_eq!(
            quarantined.status,
            DiagConcurrencyStatus::RootQuarantinedBeforeReuse
        );
    }

    #[test]
    fn p066_diag_cleanup_constructors() {
        let none = DiagCleanupState::none();
        assert_eq!(none.owner, DiagCleanupOwner::None);
        assert_eq!(none.plan, DiagCleanupPlan::None);
        assert!(none.aggregate_outcome_surfaces.is_empty());

        let close = DiagCleanupState::delete_on_close();
        assert_eq!(close.owner, DiagCleanupOwner::AcpSessionClose);
        assert_eq!(close.plan, DiagCleanupPlan::DeleteOnClose);
        assert!(close
            .aggregate_outcome_surfaces
            .contains(&"startupRecoverySummary.toolchainCache".to_string()));

        let prune = DiagCleanupState::defer_until_terminal_prune();
        assert_eq!(prune.owner, DiagCleanupOwner::GeneratedStateHousekeeping);
        assert_eq!(prune.plan, DiagCleanupPlan::DeferUntilTerminalPrune);
    }
}
