//! P075 DbWriter: the single bounded write gateway for the Rust control plane.
//!
//! All non-test runtime writes must route through `DbWriter` or appear in the
//! source-controlled bypass allowlist (`write-bypass-allowlist.toml`). The
//! proposal-075 gate enforces this from Phase 7 onward (fail-closed mode).
//!
//! # Priority lanes
//!
//! DbWriter maintains bounded MPSC queues for six lanes ordered by priority:
//!
//! | Lane                  | Class | Capacity | Drain priority |
//! |-----------------------|-------|----------|----------------|
//! | critical_barrier      | A     | 1024     | 1 (highest)    |
//! | operator_command      | A     | 512      | 1              |
//! | projection_invalidation | A/B | 2048     | 2              |
//! | coalesced_projection  | B     | 4096     | 3              |
//! | evidence_metadata     | C     | 2048     | 4              |
//! | telemetry_rollup      | D     | 1024     | 5 (lowest)     |
//!
//! `critical_barrier` and `operator_command` are always polled before lower lanes.
//! Lower lanes drain by weighted scheduling when no higher lane is over deadline or
//! above warn depth (50% of capacity).
//!
//! # Starvation watchdog
//!
//! If a lower lane is unable to drain for [`STARVATION_WATCHDOG_SECS`] seconds while
//! higher lanes are not saturated, DbWriter increments `lane_starvation_total` and
//! emits a WARN log with the lane name.
//!
//! # Retry primitive (P061 contract)
//!
//! DbWriter reuses `pool::begin_immediate_with_retry` from P061.
//! **A second retry primitive is explicitly NOT introduced here.**
//!
//! # Transaction body rules
//!
//! No provider calls, filesystem scans, network work, artifact discovery,
//! checkpoint waits, or ACP runtime waits inside a SQLite transaction.
//! Class A and B payloads must remain compact. Class C payloads must not contain
//! raw evidence bytes — only metadata pointers.
//!
//! # Class C ordering
//!
//! Evidence file must be: written → checksummed → fsync(file) → atomic rename →
//! fsync(parent_dir) **before** Class C metadata is enqueued. This ordering makes
//! metadata-without-bytes impossible by construction.
//!
//! # Shutdown drain order (P075 §architecture.shutdown_protocol)
//!
//! 1. Stop accepting new Class B, C, and D writes immediately.
//! 2. Accept only Class A writes for operations listed in [`SHUTDOWN_ADMITTED_OPERATIONS`]
//!    (LIFT-REL-03). Others receive `WriteRejected("shutdown_admission_denied")`.
//! 3. Drain Class A lanes within [`SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS`] ms.
//! 4. Force-flush Class B coalescing buffers in one bounded pass (sub-budget:
//!    [`SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS`] ms).
//! 5. Skip Class D by default. One best-effort flush only after Class A drain if budget remains.
//! 6. On timeout, log queue snapshot, unflushed coalescing keys, evidence orphan candidates.
//!
//! # Alive monitoring (LIFT-REL-05)
//!
//! DbWriter emits a 1 Hz heartbeat. Missed heartbeats surface as `alive=false` in
//! `storageHealth.writer.alive`. An in-process supervisor with bounded restart count
//! recovers transient panics; persistent `alive=false` logs at CRITICAL level.
//! Phase 2 wires the heartbeat task and supervisor.
//!
//! # WAL checkpoint policy (LIFT-REL-02)
//!
//! - PASSIVE checkpoint: requested when WAL exceeds [`WARN_WAL_SIZE_BYTES`] and no
//!   Class A write is waiting. Run by a low-priority maintenance task.
//! - TRUNCATE checkpoint: only on graceful shutdown after Class A drain, or via
//!   explicit maintenance command.
//! - Hard upper bound: [`HARD_WAL_UPPER_BOUND_BYTES`]. Above this, a barrier-coordinated
//!   brief checkpoint window is opened regardless of Class A queue state.
//!
//! # Class A WriteRejected engine policy (LIFT-REL-11)
//!
//! Engine callers receiving `WriteRejected` on a Class A barrier must apply bounded
//! retry with backoff: [`CLASS_A_REJECTED_RETRY_ATTEMPTS`] attempts,
//! [`CLASS_A_REJECTED_RETRY_INITIAL_DELAY_MS`] ms initial delay, exponential.
//! After exhaustion, surface failure to the run as a degraded canonical state with
//! an explicit resume hook.
//!
//! # Post-cancel observability (LIFT-REL-10)
//!
//! Dropping a DbWriter request before admission removes it from the queue. Dropping
//! after transaction start does not cancel the transaction; the writer logs the result
//! by `write_id` for post-cancel observability. Callers rely on idempotency to recover.
//!
//! # Class B observed_at fallback (LIFT-REL-08)
//!
//! When a Class B `WriteOperation.observed_at` is `None`, DbWriter assigns a
//! deterministic enqueue-time monotonic counter. This ensures last-writer-wins ordering
//! even when the producer omits `observed_at`.
//!
//! # Class C checksum-mismatch policy (LIFT-REL-06)
//!
//! A checksum mismatch on a Class C metadata insertion is a hard producer error.
//! The write returns `WriteFailed` and the file is flagged for manual reconcile via
//! `storage.reconcile_evidence_orphans`.

