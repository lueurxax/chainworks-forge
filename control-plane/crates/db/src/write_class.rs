//! P075 write classification types.
//!
//! All non-test runtime writes must declare their class, lane, deadline, idempotency
//! key, and replay policy via [`WriteOperation`] before being submitted to [`DbWriter`].
//!
//! Write classes (per P075 §architecture.write_classes):
//! - **Class A (barrier)**: synchronous, durable, never silently dropped.
//! - **Class B (coalesced_state)**: may be merged, delayed, and flushed at boundaries.
//! - **Class C (evidence_spool_metadata)**: raw bytes fsynced to file first; metadata pointer only.
//! - **Class D (telemetry_rollup)**: aggregated in memory; droppable with drop counters.
//!
//! The P061 `begin_immediate_with_retry` is the sole retry primitive. A second retry
//! primitive is explicitly NOT introduced here; callers must not add one (P061 contract).

use std::time::Duration;

/// Write priority class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WriteClass {
    /// Barrier: synchronous and durable before the system proceeds. Never intentionally dropped.
    A,
    /// Coalesced state: may be merged, replaced, or delayed; flushed at boundaries or periodic cadence.
    B,
    /// Evidence spool metadata: file written + checksummed + fsynced before metadata is enqueued.
    C,
    /// Telemetry rollup: aggregated in memory; droppable with drop counters.
    D,
}

impl WriteClass {
    /// Default write deadline per P075 §architecture.deadlines_and_results.
    pub const fn default_deadline(self) -> Duration {
        match self {
            Self::A => Duration::from_millis(2000),
            Self::B => Duration::from_millis(1000),
            // Class C deadline starts after fsync(file) + fsync(parent_dir) complete.
            Self::C => Duration::from_millis(5000),
            Self::D => Duration::from_millis(1000),
        }
    }

    /// Maximum allowed deadline for Class A (LIFT-REL-09, BLOCK-004).
    ///
    /// Set equal to the default (2000 ms default, capped here at 5000 ms) so that
    /// unannotated long barriers are rejected. The approved proposal allows deadlines
    /// above 5000 ms only when the caller records an explicit `deadline_reason` in logs
    /// and metrics; Phase 2 will add that field and raise this cap for annotated callers.
    /// Until then, reject anything above the 5000 ms boundary to avoid shipping an
    /// unannotated relaxed policy (DEFECT-002).
    pub const MAX_CLASS_A_DEADLINE: Duration = Duration::from_millis(5_000);

    /// Human-readable label used in logs and metrics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

/// Bounded priority lanes. Higher-priority lanes are always drained before lower ones.
///
/// Lane capacities per P075 §architecture.dbwriter.priority_lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteLane {
    /// Class A only. Capacity: 1024.
    CriticalBarrier,
    /// Class A only. Capacity: 512.
    OperatorCommand,
    /// Class A and B. Capacity: 2048.
    ProjectionInvalidation,
    /// Class B only. Capacity: 4096.
    CoalescedProjection,
    /// Class C only. Capacity: 2048.
    EvidenceMetadata,
    /// Class D only. Capacity: 1024.
    TelemetryRollup,
}

impl WriteLane {
    /// Bounded channel capacity for this lane.
    pub const fn capacity(self) -> usize {
        match self {
            Self::CriticalBarrier => 1024,
            Self::OperatorCommand => 512,
            Self::ProjectionInvalidation => 2048,
            Self::CoalescedProjection => 4096,
            Self::EvidenceMetadata => 2048,
            Self::TelemetryRollup => 1024,
        }
    }

    /// Human-readable name used in metrics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CriticalBarrier => "critical_barrier",
            Self::OperatorCommand => "operator_command",
            Self::ProjectionInvalidation => "projection_invalidation",
            Self::CoalescedProjection => "coalesced_projection",
            Self::EvidenceMetadata => "evidence_metadata",
            Self::TelemetryRollup => "telemetry_rollup",
        }
    }

    /// Warn threshold depth (50% of capacity) per initial storageHealth thresholds.
    pub const fn warn_depth(self) -> usize {
        self.capacity() / 2
    }

    /// Critical threshold depth (80% of capacity).
    pub const fn critical_depth(self) -> usize {
        self.capacity() * 4 / 5
    }
}

