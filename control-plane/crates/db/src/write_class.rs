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

    /// Threshold above which a Class A deadline requires an explicit `deadline_reason`.
    ///
    /// Callers with `deadline > MAX_CLASS_A_DEADLINE` must supply a non-None
    /// `WriteOperation.deadline_reason` explaining the business need. Unannotated
    /// long deadlines are rejected with `"deadline_exceeds_policy"`. Phase 2 adds
    /// metric recording for annotated long-deadline calls.
    ///
    /// Per P075 §architecture.deadlines_and_results.caller_override.
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

    /// Index of this lane in [`crate::writer::LANE_DRAIN_ORDER`].
    ///
    /// Used by `DbWriterHeartbeat` to index per-lane state arrays.
    pub const fn drain_order_index(self) -> usize {
        match self {
            Self::CriticalBarrier => 0,
            Self::OperatorCommand => 1,
            Self::ProjectionInvalidation => 2,
            Self::CoalescedProjection => 3,
            Self::EvidenceMetadata => 4,
            Self::TelemetryRollup => 5,
        }
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
    /// Required when `deadline > WriteClass::MAX_CLASS_A_DEADLINE` for Class A writes.
    ///
    /// Must be a compile-time constant explaining the business justification for the
    /// extended deadline. When present, DbWriter logs and records it in metrics (Phase 2).
    /// When absent and the deadline exceeds the cap, `validate()` returns
    /// `WriteRejected{reason: "deadline_exceeds_policy"}`.
    ///
    /// Per P075 §architecture.deadlines_and_results.caller_override.
    pub deadline_reason: Option<&'static str>,
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
    /// Returns `Err(WriteResult::WriteRejected)` when:
    /// - The lane is incompatible with the declared class (PPR-007).
    /// - Class A write does not declare `barrier=true` (PPR-007).
    /// - The replay policy is incompatible with the declared class (PPR-007).
    /// - The Class A deadline exceeds `MAX_CLASS_A_DEADLINE` and no `deadline_reason`
    ///   is provided (LIFT-REL-09, P075 §deadlines.caller_override).
    /// - The idempotency_key contains NUL or ASCII control characters (P075-SEC-MED-001).
    pub fn validate(&self) -> Result<(), WriteResult> {
        // PPR-007: fail-closed class/lane compatibility check.
        // A misclassified telemetry write must not be admitted into a barrier lane.
        let class_lane_ok = match self.lane {
            WriteLane::CriticalBarrier => matches!(self.class, WriteClass::A),
            WriteLane::OperatorCommand => matches!(self.class, WriteClass::A),
            WriteLane::ProjectionInvalidation => {
                matches!(self.class, WriteClass::A | WriteClass::B)
            }
            WriteLane::CoalescedProjection => matches!(self.class, WriteClass::B),
            WriteLane::EvidenceMetadata => matches!(self.class, WriteClass::C),
            WriteLane::TelemetryRollup => matches!(self.class, WriteClass::D),
        };
        if !class_lane_ok {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "class_lane_incompatible",
            });
        }

        // PPR-007: Class A writes must declare barrier=true.
        if self.class == WriteClass::A && !self.barrier {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "class_a_barrier_required",
            });
        }

        // PPR-007: replay policy must match the declared write class.
        let replay_ok = match self.class {
            WriteClass::A => {
                matches!(
                    self.replay_policy,
                    ReplayPolicy::NaturalKey | ReplayPolicy::CallerGuarded
                )
            }
            WriteClass::B => matches!(self.replay_policy, ReplayPolicy::LastWriterWins),
            WriteClass::C => matches!(self.replay_policy, ReplayPolicy::ChecksumIdempotent),
            WriteClass::D => matches!(self.replay_policy, ReplayPolicy::TelemetryMerge),
        };
        if !replay_ok {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "replay_policy_class_mismatch",
            });
        }

        if self.class == WriteClass::A && self.deadline > WriteClass::MAX_CLASS_A_DEADLINE {
            if self.deadline_reason.is_none() {
                return Err(WriteResult::WriteRejected {
                    lane: self.lane.as_str(),
                    capacity: self.lane.capacity(),
                    queued_depth: 0,
                    oldest_queued_ms: 0,
                    operation_name: self.operation_name,
                    reason: "deadline_exceeds_policy",
                });
            }
            // deadline_reason is Some: annotated long deadline is permitted.
            // Phase 2 records this in metrics (long_deadline_total by operation_name).
        }
        // Cap idempotency_key to prevent unbounded clone/hash costs (P075-SEC-MED-001).
        if self.idempotency_key.len() > 1024 {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "idempotency_key_too_long",
            });
        }
        // Reject control characters to prevent log injection (P075-SEC-MED-001).
        // NUL (0x00) and ASCII control chars (0x01–0x1f, 0x7f) are all rejected.
        if self.idempotency_key.bytes().any(|b| b < 0x20 || b == 0x7f) {
            return Err(WriteResult::WriteRejected {
                lane: self.lane.as_str(),
                capacity: self.lane.capacity(),
                queued_depth: 0,
                oldest_queued_ms: 0,
                operation_name: self.operation_name,
                reason: "idempotency_key_invalid",
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
        matches!(
            self,
            Self::Committed | Self::Coalesced | Self::DroppedTelemetry
        )
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
        assert_eq!(
            tokens.len(),
            unique_count,
            "WriteResult token collision: {tokens:?}"
        );
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
        assert_eq!(
            WriteClass::A.default_deadline(),
            Duration::from_millis(2000)
        );
        assert_eq!(
            WriteClass::B.default_deadline(),
            Duration::from_millis(1000)
        );
        assert_eq!(
            WriteClass::C.default_deadline(),
            Duration::from_millis(5000)
        );
        assert_eq!(
            WriteClass::D.default_deadline(),
            Duration::from_millis(1000)
        );
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
            deadline_reason: None,
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
            deadline_reason: None,
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
            deadline_reason: None,
            idempotency_key: "key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        // Exactly at the limit is allowed (policy is strictly >).
        assert!(op.validate().is_ok());
    }

    #[test]
    fn write_operation_validate_class_a_extended_deadline_with_reason_is_allowed() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_long_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: Duration::from_millis(10_000), // exceeds cap but reason provided
            deadline_reason: Some("multi-stage migration requiring extended durable window"),
            idempotency_key: "key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        // With deadline_reason, the extended deadline is allowed (P075 §deadlines.caller_override).
        assert!(
            op.validate().is_ok(),
            "Class A deadline above cap must be accepted when deadline_reason is provided"
        );
    }

    #[test]
    fn write_operation_validate_rejects_idempotency_key_over_1024_bytes() {
        let long_key = "a".repeat(1025);
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: long_key,
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            matches!(
                op.validate(),
                Err(WriteResult::WriteRejected {
                    reason: "idempotency_key_too_long",
                    ..
                })
            ),
            "idempotency_key > 1024 bytes must be rejected (P075-SEC-MED-001)"
        );
    }

    #[test]
    fn write_operation_validate_accepts_idempotency_key_at_1024_bytes() {
        let key_1024 = "a".repeat(1024);
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: key_1024,
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            op.validate().is_ok(),
            "idempotency_key at exactly 1024 bytes must pass"
        );
    }

    #[test]
    fn write_operation_validate_rejects_nul_in_idempotency_key() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/stage\x00/key".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            matches!(
                op.validate(),
                Err(WriteResult::WriteRejected {
                    reason: "idempotency_key_invalid",
                    ..
                })
            ),
            "NUL byte in idempotency_key must be rejected"
        );
    }

    #[test]
    fn write_operation_validate_rejects_newline_in_idempotency_key() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/stage\n/injection".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            matches!(
                op.validate(),
                Err(WriteResult::WriteRejected {
                    reason: "idempotency_key_invalid",
                    ..
                })
            ),
            "newline in idempotency_key must be rejected"
        );
    }

    #[test]
    fn write_operation_validate_accepts_clean_idempotency_key() {
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/stage-1/agent-1/transcript-001".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            op.validate().is_ok(),
            "clean idempotency_key must pass validation"
        );
    }

    #[test]
    fn spool_write_outcome_variants_exist() {
        let _ = SpoolWriteOutcome::Written;
        let _ = SpoolWriteOutcome::Timeout;
        let _ = SpoolWriteOutcome::Failed;
    }

    // -----------------------------------------------------------------------
    // PPR-007: class/lane/barrier/replay compatibility regression tests.
    // These tests ensure WriteOperation::validate() rejects misclassified writes
    // before Phase 2 wires real producers, so the admission default is fail-closed.
    // -----------------------------------------------------------------------

    fn make_class_a_op() -> WriteOperation {
        WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_barrier",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/barrier-1".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        }
    }

    #[test]
    fn validate_rejects_class_d_in_critical_barrier_lane() {
        let mut op = make_class_a_op();
        op.class = WriteClass::D;
        op.barrier = false;
        op.replay_policy = ReplayPolicy::TelemetryMerge;
        let result = op.validate();
        assert!(
            matches!(
                result,
                Err(WriteResult::WriteRejected {
                    reason: "class_lane_incompatible",
                    ..
                })
            ),
            "Class D in CriticalBarrier lane must be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn validate_rejects_class_b_in_operator_command_lane() {
        let mut op = make_class_a_op();
        op.lane = WriteLane::OperatorCommand;
        op.class = WriteClass::B;
        op.barrier = false;
        op.replay_policy = ReplayPolicy::LastWriterWins;
        let result = op.validate();
        assert!(
            matches!(
                result,
                Err(WriteResult::WriteRejected {
                    reason: "class_lane_incompatible",
                    ..
                })
            ),
            "Class B in OperatorCommand lane must be rejected"
        );
    }

    #[test]
    fn validate_rejects_class_c_in_coalesced_projection_lane() {
        let op = WriteOperation {
            class: WriteClass::C,
            lane: WriteLane::CoalescedProjection,
            operation_name: "test_c_in_b_lane",
            expected_rows: 1,
            batchable: false,
            barrier: false,
            deadline: WriteClass::C.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/path-1".to_string(),
            replay_policy: ReplayPolicy::ChecksumIdempotent,
            observed_at: None,
        };
        let result = op.validate();
        assert!(
            matches!(
                result,
                Err(WriteResult::WriteRejected {
                    reason: "class_lane_incompatible",
                    ..
                })
            ),
            "Class C in CoalescedProjection (Class B lane) must be rejected"
        );
    }

    #[test]
    fn validate_rejects_class_a_without_barrier_flag() {
        let mut op = make_class_a_op();
        op.barrier = false; // Class A must have barrier=true
        let result = op.validate();
        assert!(
            matches!(
                result,
                Err(WriteResult::WriteRejected {
                    reason: "class_a_barrier_required",
                    ..
                })
            ),
            "Class A without barrier=true must be rejected"
        );
    }

    #[test]
    fn validate_rejects_class_a_with_last_writer_wins_replay() {
        let mut op = make_class_a_op();
        op.replay_policy = ReplayPolicy::LastWriterWins;
        let result = op.validate();
        assert!(
            matches!(
                result,
                Err(WriteResult::WriteRejected {
                    reason: "replay_policy_class_mismatch",
                    ..
                })
            ),
            "Class A with LastWriterWins replay policy must be rejected"
        );
    }

    #[test]
    fn validate_rejects_class_b_with_natural_key_replay() {
        let op = WriteOperation {
            class: WriteClass::B,
            lane: WriteLane::CoalescedProjection,
            operation_name: "test_b_wrong_replay",
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::B.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/surface-1/proj-1".to_string(),
            replay_policy: ReplayPolicy::NaturalKey, // wrong for Class B
            observed_at: None,
        };
        assert!(
            matches!(
                op.validate(),
                Err(WriteResult::WriteRejected {
                    reason: "replay_policy_class_mismatch",
                    ..
                })
            ),
            "Class B with NaturalKey replay policy must be rejected"
        );
    }

    #[test]
    fn validate_rejects_class_c_with_telemetry_merge_replay() {
        let op = WriteOperation {
            class: WriteClass::C,
            lane: WriteLane::EvidenceMetadata,
            operation_name: "test_c_wrong_replay",
            expected_rows: 1,
            batchable: false,
            barrier: false,
            deadline: WriteClass::C.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/evidence/ts.jsonl".to_string(),
            replay_policy: ReplayPolicy::TelemetryMerge, // wrong for Class C
            observed_at: None,
        };
        assert!(
            matches!(
                op.validate(),
                Err(WriteResult::WriteRejected {
                    reason: "replay_policy_class_mismatch",
                    ..
                })
            ),
            "Class C with TelemetryMerge replay policy must be rejected"
        );
    }

    #[test]
    fn validate_accepts_class_c_with_checksum_idempotent_replay() {
        let op = WriteOperation {
            class: WriteClass::C,
            lane: WriteLane::EvidenceMetadata,
            operation_name: "test_c_correct_replay",
            expected_rows: 1,
            batchable: false,
            barrier: false,
            deadline: WriteClass::C.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/evidence/ts.jsonl".to_string(),
            replay_policy: ReplayPolicy::ChecksumIdempotent,
            observed_at: None,
        };
        assert!(
            op.validate().is_ok(),
            "Class C with ChecksumIdempotent must pass"
        );
    }

    #[test]
    fn validate_accepts_class_d_with_telemetry_merge_replay() {
        let op = WriteOperation {
            class: WriteClass::D,
            lane: WriteLane::TelemetryRollup,
            operation_name: "test_d_telemetry",
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::D.default_deadline(),
            deadline_reason: None,
            idempotency_key: "metric-bucket-1".to_string(),
            replay_policy: ReplayPolicy::TelemetryMerge,
            observed_at: None,
        };
        assert!(
            op.validate().is_ok(),
            "Class D with TelemetryMerge must pass"
        );
    }

    #[test]
    fn validate_accepts_class_a_projection_invalidation_lane() {
        // ProjectionInvalidation accepts both Class A and Class B.
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::ProjectionInvalidation,
            operation_name: "test_a_proj_inv",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/proj-inv-1".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        assert!(
            op.validate().is_ok(),
            "Class A in ProjectionInvalidation lane must pass"
        );
    }

    #[test]
    fn validate_accepts_class_b_projection_invalidation_lane() {
        let op = WriteOperation {
            class: WriteClass::B,
            lane: WriteLane::ProjectionInvalidation,
            operation_name: "test_b_proj_inv",
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::B.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/surface-1/proj-1".to_string(),
            replay_policy: ReplayPolicy::LastWriterWins,
            observed_at: Some(12345),
        };
        assert!(
            op.validate().is_ok(),
            "Class B in ProjectionInvalidation lane must pass"
        );
    }

    #[test]
    fn validate_accepts_class_a_with_caller_guarded_replay() {
        // CallerGuarded is a valid replay policy for Class A.
        let mut op = make_class_a_op();
        op.replay_policy = ReplayPolicy::CallerGuarded;
        assert!(
            op.validate().is_ok(),
            "Class A with CallerGuarded replay policy must pass"
        );
    }
}
