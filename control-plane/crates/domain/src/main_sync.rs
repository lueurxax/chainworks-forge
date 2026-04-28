//! P064: Domain types for run worktree main sync and cross-run knowledge transfer.
//!
//! Phase 0 contract freeze — these types define the worktree access mode,
//! mutation barrier schema, main sync attempt states, and knowledge capsule
//! attachment shapes. No I/O; pure domain types only.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Worktree access mode contract (p064-0-11) ─────────────────────────

/// Declares how a work item or system task accesses a run's worktree.
///
/// Review agents that read directly from the implementation worktree must
/// declare `Read`; write-enabled implementation work declares `Write`.
/// The scheduler must not claim new read or write work for a worktree
/// resource key while a sync barrier is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeAccessMode {
    /// Work item does not touch the worktree filesystem.
    None,
    /// Work item reads from the worktree (e.g. review agents).
    Read,
    /// Work item writes to the worktree (e.g. implementation agents).
    Write,
}

impl std::fmt::Display for WorktreeAccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
        }
    }
}

impl std::str::FromStr for WorktreeAccessMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            other => Err(format!("unknown worktree_access_mode: {other}")),
        }
    }
}

impl Default for WorktreeAccessMode {
    fn default() -> Self {
        Self::None
    }
}

// ── Worktree mutation barrier contract (p064-0-12) ─────────────────────

/// Status of a worktree mutation barrier lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarrierStatus {
    /// Barrier has been requested but not yet acquired (consumers active).
    Pending,
    /// Barrier is actively held — no worktree consumers may proceed.
    Active,
    /// Barrier was released normally after sync completed.
    Released,
    /// Barrier was stolen from a stale owner (heartbeat expired).
    Stolen,
    /// Barrier acquisition failed (e.g. fail-closed on missing metadata).
    Failed,
}

impl std::fmt::Display for BarrierStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Active => write!(f, "active"),
            Self::Released => write!(f, "released"),
            Self::Stolen => write!(f, "stolen"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for BarrierStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "released" => Ok(Self::Released),
            "stolen" => Ok(Self::Stolen),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown barrier_status: {other}")),
        }
    }
}

/// Identifies the system owner that holds or requests a barrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarrierOwnerKind {
    MainSync,
    StartupRepair,
}

impl std::fmt::Display for BarrierOwnerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MainSync => write!(f, "main_sync"),
            Self::StartupRepair => write!(f, "startup_repair"),
        }
    }
}

/// A worktree mutation barrier lease record.
///
/// While the barrier is active, the scheduler must not claim new read or
/// write work items for the same `worktree_resource_key`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorktreeMutationBarrier {
    pub id: String,
    pub run_id: String,
    pub worktree_resource_key: String,
    pub owner_id: String,
    pub owner_kind: BarrierOwnerKind,
    pub status: BarrierStatus,
    pub reason: String,
    pub acquired_at: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// ── Main sync attempt states ───────────────────────────────────────────

/// Lifecycle status of a single main-sync attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainSyncAttemptStatus {
    /// Request queued, waiting for barrier or prerequisites.
    Pending,
    /// Barrier requested but active consumers block acquisition.
    WaitingForBarrier,
    /// Barrier acquired, inspecting worktree state.
    Inspecting,
    /// Local main is already ancestor — no merge needed.
    NoOp,
    /// Preserving dirty work before merge.
    Preserving,
    /// Running git merge (fast-forward or normal).
    Merging,
    /// Merge completed successfully.
    Succeeded,
    /// Merge produced conflicts; abort completed, resolver queued.
    Conflicted,
    /// Attempt failed (preservation, merge, or abort error).
    Failed,
    /// Requires operator intervention to reconcile inconsistent state.
    RecoveryRequired,
}

impl std::fmt::Display for MainSyncAttemptStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::WaitingForBarrier => write!(f, "waiting_for_barrier"),
            Self::Inspecting => write!(f, "inspecting"),
            Self::NoOp => write!(f, "no_op"),
            Self::Preserving => write!(f, "preserving"),
            Self::Merging => write!(f, "merging"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Conflicted => write!(f, "conflicted"),
            Self::Failed => write!(f, "failed"),
            Self::RecoveryRequired => write!(f, "recovery_required"),
        }
    }
}

impl std::str::FromStr for MainSyncAttemptStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "waiting_for_barrier" => Ok(Self::WaitingForBarrier),
            "inspecting" => Ok(Self::Inspecting),
            "no_op" => Ok(Self::NoOp),
            "preserving" => Ok(Self::Preserving),
            "merging" => Ok(Self::Merging),
            "succeeded" => Ok(Self::Succeeded),
            "conflicted" => Ok(Self::Conflicted),
            "failed" => Ok(Self::Failed),
            "recovery_required" => Ok(Self::RecoveryRequired),
            other => Err(format!("unknown main_sync_attempt_status: {other}")),
        }
    }
}