/// How a write operation should behave on duplicate submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayPolicy {
    /// Natural primary/unique keys with IF NOT EXISTS or UPSERT; safe to replay at any point.
    NaturalKey,
    /// Last-writer-wins by `observed_at`; used for Class B coalescing keys.
    LastWriterWins,
    /// INSERT OR IGNORE / UPSERT only when checksum + size match; used for Class C metadata.
    ChecksumIdempotent,
    /// Caller guarantees no duplicate application; the operation registry must list a
    /// `duplicate_application_test_path` proving the invariant.
    CallerGuarded,
    /// Additive counter or max-gauge merge; used for Class D telemetry buckets.
    TelemetryMerge,
}

/// A write submitted to [`DbWriter`].
///
/// Every runtime write must populate all fields. The `operation_name` must appear in
/// `write-operation-registry.toml`; the Phase 7 gate enforces this in fail-closed mode.
#[derive(Debug, Clone)]
pub struct WriteOperation {
    /// Write priority class.
    pub class: WriteClass,
    /// Target lane. Must be compatible with `class`.
    pub lane: WriteLane,
    /// Stable name used in logs, metrics, and the operation registry.
    /// Must be a compile-time constant (&'static str).
    pub operation_name: &'static str,
    /// Expected number of rows affected; used for anomaly detection in logs.
    pub expected_rows: u32,
    /// Whether this operation can be combined with adjacent same-key writes.
    pub batchable: bool,
    /// Whether this is a barrier (Class A) write: system must not proceed until durable.
    pub barrier: bool,
    /// Deadline from enqueue to commit. Defaults to class default when not overridden.
    pub deadline: Duration,
    /// Idempotency key for deduplication and coalescing. Format is class/policy-specific.
    pub idempotency_key: String,
    /// Replay policy for this operation.
    pub replay_policy: ReplayPolicy,
    /// Monotonic timestamp for Class B last-writer-wins ordering.
    /// `None` causes DbWriter to assign an enqueue-time monotonic counter (LIFT-REL-08).
    pub observed_at: Option<u64>,
}

impl WriteOperation {
    /// Validate class-specific constraints.
    ///
    /// Returns `Err(WriteResult::WriteRejected)` when the Class A deadline exceeds
    /// the policy upper bound (LIFT-REL-09).
    pub fn validate(&self) -> Result<(), WriteResult> {
        if self.class == WriteClass::A && self.deadline > WriteClass::MAX_CLASS_A_DEADLINE {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "deadline_exceeds_policy",
            });
        }
        Ok(())
    }
}

/// Typed result returned to the caller after a write is processed by [`DbWriter`].
///
/// Variants are non-collapsed: WriteBusyExhausted is never coerced into WriteFailed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteResult {
    /// Transaction committed successfully.
    Committed,
    /// Write was merged into a pending coalesced write for the same key (Class B only).
    Coalesced,
    /// Write was dropped from the telemetry rollup queue; drop counter incremented (Class D only).
    DroppedTelemetry,
    /// Write was rejected before or at admission.
    WriteRejected {
        /// Lane name where the reject originated.
        lane: &'static str,
        /// Configured capacity of that lane.
        capacity: usize,
        /// Number of items queued at rejection time.
        queued_depth: usize,
        /// Age of the oldest queued item in milliseconds at rejection time.
        oldest_queued_ms: u64,
        /// Operation name from the original WriteOperation.
        operation_name: &'static str,
        /// Short reason token (e.g. "deadline_exceeds_policy", "lane_saturated",
        /// "shutdown_admission_denied").
        reason: &'static str,
    },
    /// Write was admitted but the deadline elapsed before the transaction committed.
    WriteTimeout,
    /// P061 BEGIN IMMEDIATE retry was exhausted; distinct from generic SQL errors.
    WriteBusyExhausted,
    /// Transaction failed for other reasons (SQL error, constraint violation, etc.).
    WriteFailed,
}

impl WriteResult {
    pub fn is_committed(&self) -> bool {
        matches!(self, Self::Committed)
    }

    /// Returns true for outcomes that represent successful persistence or intentional discard.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Committed | Self::Coalesced | Self::DroppedTelemetry)
    }

    /// Returns a short token for logging and metrics.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Coalesced => "coalesced",
            Self::DroppedTelemetry => "dropped_telemetry",
            Self::WriteRejected { .. } => "write_rejected",
            Self::WriteTimeout => "write_timeout",
            Self::WriteBusyExhausted => "write_busy_exhausted",
            Self::WriteFailed => "write_failed",
        }
    }
}