use crate::write_class::{WriteLane, WriteOperation, WriteResult};
use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// Coalescing configuration (Class B)
// ---------------------------------------------------------------------------

/// Flush coalescing buffer every 500 ms regardless of producer signals.
pub const COALESCE_FLUSH_INTERVAL_MS: u64 = 500;
/// Flush coalescing buffer after 64 merges to bound memory pressure.
pub const COALESCE_FLUSH_MAX_MERGES: usize = 64;
/// Maximum age of a coalescing key before it must be flushed.
pub const COALESCE_MAX_KEY_AGE_MS: u64 = 2000;

// ---------------------------------------------------------------------------
// Shutdown drain budgets
// ---------------------------------------------------------------------------

/// Class A drain budget during graceful shutdown (ms).
pub const SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS: u64 = 5000;
/// Class B force-flush sub-budget during graceful shutdown (ms).
pub const SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS: u64 = 2000;

// ---------------------------------------------------------------------------
// WAL checkpoint thresholds (LIFT-REL-02)
// ---------------------------------------------------------------------------

/// PASSIVE checkpoint when WAL exceeds this size and no Class A write is waiting.
pub const WARN_WAL_SIZE_BYTES: u64 = 134_217_728; // 128 MiB

/// storageHealth critical WAL threshold.
pub const CRITICAL_WAL_SIZE_BYTES: u64 = 536_870_912; // 512 MiB

/// Hard upper bound: barrier-coordinated brief checkpoint window when WAL exceeds this.
pub const HARD_WAL_UPPER_BOUND_BYTES: u64 = 1_073_741_824; // 1 GiB

// ---------------------------------------------------------------------------
// Starvation watchdog
// ---------------------------------------------------------------------------

/// Log + increment `lane_starvation_total` if a lower lane cannot drain for this long
/// while higher lanes are not saturated.
pub const STARVATION_WATCHDOG_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// Engine Class A retry policy (LIFT-REL-11)
// ---------------------------------------------------------------------------

/// Number of retry attempts before surfacing as degraded canonical state.
pub const CLASS_A_REJECTED_RETRY_ATTEMPTS: u32 = 3;
/// Initial delay for exponential backoff in milliseconds.
pub const CLASS_A_REJECTED_RETRY_INITIAL_DELAY_MS: u64 = 100;

// ---------------------------------------------------------------------------
// Shutdown admission allowlist (LIFT-REL-03)
// ---------------------------------------------------------------------------

/// Operation names admitted to Class A queues after the shutdown signal.
/// All other Class A submissions receive WriteRejected("shutdown_admission_denied").
///
/// Intentionally empty in Phase 1. Phase 2 wires the admission filter and adds
/// entries for terminal canonical state operations (run completion, stage completion).
///
/// **CLEAN-002 WARNING**: When the shutdown admission filter is wired in Phase 2,
/// an empty list will reject ALL Class A writes on the shutdown signal — including
/// terminal canonical state persistence. Phase 2 must add a regression test named
/// `shutdown_class_a_admission_empty_list_rejects_all` to document that behavior
/// and verify the list is non-empty before the filter is active.
pub const SHUTDOWN_ADMITTED_OPERATIONS: &[&str] = &[];

// ---------------------------------------------------------------------------
// Telemetry rollup budget (Class D)
// ---------------------------------------------------------------------------

/// Maximum in-memory bytes for telemetry rollup before forced eviction.
pub const TELEMETRY_MEMORY_CAP_BYTES: usize = 1_048_576; // 1 MiB

