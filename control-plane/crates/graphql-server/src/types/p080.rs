/// P080 GraphQL types — read-only diagnostics projection.
///
/// Mirrors the MCP p080_readback_v1 shape on the GraphQL surface.
/// All enum variants follow the SDL in the P080 proposal (SCREAMING_SNAKE_CASE
/// for GraphQL, snake_case for JSON/MCP).
///
/// V1 shared vocabulary includes an `Unknown` sentinel for forward-compatibility
/// per the proposal shared vocabulary (P080 §6 enum contract).  Resolvers that
/// encounter an unrecognised DB string return the Unknown variant rather than Err.
use async_graphql::*;
use chrono::{DateTime, Utc};
use tracing::warn;

// ── GraphQL Enums ─────────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080StaleClass")]
pub enum GqlP080StaleClass {
    Useful,
    WarmupPending,
    AcpStartupStale,
    AcpPromptStale,
    SchedulerOwnershipDrift,
    HelperOrphanDrift,
    ReleaseSideEffectDrift,
    AmbiguousOwner,
    /// Forward-compatibility sentinel for DB values not yet in this enum (proposal §6).
    Unknown,
}

impl From<&str> for GqlP080StaleClass {
    fn from(s: &str) -> Self {
        match s {
            "useful" => Self::Useful,
            "warmup_pending" => Self::WarmupPending,
            "acp_startup_stale" => Self::AcpStartupStale,
            "acp_prompt_stale" => Self::AcpPromptStale,
            "scheduler_ownership_drift" => Self::SchedulerOwnershipDrift,
            "helper_orphan_drift" => Self::HelperOrphanDrift,
            "release_side_effect_drift" => Self::ReleaseSideEffectDrift,
            "ambiguous_owner" => Self::AmbiguousOwner,
            other => {
                // Unknown on a valid v1 row is a decode failure (transport bug or future
                // schema value); surface it explicitly so it is not silently swallowed.
                warn!(
                    db_value = other,
                    "p080: unrecognised DB stale_class; using Unknown forward-compat sentinel"
                );
                Self::Unknown
            }
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080RunningTruth")]
pub enum GqlP080RunningTruth {
    Useful,
    WarmupPending,
    StaleSuspected,
    NeedsOperator,
    NeedsEffectReconciliation,
    StaleRepaired,
    /// Forward-compatibility sentinel for DB values not yet in this enum (proposal §6).
    Unknown,
}

impl From<&str> for GqlP080RunningTruth {
    fn from(s: &str) -> Self {
        match s {
            "useful" => Self::Useful,
            "warmup_pending" => Self::WarmupPending,
            "stale_suspected" => Self::StaleSuspected,
            "needs_operator" => Self::NeedsOperator,
            "needs_effect_reconciliation" => Self::NeedsEffectReconciliation,
            "stale_repaired" => Self::StaleRepaired,
            other => {
                warn!(
                    db_value = other,
                    "p080: unrecognised DB running_truth; using Unknown forward-compat sentinel"
                );
                Self::Unknown
            }
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080RepairAction")]
pub enum GqlP080RepairAction {
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

impl TryFrom<&str> for GqlP080RepairAction {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, ()> {
        match s {
            "none" => Ok(Self::None),
            "diagnose_only" => Ok(Self::DiagnoseOnly),
            "acp_session_reset" => Ok(Self::AcpSessionReset),
            "scheduler_lease_reclaim" => Ok(Self::SchedulerLeaseReclaim),
            "helper_reap" => Ok(Self::HelperReap),
            "delegated_to_p037" => Ok(Self::DelegatedToP037),
            "delegated_to_p076" => Ok(Self::DelegatedToP076),
            "hold" => Ok(Self::Hold),
            "clear_permanent_hold" => Ok(Self::ClearPermanentHold),
            _ => Err(()),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080HoldReason")]
pub enum GqlP080HoldReason {
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
    /// Forward-compatibility sentinel for DB values not yet in this enum (proposal §6).
    Unknown,
}

impl From<&str> for GqlP080HoldReason {
    fn from(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "cooldown_active" => Self::CooldownActive,
            "permanent_hold_active" => Self::PermanentHoldActive,
            "ambiguous_owner" => Self::AmbiguousOwner,
            "side_effect_drift_unsafe" => Self::SideEffectDriftUnsafe,
            "dependency_read_failure" => Self::DependencyReadFailure,
            "gateway_saturated" => Self::GatewaySaturated,
            "live_disable" => Self::LiveDisable,
            "warmup_pending" => Self::WarmupPending,
            "rollout_disabled" => Self::RolloutDisabled,
            other => {
                warn!(
                    db_value = other,
                    "p080: unrecognised DB hold_reason; using Unknown forward-compat sentinel"
                );
                Self::Unknown
            }
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080SideEffectStatus")]
pub enum GqlP080SideEffectStatus {
    NotApplicable,
    RetrySafe,
    Unsafe,
    /// Forward-compatibility sentinel for DB values not yet in this enum (proposal §6).
    Unknown,
}

impl From<&str> for GqlP080SideEffectStatus {
    fn from(s: &str) -> Self {
        match s {
            "not_applicable" => Self::NotApplicable,
            "retry_safe" => Self::RetrySafe,
            "unsafe" => Self::Unsafe,
            other => {
                warn!(db_value = other, "p080: unrecognised DB side_effect_status; using Unknown forward-compat sentinel");
                Self::Unknown
            }
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080RolloutDisablement")]
pub enum GqlP080RolloutDisablement {
    None,
    PhaseNotReached,
    ClassDisabled,
    LiveDisabled,
}

impl TryFrom<&str> for GqlP080RolloutDisablement {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, ()> {
        match s {
            "none" => Ok(Self::None),
            "phase_not_reached" => Ok(Self::PhaseNotReached),
            "class_disabled" => Ok(Self::ClassDisabled),
            "live_disabled" => Ok(Self::LiveDisabled),
            _ => Err(()),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080ProjectionIntegrity")]
pub enum GqlP080ProjectionIntegrity {
    Valid,
    Stale,
    TamperDetected,
}

impl TryFrom<&str> for GqlP080ProjectionIntegrity {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, ()> {
        match s {
            "valid" => Ok(Self::Valid),
            "stale" => Ok(Self::Stale),
            "tamper_detected" => Ok(Self::TamperDetected),
            _ => Err(()),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080ExecutorReregistrationState")]
pub enum GqlP080ExecutorReregistrationState {
    Expected,
    Seen,
    Missing,
    Stale,
    Recovered,
}

impl TryFrom<&str> for GqlP080ExecutorReregistrationState {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, ()> {
        match s {
            "expected" => Ok(Self::Expected),
            "seen" => Ok(Self::Seen),
            "missing" => Ok(Self::Missing),
            "stale" => Ok(Self::Stale),
            "recovered" => Ok(Self::Recovered),
            _ => Err(()),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080DiagnosticsEventType")]
pub enum GqlP080DiagnosticsEventType {
    InitialSnapshotRow,
    RowUpdated,
    RowRemoved,
    ProjectionRebuilt,
    AuthorizationLost,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P080CursorInvalidReason")]
pub enum GqlP080CursorInvalidReason {
    Expired,
    VersionMismatch,
    Malformed,
    FilterChanged,
    ProjectionGenerationMismatch,
}

// ── GraphQL Object Types ──────────────────────────────────────────────────────

/// Canonical p080_readback_v1 shape on the GraphQL surface.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080Readback")]
pub struct GqlP080Readback {
    pub schema_version: String,
    pub run_id: ID,
    pub stage_id: ID,
    pub work_item_id: ID,
    pub stale_class: GqlP080StaleClass,
    pub running_truth: GqlP080RunningTruth,
    pub repair_action: GqlP080RepairAction,
    pub hold_reason: GqlP080HoldReason,
    pub hold_age_seconds: Option<i32>,
    pub next_retry_or_backoff_time: Option<DateTime<Utc>>,
    pub projection_updated_at: DateTime<Utc>,
    pub projection_integrity: GqlP080ProjectionIntegrity,
    pub executor_reregistration_state: GqlP080ExecutorReregistrationState,
    pub rollout_disablement: GqlP080RolloutDisablement,
    pub side_effect_status: GqlP080SideEffectStatus,
    pub operator_message: String,
    pub evidence_marker_hash: Option<String>,
    pub repair_idempotency_key: Option<String>,
}

/// Per-item wrapper (p080_item_v1) in the diagnostics connection.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080DiagnosticsItem")]
pub struct GqlP080DiagnosticsItem {
    pub readback: GqlP080Readback,
    pub last_repair_event_id: Option<ID>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub recurrence_epoch: i32,
}

/// Cursor/page info for diagnostics pagination.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080PageInfo")]
pub struct GqlP080PageInfo {
    pub end_cursor: Option<String>,
    pub cursor_version: i32,
    pub cursor_expires_at: Option<DateTime<Utc>>,
    pub has_next_page: bool,
}

/// Edge wrapping a single diagnostics item with its cursor position.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080DiagnosticsEdge")]
pub struct GqlP080DiagnosticsEdge {
    pub cursor: String,
    pub node: GqlP080DiagnosticsItem,
}

/// Paginated connection returned by the p080Diagnostics query.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080DiagnosticsConnection")]
pub struct GqlP080DiagnosticsConnection {
    pub edges: Vec<GqlP080DiagnosticsEdge>,
    pub page_info: GqlP080PageInfo,
    pub projection_integrity: GqlP080ProjectionIntegrity,
    pub schema_version: String,
    pub total_count: Option<i32>,
}

/// Event emitted by the p080DiagnosticsUpdates subscription.
#[derive(SimpleObject, Clone)]
#[graphql(name = "P080DiagnosticsEvent")]
pub struct GqlP080DiagnosticsEvent {
    pub r#type: GqlP080DiagnosticsEventType,
    pub item: Option<GqlP080DiagnosticsItem>,
    pub projection_integrity: GqlP080ProjectionIntegrity,
    pub projection_updated_at: DateTime<Utc>,
    pub projection_generation: i32,
}

/// Input filter for p080Diagnostics query and p080DiagnosticsUpdates subscription.
#[derive(InputObject, Default, Clone)]
#[graphql(name = "P080DiagnosticsFilter")]
pub struct GqlP080DiagnosticsFilter {
    pub run_id: Option<ID>,
    pub stage_id: Option<ID>,
    pub work_item_id: Option<ID>,
    pub stale_class: Option<GqlP080StaleClass>,
    pub hold_reason: Option<GqlP080HoldReason>,
}