/// Outcome of a producer-side evidence spool file write.
///
/// Pinned in Phase 1 per LIFT-REL-01. The producer-side watchdog and
/// `evidence_spool_fsync_timeout_total` metric are wired in Phase 3 (P075-P3-A2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpoolWriteOutcome {
    /// File written, checksummed, fsynced, and atomically renamed; metadata enqueued.
    Written,
    /// fsync deadline elapsed; producer cancelled before any metadata write.
    /// Increments `evidence_spool_fsync_timeout_total`.
    Timeout,
    /// File write, checksum, or fsync failed; no metadata was written.
    Failed,
}

/// Metric name constant for evidence spool fsync timeout (LIFT-REL-01).
pub const METRIC_EVIDENCE_SPOOL_FSYNC_TIMEOUT_TOTAL: &str = "evidence_spool_fsync_timeout_total";

/// Metric name constant for evidence metadata conflict (Class C mismatch).
pub const METRIC_EVIDENCE_METADATA_CONFLICT_TOTAL: &str = "evidence_metadata_conflict_total";

/// Metric name for lane starvation events.
pub const METRIC_LANE_STARVATION_TOTAL: &str = "lane_starvation_total";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_result_variants_are_non_collapsed() {
        // Verify each variant is distinct and the as_str tokens don't collide.
        let variants = [
            WriteResult::Committed,
            WriteResult::Coalesced,
            WriteResult::DroppedTelemetry,
            WriteResult::WriteRejected {
                lane: "critical_barrier",
                capacity: 1024,
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: "test_op",
                reason: "lane_saturated",
            },
            WriteResult::WriteTimeout,
            WriteResult::WriteBusyExhausted,
            WriteResult::WriteFailed,
        ];
        let tokens: Vec<&str> = variants.iter().map(|v| v.as_str()).collect();
        // All tokens must be unique (no two variants map to the same string).
        let unique_count = {
            let mut s = tokens.clone();
            s.sort_unstable();
            s.dedup();
            s.len()
        };
        assert_eq!(tokens.len(), unique_count, "WriteResult token collision: {tokens:?}");
    }

    #[test]
    fn write_result_success_semantics() {
        assert!(WriteResult::Committed.is_success());
        assert!(WriteResult::Committed.is_committed());
        assert!(WriteResult::Coalesced.is_success());
        assert!(!WriteResult::Coalesced.is_committed());
        assert!(WriteResult::DroppedTelemetry.is_success());
        assert!(!WriteResult::WriteTimeout.is_success());
        assert!(!WriteResult::WriteBusyExhausted.is_success());
        assert!(!WriteResult::WriteFailed.is_success());
    }

    #[test]
    fn write_class_default_deadlines() {
        assert_eq!(WriteClass::A.default_deadline(), Duration::from_millis(2000));
        assert_eq!(WriteClass::B.default_deadline(), Duration::from_millis(1000));
        assert_eq!(WriteClass::C.default_deadline(), Duration::from_millis(5000));
        assert_eq!(WriteClass::D.default_deadline(), Duration::from_millis(1000));
    }

    #[test]
    fn write_lane_capacities() {
        assert_eq!(WriteLane::CriticalBarrier.capacity(), 1024);
        assert_eq!(WriteLane::OperatorCommand.capacity(), 512);
        assert_eq!(WriteLane::ProjectionInvalidation.capacity(), 2048);
        assert_eq!(WriteLane::CoalescedProjection.capacity(), 4096);
        assert_eq!(WriteLane::EvidenceMetadata.capacity(), 2048);
        assert_eq!(WriteLane::TelemetryRollup.capacity(), 1024);
    }

    #[test]
    fn write_operation_validate_deadline_exceeds_policy() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: Duration::from_millis(15_000), // exceeds 5_000 ms policy
            idempotency_key: "key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        let result = op.validate();
        assert!(result.is_err());
        if let Err(WriteResult::WriteRejected { reason, .. }) = result {
            assert_eq!(reason, "deadline_exceeds_policy");
        } else {
            panic!("expected WriteRejected");
        }
    }

    #[test]
    fn write_operation_validate_class_a_within_policy() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: Duration::from_millis(2000), // within policy
            idempotency_key: "key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(op.validate().is_ok());
    }

    #[test]
    fn write_operation_validate_class_a_at_max_policy() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::MAX_CLASS_A_DEADLINE,
            idempotency_key: "key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        // Exactly at the limit is allowed (policy is strictly >).
        assert!(op.validate().is_ok());
    }

    #[test]
    fn spool_write_outcome_variants_exist() {
        let _ = SpoolWriteOutcome::Written;
        let _ = SpoolWriteOutcome::Timeout;
        let _ = SpoolWriteOutcome::Failed;
    }
}