/// Maximum in-memory sample count for telemetry rollup.
pub const TELEMETRY_MAX_SAMPLES: usize = 10_000;

/// Telemetry flush cadence in milliseconds.
pub const TELEMETRY_FLUSH_CADENCE_MS: u64 = 5_000;

/// Telemetry snapshot TTL in hours.
pub const TELEMETRY_SNAPSHOT_TTL_HOURS: u64 = 24;

// ---------------------------------------------------------------------------
// DbWriter
// ---------------------------------------------------------------------------

/// DbWriter: the single bounded write gateway for the Rust control plane.
///
/// **Phase 1 skeleton.** The bounded MPSC executor loop, admission control,
/// coalescing map, starvation watchdog, and heartbeat task are implemented in
/// Phase 2 (P075-P2-A1, P075-P2-A2). Until then, `submit` validates deadline
/// constraints and passes through to the pool.
pub struct DbWriter {
    pool: SqlitePool,
    // Phase 2 additions (not yet present):
    // - per-lane MPSC sender/receiver pairs
    // - coalescing map for Class B
    // - heartbeat task handle
    // - shutdown signal sender
    // - starvation watchdog handle
}

impl DbWriter {
    /// Create a new DbWriter backed by the given pool.
    ///
    /// **Phase 1**: no executor loop is started. Phase 2 wires the actual
    /// executor, channels, coalescing map, and heartbeat task.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Submit a write operation.
    ///
    /// **Phase 1: returns WriteRejected{reason: "phase_1_unimplemented"} in production.**
    /// This method is production-visible so that accidental callers fail loudly at runtime
    /// rather than receiving a fabricated `Committed` (P075-SEC-003 / CLEAN-001). Phase 2
    /// replaces the body with bounded MPSC lane routing.
    ///
    /// Validates deadline constraints (LIFT-REL-09) first; returns a deadline-specific
    /// WriteRejected before the phase_1_unimplemented sentinel when the deadline policy
    /// is violated.
    pub async fn submit(&self, op: WriteOperation) -> WriteResult {
        if let Err(rejected) = op.validate() {
            return rejected;
        }

        tracing::debug!(
            operation_name = op.operation_name,
            lane = op.lane.as_str(),
            class = op.class.as_str(),
            idempotency_key = %op.idempotency_key,
            "DbWriter.submit (phase-1 stub: WriteRejected phase_1_unimplemented)"
        );

        WriteResult::WriteRejected {
            lane: op.lane.as_str(),
            capacity: op.lane.capacity(),
            queued_depth: 0,
            oldest_queued_ms: 0,
            operation_name: op.operation_name,
            reason: "phase_1_unimplemented",
        }
    }

    /// Expose pool for callers that own a separate read path.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

// ---------------------------------------------------------------------------
// Lane priority ordering (used in Phase 2 executor)
// ---------------------------------------------------------------------------

/// Ordered list of lanes from highest to lowest priority.
/// Phase 2 executor polls in this order.
pub const LANE_DRAIN_ORDER: &[WriteLane] = &[
    WriteLane::CriticalBarrier,
    WriteLane::OperatorCommand,
    WriteLane::ProjectionInvalidation,
    WriteLane::CoalescedProjection,
    WriteLane::EvidenceMetadata,
    WriteLane::TelemetryRollup,
];

/// Lanes that must be polled before all others regardless of depth.
pub const HIGH_PRIORITY_LANES: &[WriteLane] =
    &[WriteLane::CriticalBarrier, WriteLane::OperatorCommand];

// ---------------------------------------------------------------------------
// Coalescing key
// ---------------------------------------------------------------------------

/// Key for Class B coalescing map.
///
/// For projection invalidations: `(run_id, surface, projection_kind)`.
/// For runtime status and session health: producer-specific stable keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoalescingKey {
    pub run_id: String,
    pub surface: String,
    pub projection_kind: String,
}

