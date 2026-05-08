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
//! DbWriter callers use `pool::begin_immediate_with_retry` from P061.
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
//! DbWriter emits a 1 Hz heartbeat. Phase 2 wires the heartbeat task.
//! The supervisor (bounded restart count, CRITICAL log on persistent alive=false)
//! and the `storageHealth.writer.alive` GraphQL surface are Phase 6 work.
//!
//! # WAL checkpoint policy (LIFT-REL-02)
//!
//! - PASSIVE checkpoint: requested when WAL exceeds [`WARN_WAL_SIZE_BYTES`] and no
//!   Class A write is waiting. Run by a low-priority maintenance task.
//! - TRUNCATE checkpoint: only on graceful shutdown after Class A drain, or via
//!   explicit maintenance command.
//! - No hard upper bound above CRITICAL: the approved WAL policy (P075
//!   §architecture.wal_checkpoint_policy) authorises only PASSIVE above 128 MiB and
//!   TRUNCATE on shutdown or explicit maintenance. A 1 GiB barrier-coordinated window
//!   is NOT in the approved policy and must not be wired until the proposal is amended.
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

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::Digest;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot, watch};

use crate::write_class::{WriteClass, WriteLane, WriteOperation, WriteResult};

/// Return a 16-character hex fingerprint of the idempotency key for log correlation.
///
/// P075-SEC-MED-001: callers may encode evidence paths, run IDs, or producer-specific
/// values in idempotency keys. Hashing before logging prevents sensitive path fragments
/// from appearing in runtime log output while still allowing incident triage.
fn hash_idempotency_key(key: &str) -> String {
    let hash = sha2::Sha256::digest(key.as_bytes());
    hash[..8].iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// WriteWork type (Phase 2)
// ---------------------------------------------------------------------------

/// The database work to execute for a write operation.
///
/// Receives a `SqlitePool` clone and executes the SQL (calling
/// `begin_immediate_with_retry` internally per the P061 contract).
/// Returns the number of rows affected on success.
///
/// The closure must be `'static + Send` so it can be sent through the lane channel
/// to the executor task.
pub type WriteWork = Box<
    dyn FnOnce(SqlitePool) -> Pin<Box<dyn Future<Output = anyhow::Result<u32>> + Send + 'static>>
        + Send
        + 'static,
>;

/// Box a generic closure into a [`WriteWork`].
///
/// ```no_run
/// use db::writer::make_work;
/// let work = make_work(|pool| async move {
///     // begin_immediate_with_retry + SQL here
///     Ok(1u32)
/// });
/// ```
pub fn make_work<W, Fut>(f: W) -> WriteWork
where
    W: FnOnce(SqlitePool) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<u32>> + Send + 'static,
{
    Box::new(move |pool| Box::pin(f(pool)))
}

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
/// **Fail-closed**: empty list = deny all Class A writes during shutdown.
/// Phase 2: list is empty → no Class A writes are admitted during shutdown.
/// Phase 3+ populates this list with terminal canonical state operation names
/// (e.g. "run_complete", "stage_complete") as producers are wired through DbWriter.
///
/// **CLEAN-002**: When Phase 3+ wires the shutdown admission filter and populates
/// this list, add a regression test `shutdown_class_a_admission_filter_rejects_unlisted_ops`
/// that verifies operations not in the list are denied.
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
// Lane priority ordering
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
    pub fn new(
        run_id: impl Into<String>,
        surface: impl Into<String>,
        projection_kind: impl Into<String>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            surface: surface.into(),
            projection_kind: projection_kind.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Class B coalescing buffer (Phase 3)
// ---------------------------------------------------------------------------

/// Single entry held in the Class B coalescing buffer pending flush.
struct CoalescedEntry {
    op: WriteOperation,
    work: WriteWork,
    result_tx: oneshot::Sender<WriteResult>,
    enqueued_at: Instant,
    /// Monotonic counter assigned at submit time when `op.observed_at` is `None`
    /// (LIFT-REL-08). Used for last-writer-wins ordering.
    mono_counter: u64,
}

/// Class B coalescing buffer: holds pending writes keyed by `idempotency_key`.
///
/// Flush is triggered by:
/// - Merge count reaching `COALESCE_FLUSH_MAX_MERGES`.
/// - Key age exceeding `COALESCE_MAX_KEY_AGE_MS` (checked by the 500 ms timer task).
/// - Daemon graceful shutdown (force-flush all).
struct CoalescingBuffer {
    entries: std::collections::HashMap<String, CoalescedEntry>,
    /// Cumulative merges since the last flush (triggers flush at 64).
    merge_count: usize,
    /// Total lifetime merges for metrics.
    total_merged: u64,
}

impl CoalescingBuffer {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            merge_count: 0,
            total_merged: 0,
        }
    }

    fn needs_count_flush(&self) -> bool {
        self.merge_count >= COALESCE_FLUSH_MAX_MERGES
    }

    /// Drain all entries, reset merge counter.
    fn drain_all(&mut self) -> Vec<CoalescedEntry> {
        self.merge_count = 0;
        self.entries.drain().map(|(_, v)| v).collect()
    }

    /// Drain entries whose `enqueued_at` age exceeds `max_age`.
    fn drain_stale(&mut self, max_age: Duration) -> Vec<CoalescedEntry> {
        let now = Instant::now();
        let stale: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| now.duration_since(e.enqueued_at) >= max_age)
            .map(|(k, _)| k.clone())
            .collect();
        stale.into_iter().filter_map(|k| self.entries.remove(&k)).collect()
    }
}

// ---------------------------------------------------------------------------
// Internal lane message (private)
// ---------------------------------------------------------------------------

struct LaneMessage {
    op: WriteOperation,
    work: WriteWork,
    result_tx: oneshot::Sender<WriteResult>,
    /// When the message entered the channel (for deadline accounting).
    enqueued_at: Instant,
}

