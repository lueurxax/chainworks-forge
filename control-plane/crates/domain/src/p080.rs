/// P080 Continuous Stale Execution Reconciliation — domain types.
///
/// All enums are closed vocabularies matching the SQL CHECK constraints in
/// migration 086_p080_stale_execution_reconciliation.sql and the MCP/GraphQL
/// wire schemas defined in the P080 proposal.
use serde::{Deserialize, Serialize};

// ── Closed-vocabulary enums ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080StaleClass {
    Useful,
    WarmupPending,
    AcpStartupStale,
    AcpPromptStale,
    SchedulerOwnershipDrift,
    HelperOrphanDrift,
    ReleaseSideEffectDrift,
    AmbiguousOwner,
    Unknown,
}

impl P080StaleClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::WarmupPending => "warmup_pending",
            Self::AcpStartupStale => "acp_startup_stale",
            Self::AcpPromptStale => "acp_prompt_stale",
            Self::SchedulerOwnershipDrift => "scheduler_ownership_drift",
            Self::HelperOrphanDrift => "helper_orphan_drift",
            Self::ReleaseSideEffectDrift => "release_side_effect_drift",
            Self::AmbiguousOwner => "ambiguous_owner",
            Self::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for P080StaleClass {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "useful" => Ok(Self::Useful),
            "warmup_pending" => Ok(Self::WarmupPending),
            "acp_startup_stale" => Ok(Self::AcpStartupStale),
            "acp_prompt_stale" => Ok(Self::AcpPromptStale),
            "scheduler_ownership_drift" => Ok(Self::SchedulerOwnershipDrift),
            "helper_orphan_drift" => Ok(Self::HelperOrphanDrift),
            "release_side_effect_drift" => Ok(Self::ReleaseSideEffectDrift),
            "ambiguous_owner" => Ok(Self::AmbiguousOwner),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!("unknown P080StaleClass: {other}")),
        }
    }
}

impl std::fmt::Display for P080StaleClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080RunningTruth {
    Useful,
    WarmupPending,
    StaleSuspected,
    NeedsOperator,
    NeedsEffectReconciliation,
    StaleRepaired,
    Unknown,
}

impl P080RunningTruth {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::WarmupPending => "warmup_pending",
            Self::StaleSuspected => "stale_suspected",
            Self::NeedsOperator => "needs_operator",
            Self::NeedsEffectReconciliation => "needs_effect_reconciliation",
            Self::StaleRepaired => "stale_repaired",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for P080RunningTruth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080RepairAction {
    None,
    DiagnoseOnly,
    AcpSessionReset,
    SchedulerLeaseReclaim,
    HelperReap,
    DelegatedToP037,
    DelegatedToP076,
    Hold,
    ClearPermanentHold,
}

impl P080RepairAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DiagnoseOnly => "diagnose_only",
            Self::AcpSessionReset => "acp_session_reset",
            Self::SchedulerLeaseReclaim => "scheduler_lease_reclaim",
            Self::HelperReap => "helper_reap",
            Self::DelegatedToP037 => "delegated_to_p037",
            Self::DelegatedToP076 => "delegated_to_p076",
            Self::Hold => "hold",
            Self::ClearPermanentHold => "clear_permanent_hold",
        }
    }
}

impl std::fmt::Display for P080RepairAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080HoldReason {
    None,
    CooldownActive,
    PermanentHoldActive,
    AmbiguousOwner,
    SideEffectDriftUnsafe,
    DependencyReadFailure,
    GatewaySaturated,
    LiveDisable,
    WarmupPending,
    RolloutDisabled,
    Unknown,
}

impl P080HoldReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CooldownActive => "cooldown_active",
            Self::PermanentHoldActive => "permanent_hold_active",
            Self::AmbiguousOwner => "ambiguous_owner",
            Self::SideEffectDriftUnsafe => "side_effect_drift_unsafe",
            Self::DependencyReadFailure => "dependency_read_failure",
            Self::GatewaySaturated => "gateway_saturated",
            Self::LiveDisable => "live_disable",
            Self::WarmupPending => "warmup_pending",
            Self::RolloutDisabled => "rollout_disabled",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for P080HoldReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080Decision {
    Diagnosed,
    Repaired,
    Held,
    RaceAborted,
    Delegated,
    Cleared,
    AlreadyCleared,
    Rejected,
}

impl P080Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Diagnosed => "diagnosed",
            Self::Repaired => "repaired",
            Self::Held => "held",
            Self::RaceAborted => "race_aborted",
            Self::Delegated => "delegated",
            Self::Cleared => "cleared",
            Self::AlreadyCleared => "already_cleared",
            Self::Rejected => "rejected",
        }
    }
}

impl std::fmt::Display for P080Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080SideEffectStatus {
    NotApplicable,
    RetrySafe,
    Unsafe,
    Unknown,
}

impl P080SideEffectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::RetrySafe => "retry_safe",
            Self::Unsafe => "unsafe",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for P080SideEffectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080RolloutDisablement {
    None,
    PhaseNotReached,
    ClassDisabled,
    LiveDisabled,
}

impl P080RolloutDisablement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PhaseNotReached => "phase_not_reached",
            Self::ClassDisabled => "class_disabled",
            Self::LiveDisabled => "live_disabled",
        }
    }
}

impl std::fmt::Display for P080RolloutDisablement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080ProjectionIntegrity {
    Valid,
    Stale,
    TamperDetected,
}

impl P080ProjectionIntegrity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Stale => "stale",
            Self::TamperDetected => "tamper_detected",
        }
    }
}