/// Resolver terminal result for a conflict-resolution pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolverResult {
    Resolved,
    Retryable,
    Failed,
    RecoveryRequired,
}

impl std::fmt::Display for ResolverResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolved => write!(f, "resolved"),
            Self::Retryable => write!(f, "retryable"),
            Self::Failed => write!(f, "failed"),
            Self::RecoveryRequired => write!(f, "recovery_required"),
        }
    }
}

// ── Knowledge capsule types ────────────────────────────────────────────

/// Status of a persisted knowledge capsule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeCapsuleStatus {
    /// Capsule emitted and available for matching.
    Active,
    /// Capsule ignored by operator.
    Ignored,
    /// Capsule expired (TTL exceeded).
    Expired,
}

impl std::fmt::Display for KnowledgeCapsuleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Ignored => write!(f, "ignored"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Match rule kind used when attaching a capsule to a run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleMatchRule {
    ProposalId,
    OverlappingFiles,
    SharedReferenceDoc,
    SharedWorkflowState,
    SharedSubsystemTag,
    OperatorPin,
}

impl std::fmt::Display for CapsuleMatchRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposalId => write!(f, "proposal_id"),
            Self::OverlappingFiles => write!(f, "overlapping_files"),
            Self::SharedReferenceDoc => write!(f, "shared_reference_doc"),
            Self::SharedWorkflowState => write!(f, "shared_workflow_state"),
            Self::SharedSubsystemTag => write!(f, "shared_subsystem_tag"),
            Self::OperatorPin => write!(f, "operator_pin"),
        }
    }
}

// ── Active consumer readback ───────────────────────────────────────────

/// Projected summary of an active worktree consumer blocking a barrier.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveWorktreeConsumer {
    pub consumer_kind: String,
    pub work_item_id: Option<String>,
    pub system_owner_id: Option<String>,
    pub access_mode: WorktreeAccessMode,
    pub barrier_reason: String,
    pub started_at: DateTime<Utc>,
    pub lease_expires_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_access_mode_roundtrip() {
        for mode in &[
            WorktreeAccessMode::None,
            WorktreeAccessMode::Read,
            WorktreeAccessMode::Write,
        ] {
            let s = mode.to_string();
            let parsed: WorktreeAccessMode = s.parse().unwrap();
            assert_eq!(*mode, parsed);

            let json = serde_json::to_value(mode).unwrap();
            let deserialized: WorktreeAccessMode = serde_json::from_value(json).unwrap();
            assert_eq!(*mode, deserialized);
        }
    }

    #[test]
    fn worktree_access_mode_default_is_none() {
        assert_eq!(WorktreeAccessMode::default(), WorktreeAccessMode::None);
    }

    #[test]
    fn barrier_status_roundtrip() {
        for status in &[
            BarrierStatus::Pending,
            BarrierStatus::Active,
            BarrierStatus::Released,
            BarrierStatus::Stolen,
            BarrierStatus::Failed,
        ] {
            let s = status.to_string();
            let parsed: BarrierStatus = s.parse().unwrap();
            assert_eq!(*status, parsed);
        }
    }

    #[test]
    fn main_sync_attempt_status_roundtrip() {
        for status in &[
            MainSyncAttemptStatus::Pending,
            MainSyncAttemptStatus::WaitingForBarrier,
            MainSyncAttemptStatus::Inspecting,
            MainSyncAttemptStatus::NoOp,
            MainSyncAttemptStatus::Preserving,
            MainSyncAttemptStatus::Merging,
            MainSyncAttemptStatus::Succeeded,
            MainSyncAttemptStatus::Conflicted,
            MainSyncAttemptStatus::Failed,
            MainSyncAttemptStatus::RecoveryRequired,
        ] {
            let s = status.to_string();
            let parsed: MainSyncAttemptStatus = s.parse().unwrap();
            assert_eq!(*status, parsed);
        }
    }

    #[test]
    fn knowledge_capsule_status_roundtrip() {
        for status in &[
            KnowledgeCapsuleStatus::Active,
            KnowledgeCapsuleStatus::Ignored,
            KnowledgeCapsuleStatus::Expired,
        ] {
            let s = status.to_string();
            let parsed: KnowledgeCapsuleStatus =
                serde_json::from_value(serde_json::json!(s)).unwrap();
            assert_eq!(*status, parsed);
        }
    }

    #[test]
    fn capsule_match_rule_serde_snake_case() {
        assert_eq!(
            serde_json::to_value(CapsuleMatchRule::OverlappingFiles).unwrap(),
            serde_json::Value::String("overlapping_files".into())
        );
        assert_eq!(
            serde_json::to_value(CapsuleMatchRule::OperatorPin).unwrap(),
            serde_json::Value::String("operator_pin".into())
        );
    }
}