impl CoalescingKey {
    pub fn new(run_id: impl Into<String>, surface: impl Into<String>, projection_kind: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            surface: surface.into(),
            projection_kind: projection_kind.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_class::{WriteClass, ReplayPolicy, WriteOperation, WriteResult};
    use std::time::Duration;

    // Regression test: DbWriter::submit is production-visible (CLEAN-001).
    // Phase 1 returns WriteRejected{reason="phase_1_unimplemented"} so accidental
    // production callers fail loudly rather than fabricating a Committed.
    /// CLEAN-002 / MAJOR-006 regression sentinel.
    ///
    /// Phase 1: `SHUTDOWN_ADMITTED_OPERATIONS` is intentionally empty — the shutdown
    /// admission filter is not yet wired. This test documents the Phase 1 state.
    ///
    /// When Phase 2 wires the admission filter:
    ///   1. Populate `SHUTDOWN_ADMITTED_OPERATIONS` with at least the terminal canonical
    ///      state operation names (e.g. "run_completion", "stage_completion").
    ///   2. Change this assertion from `is_empty()` to `!is_empty()`.
    ///   3. Add `shutdown_class_a_admission_filter_rejects_unlisted_ops` to verify that
    ///      an operation not in the list receives `WriteRejected("shutdown_admission_denied")`.
    ///
    /// Leaving the list empty while the filter is active would reject ALL Class A writes
    /// on the shutdown signal — including terminal canonical state persistence.
    #[test]
    fn shutdown_class_a_admission_empty_list_rejects_all_phase1_sentinel() {
        assert!(
            SHUTDOWN_ADMITTED_OPERATIONS.is_empty(),
            "Phase 1 sentinel: SHUTDOWN_ADMITTED_OPERATIONS must be empty until Phase 2 \
             wires the admission filter. If you are adding entries here, also update this \
             test to assert !is_empty() and wire the filter (CLEAN-002)."
        );
    }

    #[test]
    fn dbwriter_submit_is_production_visible() {
        // Confirm the method is callable from both test and production contexts.
        // Phase 2 removes this assertion when submit returns Committed for real writes.
        let _ = DbWriter::submit; // method exists and is accessible
    }

    #[test]
    fn dbwriter_constants_consistent() {
        // Shutdown budgets are positive.
        assert!(SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS > 0);
        assert!(SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS > 0);
        // WAL thresholds are ordered.
        assert!(WARN_WAL_SIZE_BYTES < CRITICAL_WAL_SIZE_BYTES);
        assert!(CRITICAL_WAL_SIZE_BYTES < HARD_WAL_UPPER_BOUND_BYTES);
        // Telemetry budget.
        assert!(TELEMETRY_MEMORY_CAP_BYTES > 0);
        assert!(TELEMETRY_MAX_SAMPLES > 0);
    }

    #[test]
    fn lane_drain_order_covers_all_lanes() {
        use std::collections::HashSet;
        let in_order: HashSet<_> = LANE_DRAIN_ORDER.iter().collect();
        let all_lanes = [
            WriteLane::CriticalBarrier,
            WriteLane::OperatorCommand,
            WriteLane::ProjectionInvalidation,
            WriteLane::CoalescedProjection,
            WriteLane::EvidenceMetadata,
            WriteLane::TelemetryRollup,
        ];
        for lane in &all_lanes {
            assert!(in_order.contains(lane), "Lane {:?} missing from LANE_DRAIN_ORDER", lane);
        }
        assert_eq!(LANE_DRAIN_ORDER.len(), all_lanes.len());
    }

    #[test]
    fn high_priority_lanes_are_subset_of_drain_order() {
        for lane in HIGH_PRIORITY_LANES {
            assert!(
                LANE_DRAIN_ORDER.contains(lane),
                "{:?} in HIGH_PRIORITY_LANES but not in LANE_DRAIN_ORDER",
                lane
            );
        }
    }

    #[tokio::test]
    async fn dbwriter_submit_deadline_exceeds_policy_is_rejected() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_deadline_policy",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: Duration::from_millis(20_000), // exceeds 5_000 ms policy
            idempotency_key: "k".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        let result = writer.submit(op).await;
        assert!(
            matches!(result, WriteResult::WriteRejected { reason: "deadline_exceeds_policy", .. }),
            "expected WriteRejected(deadline_exceeds_policy), got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn dbwriter_submit_phase1_returns_rejected_unimplemented() {
        // Phase 1: submit returns WriteRejected{phase_1_unimplemented} for valid ops
        // so accidental production callers fail loudly. Phase 2 changes this to Committed.
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_barrier_op",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: WriteClass::A.default_deadline(),
            idempotency_key: "run-1/stage-1".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        let result = writer.submit(op).await;
        assert!(
            matches!(result, WriteResult::WriteRejected { reason: "phase_1_unimplemented", .. }),
            "Phase 1 submit must return WriteRejected(phase_1_unimplemented), got {:?}",
            result
        );
    }
}