impl std::fmt::Display for P080ProjectionIntegrity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080ExecutorReregistrationState {
    Expected,
    Seen,
    Missing,
    Stale,
    Recovered,
}

impl P080ExecutorReregistrationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expected => "expected",
            Self::Seen => "seen",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Recovered => "recovered",
        }
    }
}

impl std::fmt::Display for P080ExecutorReregistrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080RequestedAction {
    DiagnoseOnly,
    RepairIfSafe,
    Hold,
}

impl P080RequestedAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DiagnoseOnly => "diagnose_only",
            Self::RepairIfSafe => "repair_if_safe",
            Self::Hold => "hold",
        }
    }
}

impl std::str::FromStr for P080RequestedAction {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "diagnose_only" => Ok(Self::DiagnoseOnly),
            "repair_if_safe" => Ok(Self::RepairIfSafe),
            "hold" => Ok(Self::Hold),
            other => Err(format!("unknown P080RequestedAction: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080EventSource {
    LiveLoop,
    StartupRepair,
    OperatorRequest,
}

impl P080EventSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LiveLoop => "live_loop",
            Self::StartupRepair => "startup_repair",
            Self::OperatorRequest => "operator_request",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080CursorInvalidReason {
    Expired,
    VersionMismatch,
    Malformed,
    FilterChanged,
    ProjectionGenerationMismatch,
}

impl P080CursorInvalidReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::VersionMismatch => "version_mismatch",
            Self::Malformed => "malformed",
            Self::FilterChanged => "filter_changed",
            Self::ProjectionGenerationMismatch => "projection_generation_mismatch",
        }
    }
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

/// Canonical readback payload (p080_readback_v1).
/// Appears on all output lanes: MCP, GraphQL, run-report, release-receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P080Readback {
    pub schema_version: String,
    pub run_id: String,
    pub stage_id: String,
    pub work_item_id: String,
    pub stale_class: P080StaleClass,
    pub running_truth: P080RunningTruth,
    pub repair_action: P080RepairAction,
    pub hold_reason: P080HoldReason,
    pub hold_age_seconds: Option<i64>,
    pub next_retry_or_backoff_time: Option<String>,
    pub projection_updated_at: String,
    pub projection_integrity: P080ProjectionIntegrity,
    pub executor_reregistration_state: P080ExecutorReregistrationState,
    pub rollout_disablement: P080RolloutDisablement,
    pub side_effect_status: P080SideEffectStatus,
    pub operator_message: String,
    pub evidence_marker_hash: Option<String>,
    pub repair_idempotency_key: Option<String>,
}

impl P080Readback {
    pub fn rollout_disabled_stub(
        run_id: String,
        stage_id: String,
        work_item_id: String,
        stale_class: P080StaleClass,
        now_rfc3339: String,
    ) -> Self {
        Self {
            schema_version: "p080_readback_v1".to_string(),
            run_id,
            stage_id,
            work_item_id,
            stale_class,
            running_truth: P080RunningTruth::Unknown,
            repair_action: P080RepairAction::None,
            hold_reason: P080HoldReason::RolloutDisabled,
            hold_age_seconds: None,
            next_retry_or_backoff_time: None,
            projection_updated_at: now_rfc3339,
            projection_integrity: P080ProjectionIntegrity::Stale,
            executor_reregistration_state: P080ExecutorReregistrationState::Expected,
            rollout_disablement: P080RolloutDisablement::PhaseNotReached,
            side_effect_status: P080SideEffectStatus::Unknown,
            operator_message: String::new(),
            evidence_marker_hash: None,
            repair_idempotency_key: None,
        }
    }
}

/// Per-item wrapper returned by p080.diagnostics.get.v1 (p080_item_v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P080Item {
    pub readback: P080Readback,
    pub last_repair_event_id: Option<String>,
    pub last_event_at: Option<String>,
    pub recurrence_epoch: i64,
}

/// Pagination info (p080_page_info_v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P080PageInfo {
    pub next_cursor: Option<String>,
    pub cursor_version: i64,
    pub cursor_expires_at: Option<String>,
    pub has_next_page: bool,
    pub total_count_exact: Option<i64>,
}

/// Filter for p080.diagnostics.get.v1 requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct P080DiagnosticsFilter {
    pub run_id: Option<String>,
    pub stage_id: Option<String>,
    pub work_item_id: Option<String>,
    pub stale_class: Option<P080StaleClass>,
    pub hold_reason: Option<P080HoldReason>,
    pub include_recent_repaired: bool,
}

/// Rollout control row (one per feature class).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P080RolloutControl {
    pub class: String,
    pub enabled: bool,
    pub phase: String,
    pub generation: i64,
    pub updated_at: String,
}

/// P080 error code vocabulary (closed set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P080ErrorCode {
    RequestTooLarge,
    JsonDepthExceeded,
    DuplicateKey,
    ArrayLengthExceeded,
    StringTooLarge,
    OperatorMessageTooLarge,
    InvalidCursor,
    InvalidDedupKey,
    DetailsJsonTooLarge,
    CanonicalizationBudgetExceeded,
    Unauthenticated,
    UnauthorizedMissingCapability,
    RolloutDisabled,
    ClassDisabled,
    LiveDisabled,
    ActionDisabledInPhase,
    UnknownField,
    InvalidField,
    IdempotencyConflict,
    EnumerationBudgetExceeded,
    PermanentHoldActive,
    PredicateRevalidationFailed,
    SideEffectUnsafe,
    DependencyUnavailable,
    UnsupportedVersion,
    VersionMismatch,
    InternalError,
}