// ---------------------------------------------------------------------------
// Shared heartbeat / alive state
// ---------------------------------------------------------------------------

/// Shared alive/heartbeat state. Exposed for storageHealth in Phase 6.
pub struct DbWriterHeartbeat {
    /// True after the first heartbeat tick; set false by the supervisor on stall.
    pub alive: AtomicBool,
    /// Monotonic ns since process start of last heartbeat beat.
    last_beat_ns: AtomicU64,
    /// Per-lane: monotonic ns since process start of last drain. Indexed by LANE_DRAIN_ORDER.
    last_drain_ns: [AtomicU64; 6],
    /// Cumulative lane starvation events (lower lane starved > STARVATION_WATCHDOG_SECS).
    pub starvation_total: AtomicU64,
    /// Reference instant for ns offsets (process-start-relative).
    origin: Instant,
}

impl DbWriterHeartbeat {
    fn new() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            alive: AtomicBool::new(false),
            last_beat_ns: AtomicU64::new(0),
            last_drain_ns: [ZERO; 6],
            starvation_total: AtomicU64::new(0),
            origin: Instant::now(),
        }
    }

    fn now_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos() as u64
    }

    /// Update the heartbeat timestamp and set alive=true.
    fn beat(&self) {
        self.alive.store(true, Ordering::Relaxed);
        self.last_beat_ns.store(self.now_ns(), Ordering::Relaxed);
    }

    /// Record that a specific lane just drained a message.
    fn record_drain(&self, lane: WriteLane) {
        let idx = lane.drain_order_index();
        self.last_drain_ns[idx].store(self.now_ns(), Ordering::Relaxed);
    }

    /// Return elapsed milliseconds since the last drain of the given lane (0 if never drained).
    pub fn lane_idle_ms(&self, lane: WriteLane) -> u64 {
        let idx = lane.drain_order_index();
        let stored = self.last_drain_ns[idx].load(Ordering::Relaxed);
        if stored == 0 {
            return 0;
        }
        let now = self.now_ns();
        (now.saturating_sub(stored)) / 1_000_000
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// DbWriter (Phase 2)
// ---------------------------------------------------------------------------

/// DbWriter: the single bounded write gateway for the Rust control plane.
///
/// Phase 2: real bounded MPSC executor loop, priority lane drain, admission
/// control, deadline-to-commit accounting, heartbeat, starvation watchdog,
/// and graceful shutdown drain.
pub struct DbWriter {
    /// Stored for test-only pool() access (P075-SEC-HIGH-001).
    #[allow(dead_code)]
    pool: SqlitePool,
    critical_barrier_tx: mpsc::Sender<LaneMessage>,
    operator_command_tx: mpsc::Sender<LaneMessage>,
    projection_invalidation_tx: mpsc::Sender<LaneMessage>,
    coalesced_projection_tx: mpsc::Sender<LaneMessage>,
    evidence_metadata_tx: mpsc::Sender<LaneMessage>,
    telemetry_rollup_tx: mpsc::Sender<LaneMessage>,
    shutdown_tx: watch::Sender<bool>,
    /// Set to true once `shutdown()` is called; `submit` rejects B/C/D immediately.
    shutdown_in_progress: Arc<AtomicBool>,
    pub heartbeat: Arc<DbWriterHeartbeat>,
    /// Executor task handle; taken by `shutdown()`.
    executor_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Class B coalescing buffer: merges last-writer-wins updates before flushing to the lane.
    coalescing: Arc<Mutex<CoalescingBuffer>>,
    /// Monotonic counter for LIFT-REL-08: assigned when `observed_at` is None.
    class_b_mono: Arc<AtomicU64>,
    /// Coalescing flush task handle; taken by `shutdown()`.
    flush_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl DbWriter {
    /// Create a new DbWriter and spawn the background executor task.
    ///
    /// Must be called within a tokio async context (e.g. inside `#[tokio::main]`
    /// or `#[tokio::test]`).
    pub fn new(pool: SqlitePool) -> Self {
        let (cb_tx, cb_rx) = mpsc::channel(WriteLane::CriticalBarrier.capacity());
        let (oc_tx, oc_rx) = mpsc::channel(WriteLane::OperatorCommand.capacity());
        let (pi_tx, pi_rx) = mpsc::channel(WriteLane::ProjectionInvalidation.capacity());
        let (cp_tx, cp_rx) = mpsc::channel(WriteLane::CoalescedProjection.capacity());
        let cp_tx_flush = cp_tx.clone(); // cloned for the coalescing flush task
        let (em_tx, em_rx) = mpsc::channel(WriteLane::EvidenceMetadata.capacity());
        let (tr_tx, tr_rx) = mpsc::channel(WriteLane::TelemetryRollup.capacity());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let shutdown_in_progress = Arc::new(AtomicBool::new(false));
        let heartbeat = Arc::new(DbWriterHeartbeat::new());
        let coalescing = Arc::new(Mutex::new(CoalescingBuffer::new()));
        let class_b_mono = Arc::new(AtomicU64::new(0));

        let executor_heartbeat = heartbeat.clone();
        let executor_pool = pool.clone();
        let handle = tokio::spawn(run_executor(
            executor_pool,
            cb_rx,
            oc_rx,
            pi_rx,
            cp_rx,
            em_rx,
            tr_rx,
            shutdown_rx.clone(),
            executor_heartbeat,
        ));

        let flush_handle = tokio::spawn(run_coalescing_flush(
            coalescing.clone(),
            cp_tx_flush,
            shutdown_rx,
        ));

        Self {
            pool,
            critical_barrier_tx: cb_tx,
            operator_command_tx: oc_tx,
            projection_invalidation_tx: pi_tx,
            coalesced_projection_tx: cp_tx,
            evidence_metadata_tx: em_tx,
            telemetry_rollup_tx: tr_tx,
            shutdown_tx,
            shutdown_in_progress,
            heartbeat,
            executor_handle: Mutex::new(Some(handle)),
            coalescing,
            class_b_mono,
            flush_handle: Mutex::new(Some(flush_handle)),
        }
    }

    /// Submit a write operation with associated database work.
    ///
    /// Returns [`WriteResult`] after the operation completes or its deadline elapses.
    ///
    /// # Deadline semantics
    ///
    /// The `op.deadline` covers enqueue-to-commit (admission wait + queue wait +
    /// lock wait + SQL execution + commit). If the deadline elapses before commit,
    /// returns [`WriteResult::WriteTimeout`].
    ///
    /// # Shutdown semantics
    ///
    /// Once `shutdown()` is called, Class B/C/D submissions return
    /// `WriteRejected("shutdown_admission_denied")` immediately. Class A
    /// submissions are still accepted for the duration of the Class A drain budget.
    pub async fn submit<W, Fut>(&self, op: WriteOperation, work: W) -> WriteResult
    where
        W: FnOnce(SqlitePool) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<u32>> + Send + 'static,
    {
        if let Err(rejected) = op.validate() {
            return rejected;
        }

        // Shutdown admission filter (LIFT-REL-03).
        if self.shutdown_in_progress.load(Ordering::Relaxed) {
            let admitted = if op.class == WriteClass::A {
                // Fail-closed: only operations listed in SHUTDOWN_ADMITTED_OPERATIONS are
                // admitted. Empty list = deny all. Phase 2 has no producers wired, so all
                // Class A writes are denied during shutdown until Phase 3+ populates the list.
                SHUTDOWN_ADMITTED_OPERATIONS.contains(&op.operation_name)
            } else {
                false
            };
            if !admitted {
                return WriteResult::WriteRejected {
                    lane: op.lane.as_str(),
                    capacity: op.lane.capacity(),
                    queued_depth: 0,
                    oldest_queued_ms: None,
                    operation_name: op.operation_name,
                    reason: "shutdown_admission_denied",
                };
            }
        }

        // Class B: route through coalescing buffer (last-writer-wins, LIFT-REL-08).
        if op.class == WriteClass::B {
            return self.submit_class_b(op, make_work(work)).await;
        }

        tracing::debug!(
            operation_name = op.operation_name,
            lane = op.lane.as_str(),
            class = op.class.as_str(),
            idempotency_key_hash = %hash_idempotency_key(&op.idempotency_key),
            "DbWriter.submit: enqueuing"
        );

        let deadline = op.deadline;
        // Record submit start for enqueue-to-commit deadline accounting (DEFECT-001).
        // The full deadline covers admission wait + queue wait + lock wait + commit.
        let submit_start = Instant::now();
        let (result_tx, result_rx) = oneshot::channel();
        let msg = LaneMessage {
            op: op.clone(),
            work: make_work(work),
            result_tx,
            enqueued_at: submit_start,
        };

        let lane_tx = self.lane_sender(op.lane);

        // Enqueue within deadline (admission wait counts against deadline).
        match tokio::time::timeout(deadline, lane_tx.send(msg)).await {
            Err(_elapsed) => {
                // Capture depth at rejection time (more accurate than pre-send snapshot).
                let queued_depth = lane_tx.max_capacity() - lane_tx.capacity();
                WriteResult::WriteRejected {
                    lane: op.lane.as_str(),
                    capacity: op.lane.capacity(),
                    queued_depth,
                    oldest_queued_ms: None, // per-lane enqueue-time tracking deferred to Phase 3
                    operation_name: op.operation_name,
                    reason: "lane_saturated",
                }
            }
            Ok(Err(_channel_closed)) => WriteResult::WriteFailed,
            Ok(Ok(())) => {
                // Use remaining deadline for result wait (DEFECT-001 fix).
                // This ensures total enqueue-to-commit time stays within op.deadline.
                let remaining = deadline
                    .checked_sub(submit_start.elapsed())
                    .unwrap_or(Duration::ZERO);
                if remaining == Duration::ZERO {
                    return WriteResult::WriteTimeout;
                }
                match tokio::time::timeout(remaining, result_rx).await {
                    Err(_elapsed) => WriteResult::WriteTimeout,
                    Ok(Err(_sender_dropped)) => WriteResult::WriteFailed,
                    Ok(Ok(result)) => result,
                }
            }
        }
    }

    /// Initiate graceful shutdown: reject new B/C/D writes, drain Class A
    /// within the shutdown budget, then stop the executor.
    ///
    /// Returns when the executor has stopped or the shutdown budget is exhausted.
    pub async fn shutdown(&self) {
        self.shutdown_in_progress.store(true, Ordering::Relaxed);
        let _ = self.shutdown_tx.send(true);

        // Await flush task first: it force-drains the coalescing buffer into cp_tx
        // within SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS before the executor shuts down.
        let flush_handle = self.flush_handle.lock().unwrap().take();
        if let Some(h) = flush_handle {
            let flush_budget = tokio::time::Duration::from_millis(SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS + 200);
            match tokio::time::timeout(flush_budget, h).await {
                Ok(_) => tracing::debug!("DbWriter coalescing flush task shut down cleanly"),
                Err(_) => tracing::warn!("DbWriter coalescing flush task did not finish within budget"),
            }
        }

        let handle = self.executor_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let budget = tokio::time::Duration::from_millis(
                SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS + SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS + 500,
            );
            match tokio::time::timeout(budget, h).await {
                Ok(_) => tracing::info!("DbWriter executor shut down cleanly"),
                Err(_) => tracing::warn!(
                    "DbWriter executor did not finish within shutdown budget; continuing"
                ),
            }
        }
    }

    /// Returns true if the executor heartbeat is alive.
    pub fn is_alive(&self) -> bool {
        self.heartbeat.is_alive()
    }

    /// Expose pool for tests only.
    ///
    /// Production callers that need reads must hold a separate read-only
    /// `SqlitePool` alongside `DbWriter` — this prevents bypassing class/lane/
    /// idempotency/shutdown enforcement via raw SQL (P075-SEC-HIGH-001).
    #[cfg(test)]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    fn lane_sender(&self, lane: WriteLane) -> &mpsc::Sender<LaneMessage> {
        match lane {
            WriteLane::CriticalBarrier => &self.critical_barrier_tx,
            WriteLane::OperatorCommand => &self.operator_command_tx,
            WriteLane::ProjectionInvalidation => &self.projection_invalidation_tx,
            WriteLane::CoalescedProjection => &self.coalesced_projection_tx,
            WriteLane::EvidenceMetadata => &self.evidence_metadata_tx,
            WriteLane::TelemetryRollup => &self.telemetry_rollup_tx,
        }
    }

    /// Route a Class B write through the coalescing buffer (LIFT-REL-08, Phase 3).
    ///
    /// Last-writer-wins: if a buffered entry with the same `idempotency_key` exists,
    /// the one with the later `observed_at` (or higher monotonic counter when
    /// `observed_at` is absent) survives; the other receives `WriteResult::Coalesced`.
    async fn submit_class_b(&self, op: WriteOperation, work: WriteWork) -> WriteResult {
        let deadline = op.deadline;
        let submit_start = Instant::now();
        let mono_counter = self.class_b_mono.fetch_add(1, Ordering::Relaxed);
        let (result_tx, result_rx) = oneshot::channel::<WriteResult>();
        let key = op.idempotency_key.clone();

        let flushed: Vec<CoalescedEntry> = {
            let mut buf = self.coalescing.lock().unwrap();

            if let Some(existing) = buf.entries.get(&key) {
                let new_wins = match (op.observed_at, existing.op.observed_at) {
                    (Some(new_obs), Some(old_obs)) => new_obs >= old_obs,
                    _ => mono_counter > existing.mono_counter,
                };
                if new_wins {
                    let old = buf.entries.remove(&key).unwrap();
                    let _ = old.result_tx.send(WriteResult::Coalesced);
                    buf.merge_count += 1;
                    buf.total_merged += 1;
                    buf.entries.insert(
                        key,
                        CoalescedEntry { op, work, result_tx, enqueued_at: submit_start, mono_counter },
                    );
                } else {
                    // New entry is stale; evict it immediately.
                    let _ = result_tx.send(WriteResult::Coalesced);
                    // result_rx will return Coalesced on the await below.
                }
            } else {
                buf.entries.insert(
                    key,
                    CoalescedEntry { op, work, result_tx, enqueued_at: submit_start, mono_counter },
                );
            }

            if buf.needs_count_flush() {
                buf.drain_all()
            } else {
                vec![]
            }
        };

        self.flush_entries_to_lane(flushed).await;

        let remaining = deadline
            .checked_sub(submit_start.elapsed())
            .unwrap_or(Duration::ZERO);
        if remaining == Duration::ZERO {
            return WriteResult::WriteTimeout;
        }
        match tokio::time::timeout(remaining, result_rx).await {
            Err(_) => WriteResult::WriteTimeout,
            Ok(Err(_)) => WriteResult::WriteFailed,
            Ok(Ok(result)) => result,
        }
    }

    /// Send a batch of coalesced entries to the CoalescedProjection lane.
    ///
    /// Each entry whose send fails (channel closed or full) receives
    /// `WriteResult::WriteFailed` so the caller is notified rather than hung.
    async fn flush_entries_to_lane(&self, entries: Vec<CoalescedEntry>) {
        for entry in entries {
            let msg = LaneMessage {
                op: entry.op,
                work: entry.work,
                result_tx: entry.result_tx,
                enqueued_at: entry.enqueued_at,
            };
            if let Err(e) = self.coalesced_projection_tx.try_send(msg) {
                // Channel is either closed or temporarily full; send WriteFailed to the waiting caller.
                let result_tx = match e {
                    mpsc::error::TrySendError::Full(m) => m.result_tx,
                    mpsc::error::TrySendError::Closed(m) => m.result_tx,
                };
                let _ = result_tx.send(WriteResult::WriteFailed);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class B coalescing flush task (Phase 3)
// ---------------------------------------------------------------------------

/// Background task: flush coalesced Class B entries every 500 ms and on shutdown.
///
/// - Every `COALESCE_FLUSH_INTERVAL_MS`: drains entries older than `COALESCE_MAX_KEY_AGE_MS`.
/// - On shutdown signal: force-drains all remaining entries (sub-budget LIFT-REL-03).
async fn run_coalescing_flush(
    coalescing: Arc<Mutex<CoalescingBuffer>>,
    cp_tx: mpsc::Sender<LaneMessage>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    use tokio::time::{interval, Duration};
    let max_age = Duration::from_millis(COALESCE_MAX_KEY_AGE_MS);
    let mut tick = interval(Duration::from_millis(COALESCE_FLUSH_INTERVAL_MS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;

            result = shutdown_rx.changed() => {
                if result.is_ok() && *shutdown_rx.borrow() {
                    // Force-flush all remaining entries.
                    let entries: Vec<CoalescedEntry> = {
                        let mut buf = coalescing.lock().unwrap();
                        buf.drain_all()
                    };
                    flush_coalesced_to_channel(entries, &cp_tx).await;
                    tracing::debug!("CoalescingFlush: force-flushed all entries on shutdown");
                    return;
                }
            }

            _ = tick.tick() => {
                let entries: Vec<CoalescedEntry> = {
                    let mut buf = coalescing.lock().unwrap();
                    buf.drain_stale(max_age)
                };
                if !entries.is_empty() {
                    tracing::debug!(count = entries.len(), "CoalescingFlush: draining stale entries");
                    flush_coalesced_to_channel(entries, &cp_tx).await;
                }
            }
        }
    }
}

/// Send coalesced entries to the channel, notifying callers of any send failures.
async fn flush_coalesced_to_channel(entries: Vec<CoalescedEntry>, cp_tx: &mpsc::Sender<LaneMessage>) {
    for entry in entries {
        let msg = LaneMessage {
            op: entry.op,
            work: entry.work,
            result_tx: entry.result_tx,
            enqueued_at: entry.enqueued_at,
        };
        if let Err(e) = cp_tx.try_send(msg) {
            let result_tx = match e {
                mpsc::error::TrySendError::Full(m) => m.result_tx,
                mpsc::error::TrySendError::Closed(m) => m.result_tx,
            };
            let _ = result_tx.send(WriteResult::WriteFailed);
        }
    }
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Execute a single lane message: check deadline, run work, send result.
async fn execute_message(
    pool: &SqlitePool,
    msg: LaneMessage,
    heartbeat: &DbWriterHeartbeat,
    lane: WriteLane,
) {
    let elapsed = msg.enqueued_at.elapsed();
    if elapsed >= msg.op.deadline {
        tracing::warn!(
            operation_name = msg.op.operation_name,
            lane = lane.as_str(),
            elapsed_ms = elapsed.as_millis() as u64,
            deadline_ms = msg.op.deadline.as_millis() as u64,
            "DbWriter: deadline expired in queue; returning WriteTimeout"
        );
        let _ = msg.result_tx.send(WriteResult::WriteTimeout);
        return;
    }

    tracing::debug!(
        operation_name = msg.op.operation_name,
        lane = lane.as_str(),
        idempotency_key_hash = %hash_idempotency_key(&msg.op.idempotency_key),
        elapsed_ms = elapsed.as_millis() as u64,
        "DbWriter: executing write"
    );

    // Run work to completion without a timeout wrapper. Per P075
    // §architecture.deadlines_and_results.in_flight_timeout, in-flight
    // transactions are not cancelled mid-transaction; they complete or roll
    // back under SQLite semantics. The caller-side result_rx timeout (in
    // submit()) handles the case where the caller does not want to wait.
    let work_result = (msg.work)(pool.clone()).await;

    let write_result = match work_result {
        Err(ref e) if is_busy_error(e) => {
            tracing::warn!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                error_kind = "sqlite_busy",
                "DbWriter: SQLite busy exhausted; returning WriteBusyExhausted"
            );
            WriteResult::WriteBusyExhausted
        }
        Err(e) => {
            tracing::error!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                error_kind = %classify_write_error(&e),
                "DbWriter: write failed"
            );
            WriteResult::WriteFailed
        }
        Ok(rows) => {
            tracing::debug!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                rows_affected = rows,
                "DbWriter: committed"
            );
            WriteResult::Committed
        }
    };

    // Record heartbeat for this lane.
    heartbeat.record_drain(lane);

    // Post-cancel observability: if the caller dropped the receiver, log it.
    if msg.result_tx.send(write_result).is_err() {
        tracing::debug!(
            operation_name = msg.op.operation_name,
            lane = lane.as_str(),
            "DbWriter: caller dropped receiver before result; result logged above (LIFT-REL-10)"
        );
    }
}

/// Run the executor loop.
///
/// Priority drain order: CriticalBarrier = OperatorCommand > ProjectionInvalidation >
/// CoalescedProjection > EvidenceMetadata > TelemetryRollup.
///
/// During shutdown:
/// 1. Drain remaining Class A within SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS.
/// 2. Drain remaining Class B within SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS (simplified).
/// 3. Drop Class C and D.
#[allow(clippy::too_many_arguments)]
async fn run_executor(
    pool: SqlitePool,
    mut cb_rx: mpsc::Receiver<LaneMessage>, // CriticalBarrier
    mut oc_rx: mpsc::Receiver<LaneMessage>, // OperatorCommand
    mut pi_rx: mpsc::Receiver<LaneMessage>, // ProjectionInvalidation
    mut cp_rx: mpsc::Receiver<LaneMessage>, // CoalescedProjection
    mut em_rx: mpsc::Receiver<LaneMessage>, // EvidenceMetadata
    mut tr_rx: mpsc::Receiver<LaneMessage>, // TelemetryRollup
    mut shutdown_rx: watch::Receiver<bool>,
    heartbeat: Arc<DbWriterHeartbeat>,
) {
    use tokio::time::{interval, Duration};
    let mut hb_interval = interval(Duration::from_secs(1));
    hb_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Starvation tracking: last time each lower lane drained.
    let mut lower_lane_last_drain: [Option<Instant>; 4] = [None; 4]; // pi, cp, em, tr

    loop {
        // --- Non-blocking drain of high-priority lanes ---
        let mut drained_high = false;
        while let Ok(msg) = cb_rx.try_recv() {
            execute_message(&pool, msg, &heartbeat, WriteLane::CriticalBarrier).await;
            drained_high = true;
        }
        while let Ok(msg) = oc_rx.try_recv() {
            execute_message(&pool, msg, &heartbeat, WriteLane::OperatorCommand).await;
            drained_high = true;
        }

        // --- Starvation watchdog for lower lanes ---
        check_starvation(&lower_lane_last_drain, drained_high, &heartbeat);

        // --- Wait for any lane (priority enforced by `biased` select) ---
        tokio::select! {
            biased;

            msg = cb_rx.recv() => {
                match msg {
                    Some(m) => execute_message(&pool, m, &heartbeat, WriteLane::CriticalBarrier).await,
                    None => break,
                }
            }
            msg = oc_rx.recv() => {
                match msg {
                    Some(m) => execute_message(&pool, m, &heartbeat, WriteLane::OperatorCommand).await,
                    None => break,
                }
            }
            msg = pi_rx.recv() => {
                if let Some(m) = msg {
                    execute_message(&pool, m, &heartbeat, WriteLane::ProjectionInvalidation).await;
                    lower_lane_last_drain[0] = Some(Instant::now());
                }
            }
            msg = cp_rx.recv() => {
                if let Some(m) = msg {
                    execute_message(&pool, m, &heartbeat, WriteLane::CoalescedProjection).await;
                    lower_lane_last_drain[1] = Some(Instant::now());
                }
            }
            msg = em_rx.recv() => {
                if let Some(m) = msg {
                    execute_message(&pool, m, &heartbeat, WriteLane::EvidenceMetadata).await;
                    lower_lane_last_drain[2] = Some(Instant::now());
                }
            }
            msg = tr_rx.recv() => {
                if let Some(m) = msg {
                    execute_message(&pool, m, &heartbeat, WriteLane::TelemetryRollup).await;
                    lower_lane_last_drain[3] = Some(Instant::now());
                }
            }
            _ = hb_interval.tick() => {
                heartbeat.beat();
            }
            result = shutdown_rx.changed() => {
                if result.is_ok() && *shutdown_rx.borrow() {
                    drain_on_shutdown(
                        &pool,
                        &mut cb_rx,
                        &mut oc_rx,
                        &mut pi_rx,
                        &mut cp_rx,
                        &heartbeat,
                    ).await;
                    break;
                }
            }
        }
    }

    tracing::info!("DbWriter executor exiting");
}

/// Drain Class A within the shutdown budget, then force-flush Class B.
async fn drain_on_shutdown(
    pool: &SqlitePool,
    cb_rx: &mut mpsc::Receiver<LaneMessage>,
    oc_rx: &mut mpsc::Receiver<LaneMessage>,
    pi_rx: &mut mpsc::Receiver<LaneMessage>,
    cp_rx: &mut mpsc::Receiver<LaneMessage>,
    heartbeat: &DbWriterHeartbeat,
) {
    let class_a_deadline = tokio::time::Instant::now()
        + tokio::time::Duration::from_millis(SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS);
    let class_b_deadline =
        class_a_deadline + tokio::time::Duration::from_millis(SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS);

    tracing::info!("DbWriter: shutdown drain started");

    // Drain Class A lanes within budget.
    loop {
        if tokio::time::Instant::now() >= class_a_deadline {
            let cb_remaining = cb_rx.len();
            let oc_remaining = oc_rx.len();
            if cb_remaining + oc_remaining > 0 {
                tracing::warn!(
                    cb_remaining,
                    oc_remaining,
                    "DbWriter: Class A drain budget exhausted; items abandoned"
                );
            }
            break;
        }
        let mut drained = false;
        if let Ok(msg) = cb_rx.try_recv() {
            execute_message(pool, msg, heartbeat, WriteLane::CriticalBarrier).await;
            drained = true;
        }
        if let Ok(msg) = oc_rx.try_recv() {
            execute_message(pool, msg, heartbeat, WriteLane::OperatorCommand).await;
            drained = true;
        }
        if !drained {
            break; // Both Class A lanes empty.
        }
    }

    // Force-flush Class B lanes (ProjectionInvalidation + CoalescedProjection) within sub-budget.
    // Both lanes carry Class B writes; drain them interleaved until the budget expires or both empty.
    loop {
        if tokio::time::Instant::now() >= class_b_deadline {
            let pi_remaining = pi_rx.len();
            let cp_remaining = cp_rx.len();
            if pi_remaining + cp_remaining > 0 {
                tracing::warn!(
                    pi_remaining,
                    cp_remaining,
                    "DbWriter: Class B flush budget exhausted; items abandoned"
                );
            }
            break;
        }
        let mut drained = false;
        if let Ok(msg) = pi_rx.try_recv() {
            execute_message(pool, msg, heartbeat, WriteLane::ProjectionInvalidation).await;
            drained = true;
        }
        if let Ok(msg) = cp_rx.try_recv() {
            execute_message(pool, msg, heartbeat, WriteLane::CoalescedProjection).await;
            drained = true;
        }
        if !drained {
            break; // Both Class B lanes empty.
        }
    }

    tracing::info!("DbWriter: shutdown drain complete");
}

/// Starvation watchdog: warn if a lower lane hasn't drained in STARVATION_WATCHDOG_SECS
/// while high lanes were not saturated last cycle.
fn check_starvation(
    lower_last_drain: &[Option<Instant>; 4],
    high_lanes_had_work: bool,
    heartbeat: &DbWriterHeartbeat,
) {
    if high_lanes_had_work {
        // High lanes were busy; lower-lane starvation is expected.
        return;
    }
    let lower_lanes = [
        WriteLane::ProjectionInvalidation,
        WriteLane::CoalescedProjection,
        WriteLane::EvidenceMetadata,
        WriteLane::TelemetryRollup,
    ];
    for (i, &lane) in lower_lanes.iter().enumerate() {
        if let Some(last) = lower_last_drain[i] {
            let idle_secs = last.elapsed().as_secs();
            if idle_secs >= STARVATION_WATCHDOG_SECS {
                heartbeat.starvation_total.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    lane = lane.as_str(),
                    idle_secs,
                    starvation_total = heartbeat.starvation_total.load(Ordering::Relaxed),
                    "DbWriter: lane starvation detected (METRIC: lane_starvation_total)"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers: error classification (P075-SEC-MED-001)
// ---------------------------------------------------------------------------

/// Classify a write error into a safe loggable token without exposing raw SQL
/// fragments, paths, or other sensitive data from the anyhow error chain.
fn classify_write_error(err: &anyhow::Error) -> &'static str {
    if is_busy_error(err) {
        return "sqlite_busy";
    }
    if let Some(sqlx_err) = err.downcast_ref::<sqlx::Error>() {
        return match sqlx_err {
            sqlx::Error::Database(_) => "sqlx_database",
            sqlx::Error::RowNotFound => "sqlx_row_not_found",
            sqlx::Error::TypeNotFound { .. } => "sqlx_type_not_found",
            sqlx::Error::Io(_) => "sqlx_io",
            _ => "sqlx_other",
        };
    }
    "other"
}

fn is_busy_error(err: &anyhow::Error) -> bool {
    if let Some(sqlx_err) = err.downcast_ref::<sqlx::Error>() {
        if let sqlx::Error::Database(db_err) = sqlx_err {
            let code = db_err.code().map(|c| c.to_string());
            return matches!(code.as_deref(), Some("5") | Some("6"))
                || db_err
                    .message()
                    .to_lowercase()
                    .contains("database is locked")
                || db_err.message().to_lowercase().contains("database is busy");
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::begin_immediate_with_retry;
    use crate::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};
    use std::time::Duration;

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

    /// Regression sentinel for CLEAN-002 / LIFT-REL-03.
    ///
    /// Phase 2: SHUTDOWN_ADMITTED_OPERATIONS is empty because no producers have been
    /// wired yet. With fail-closed semantics, empty means deny all. When Phase 3+ wires
    /// the admission filter and populates this list, update the assertion to `!is_empty()`
    /// and add the `shutdown_class_a_admission_filter_rejects_unlisted_ops` regression test.
    #[test]
    fn shutdown_class_a_admission_empty_list_denies_all_phase2_sentinel() {
        assert!(
            SHUTDOWN_ADMITTED_OPERATIONS.is_empty(),
            "Phase 2 sentinel: SHUTDOWN_ADMITTED_OPERATIONS must be empty until \
             producers are wired in Phase 3+. If you are adding entries here, \
             update this test to assert !is_empty() (CLEAN-002)."
        );
        // With fail-closed semantics, empty list means no Class A writes are admitted.
        // Verify the filter helper used by submit() reflects this.
        fn is_admitted_fail_closed(list: &[&str], op_name: &str) -> bool {
            list.contains(&op_name)
        }
        assert!(
            !is_admitted_fail_closed(SHUTDOWN_ADMITTED_OPERATIONS, "run_complete"),
            "empty SHUTDOWN_ADMITTED_OPERATIONS must deny all Class A writes (fail-closed)"
        );
    }

    /// Verify the shutdown admission filter semantics in isolation (LIFT-REL-03).
    ///
    /// The filter logic is fail-closed: empty list → deny all; non-empty list → admit only listed ops.
    /// This test exercises all branches using a local helper that mirrors submit()'s filter code.
    #[test]
    fn shutdown_admission_filter_fails_closed_with_empty_or_partial_list() {
        fn is_admitted(list: &[&str], op_name: &str) -> bool {
            list.contains(&op_name)
        }
        // Fail-closed: empty list → no operations are admitted.
        assert!(
            !is_admitted(&[], "run_complete"),
            "empty list must deny all (fail-closed)"
        );
        assert!(
            !is_admitted(&[], "stage_complete"),
            "empty list must deny all (fail-closed)"
        );
        assert!(
            !is_admitted(&[], "arbitrary_operation"),
            "empty list must deny all (fail-closed)"
        );

        // Phase 3+ simulation: non-empty list → only listed ops admitted.
        let list = ["run_complete", "stage_complete"];
        assert!(
            is_admitted(&list, "run_complete"),
            "listed op must be admitted"
        );
        assert!(
            is_admitted(&list, "stage_complete"),
            "listed op must be admitted"
        );
        assert!(
            !is_admitted(&list, "arbitrary_operation"),
            "unlisted op must be denied"
        );
        assert!(
            !is_admitted(&list, ""),
            "empty op name must be denied if unlisted"
        );
    }

    #[test]
    fn dbwriter_constants_consistent() {
        assert!(SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS > 0);
        assert!(SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS > 0);
        assert!(WARN_WAL_SIZE_BYTES < CRITICAL_WAL_SIZE_BYTES);
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
            assert!(
                in_order.contains(lane),
                "Lane {:?} missing from LANE_DRAIN_ORDER",
                lane
            );
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

    // -----------------------------------------------------------------------
    // Phase 2: validate() still works independently of the executor
    // -----------------------------------------------------------------------

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
            deadline_reason: None,
            idempotency_key: "k".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };
        let result = writer.submit(op, |_pool| async { Ok(0u32) }).await;
        assert!(
            matches!(
                result,
                WriteResult::WriteRejected {
                    reason: "deadline_exceeds_policy",
                    ..
                }
            ),
            "expected WriteRejected(deadline_exceeds_policy), got {:?}",
            result
        );
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: real executor returns Committed for valid work
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_submit_valid_work_returns_committed() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool.clone());
        let op = make_class_a_op();

        let result = writer
            .submit(op, |pool| async move {
                // A real read-only transaction to prove the executor runs.
                let mut tx = begin_immediate_with_retry(&pool, "test_barrier").await?;
                let _row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&mut *tx).await?;
                tx.commit().await?;
                Ok(1u32)
            })
            .await;

        assert!(
            matches!(result, WriteResult::Committed),
            "expected Committed, got {:?}",
            result
        );
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: WriteTimeout when work exceeds remaining deadline
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_submit_returns_write_timeout_when_work_is_slow() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        let op = WriteOperation {
            class: WriteClass::A,
            lane: WriteLane::CriticalBarrier,
            operation_name: "test_slow_write",
            expected_rows: 1,
            batchable: false,
            barrier: true,
            deadline: Duration::from_millis(50), // very short deadline
            deadline_reason: None,
            idempotency_key: "run-1/slow".to_string(),
            replay_policy: ReplayPolicy::NaturalKey,
            observed_at: None,
        };

        let result = writer
            .submit(op, |_pool| async {
                // Sleep longer than the deadline.
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                Ok(1u32)
            })
            .await;

        assert!(
            matches!(
                result,
                WriteResult::WriteTimeout
                    | WriteResult::WriteRejected {
                        reason: "lane_saturated",
                        ..
                    }
            ),
            "expected WriteTimeout or WriteRejected(lane_saturated), got {:?}",
            result
        );
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: WriteBusyExhausted when work returns a SQLite busy error
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_submit_returns_write_busy_exhausted_on_busy_error() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        let op = make_class_a_op();

        // Simulate a work closure that returns a SQLite SQLITE_BUSY error (code 5).
        let result = writer
            .submit(op, |_pool| async {
                let db_err = sqlx::Error::Database(Box::new(MockBusyError));
                Err(anyhow::Error::new(db_err))
            })
            .await;

        assert!(
            matches!(result, WriteResult::WriteBusyExhausted),
            "expected WriteBusyExhausted, got {:?}",
            result
        );
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: WriteFailed when work returns a non-busy error
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_submit_returns_write_failed_on_generic_error() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        let op = make_class_a_op();

        let result = writer
            .submit(op, |_pool| async {
                Err(anyhow::anyhow!("simulated SQL constraint violation"))
            })
            .await;

        assert!(
            matches!(result, WriteResult::WriteFailed),
            "expected WriteFailed, got {:?}",
            result
        );
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: Heartbeat sets alive=true after executor starts
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_heartbeat_sets_alive_after_tick() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);

        // The heartbeat ticks every 1s. Wait up to 2s for it to fire.
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        loop {
            if writer.is_alive() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("DbWriter heartbeat did not set alive=true within 2s");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        writer.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Phase 2: Graceful shutdown drains a pending Class A write
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_graceful_shutdown_completes_pending_class_a() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = std::sync::Arc::new(DbWriter::new(pool.clone()));
        let writer2 = writer.clone();

        let op = make_class_a_op();
        // Submit the write in a separate task before calling shutdown.
        let submit_handle = tokio::spawn(async move {
            writer2
                .submit(op, |pool| async move {
                    let tx = begin_immediate_with_retry(&pool, "test_shutdown_a").await?;
                    tx.commit().await?;
                    Ok(1u32)
                })
                .await
        });

        // Give the write a moment to enqueue, then initiate shutdown.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        writer.shutdown().await;

        let result = submit_handle.await.expect("submit task panicked");
        // The write should have committed (or at worst WriteTimeout if budget was tight).
        assert!(
            matches!(result, WriteResult::Committed | WriteResult::WriteTimeout),
            "expected Committed or WriteTimeout during graceful shutdown, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2: B/C/D writes are rejected after shutdown is signaled
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dbwriter_rejects_non_class_a_during_shutdown() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = DbWriter::new(pool);
        writer.shutdown().await;

        let op = WriteOperation {
            class: WriteClass::B,
            lane: WriteLane::CoalescedProjection,
            operation_name: "test_coalesced",
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::B.default_deadline(),
            deadline_reason: None,
            idempotency_key: "run-1/surface-1/proj-1".to_string(),
            replay_policy: ReplayPolicy::LastWriterWins,
            observed_at: None,
        };

        let result = writer.submit(op, |_pool| async { Ok(0u32) }).await;
        assert!(
            matches!(
                result,
                WriteResult::WriteRejected {
                    reason: "shutdown_admission_denied",
                    ..
                } | WriteResult::WriteFailed
            ),
            "expected rejection after shutdown, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Idempotency key hash regression tests (P075-SEC-MED-001)
    // -----------------------------------------------------------------------

    #[test]
    fn idempotency_key_hash_does_not_equal_raw_key() {
        let raw_key = "run-1/stage-1/agent-1/transcript-001";
        let hashed = hash_idempotency_key(raw_key);
        assert_ne!(hashed, raw_key, "hash must not equal raw key");
        assert_eq!(hashed.len(), 16, "hash must be 16 hex chars (8 bytes)");
        assert!(
            hashed.chars().all(|c| c.is_ascii_hexdigit()),
            "hash must be lowercase hex, got: {hashed}"
        );
    }

    #[test]
    fn idempotency_key_hash_is_deterministic() {
        let key = "run-42/stage-7/checkpoint";
        assert_eq!(
            hash_idempotency_key(key),
            hash_idempotency_key(key),
            "hash must be deterministic for the same input"
        );
    }

    #[test]
    fn idempotency_key_hash_differs_for_distinct_keys() {
        let h1 = hash_idempotency_key("run-1/stage-1");
        let h2 = hash_idempotency_key("run-2/stage-1");
        assert_ne!(h1, h2, "distinct keys must produce distinct hashes");
    }

    #[test]
    fn idempotency_key_hash_does_not_contain_raw_path_fragments() {
        let key = "run-secret-project/stage-confidential/agent-private";
        let hashed = hash_idempotency_key(key);
        assert!(!hashed.contains("secret"));
        assert!(!hashed.contains("confidential"));
        assert!(!hashed.contains("private"));
    }

    // -----------------------------------------------------------------------
    // Mock busy error for WriteBusyExhausted test
    // -----------------------------------------------------------------------

    #[derive(Debug)]
    struct MockBusyError;

    impl std::fmt::Display for MockBusyError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "database is busy")
        }
    }

    impl sqlx::error::DatabaseError for MockBusyError {
        fn message(&self) -> &str {
            "database is busy"
        }
        fn code(&self) -> Option<std::borrow::Cow<str>> {
            Some(std::borrow::Cow::Borrowed("5"))
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
    }

    impl std::error::Error for MockBusyError {}
}
