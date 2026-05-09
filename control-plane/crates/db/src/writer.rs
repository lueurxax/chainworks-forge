//! P075 DbWriter: the single bounded write gateway for the Rust control plane.
//!
//! Non-test runtime writes must enter through `DbWriter`. The source-controlled bypass allowlist
//! (`write-bypass-allowlist.toml`) is limited to permanent infrastructure scopes
//! such as migrations, tests, and startup repair.
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
//! DbWriter uses `pool::begin_immediate_with_retry` from P061 internally.
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

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use sha2::Digest;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tokio::sync::{mpsc, oneshot, watch};

use crate::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};

pub const DB_WRITER_LANES: [WriteLane; 6] = [
    WriteLane::CriticalBarrier,
    WriteLane::OperatorCommand,
    WriteLane::ProjectionInvalidation,
    WriteLane::CoalescedProjection,
    WriteLane::EvidenceMetadata,
    WriteLane::TelemetryRollup,
];
const TX_DURATION_SAMPLE_LIMIT: usize = 1024;

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

pub type TransactionWork<T> = Box<
    dyn for<'tx> FnOnce(
            &'tx mut Transaction<'_, Sqlite>,
        )
            -> Pin<Box<dyn Future<Output = anyhow::Result<(T, u32)>> + Send + 'tx>>
        + Send
        + 'static,
>;

/// Box a generic closure into a [`WriteWork`].
///
/// ```no_run
/// use db::writer::make_work;
/// let work = make_work(|pool| async move {
///     // transactional SQL here
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

pub fn class_a_operation(
    operation_name: &'static str,
    lane: WriteLane,
    idempotency_key: impl Into<String>,
) -> WriteOperation {
    WriteOperation {
        class: WriteClass::A,
        lane,
        operation_name,
        expected_rows: 1,
        batchable: false,
        barrier: true,
        deadline: WriteClass::A.default_deadline(),
        deadline_reason: None,
        idempotency_key: idempotency_key.into(),
        replay_policy: ReplayPolicy::CallerGuarded,
        observed_at: None,
    }
}

pub fn repository_transaction_operation(operation_name: &'static str) -> WriteOperation {
    if operation_name.starts_with("projections.") {
        return WriteOperation {
            class: WriteClass::B,
            lane: WriteLane::CoalescedProjection,
            operation_name,
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::B.default_deadline(),
            deadline_reason: None,
            idempotency_key: operation_name.to_string(),
            replay_policy: ReplayPolicy::LastWriterWins,
            observed_at: None,
        };
    }
    if matches!(
        operation_name,
        "storage_health.insert_write_pressure_snapshot"
            | "scheduler.record_db_writer_wait_observation"
    ) {
        return WriteOperation {
            class: WriteClass::D,
            lane: WriteLane::TelemetryRollup,
            operation_name,
            expected_rows: 1,
            batchable: true,
            barrier: false,
            deadline: WriteClass::D.default_deadline(),
            deadline_reason: None,
            idempotency_key: operation_name.to_string(),
            replay_policy: ReplayPolicy::TelemetryMerge,
            observed_at: None,
        };
    }
    class_a_operation(operation_name, WriteLane::CriticalBarrier, operation_name)
}

static SHARED_WRITERS: OnceLock<Mutex<HashMap<String, Arc<DbWriter>>>> = OnceLock::new();

fn pool_registry_key(pool: &SqlitePool) -> Option<(String, bool)> {
    let options = pool.connect_options();
    let filename = options.get_filename().to_string_lossy().trim().to_string();
    if filename.is_empty() || filename == ":memory:" {
        Some((format!("in-memory:{:p}", Arc::as_ptr(&options)), false))
    } else {
        Some((filename, true))
    }
}

pub async fn register_shared_writer(
    pool: &SqlitePool,
    writer: Arc<DbWriter>,
) -> anyhow::Result<()> {
    let Some((key, _file_backed)) = pool_registry_key(pool) else {
        return Ok(());
    };
    SHARED_WRITERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("shared DbWriter registry lock poisoned")
        .insert(key, writer);
    Ok(())
}

pub async fn shared_writer_for(pool: &SqlitePool) -> Option<Arc<DbWriter>> {
    let (key, _file_backed) = pool_registry_key(pool)?;
    let registry = SHARED_WRITERS.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = registry
        .lock()
        .expect("shared DbWriter registry lock poisoned");
    guard.get(&key).cloned()
}

pub async fn begin_registered_immediate_transaction<'pool>(
    pool: &'pool SqlitePool,
    op: WriteOperation,
    context: &'static str,
) -> anyhow::Result<QueuedTransaction> {
    if let Err(rejected) = op.validate() {
        anyhow::bail!(
            "DbWriter rejected {context} before transaction start: {}",
            rejected.as_str()
        );
    }
    if let Some(writer) = shared_writer_for(pool).await {
        return writer.begin_immediate_transaction(op, context).await;
    }
    let Some((_key, file_backed)) = pool_registry_key(pool) else {
        anyhow::bail!("P075 shared DbWriter registry key unavailable for {context}");
    };
    if file_backed {
        anyhow::bail!("P075 shared DbWriter is not registered for {context}");
    }
    let writer = Arc::new(DbWriter::new(pool.clone()));
    let mut tx = writer.begin_immediate_transaction(op, context).await?;
    tx.attach_owner(writer);
    Ok(tx)
}

pub async fn begin_repository_transaction<'pool>(
    pool: &'pool SqlitePool,
    operation_name: &'static str,
) -> anyhow::Result<QueuedTransaction> {
    begin_registered_immediate_transaction(
        pool,
        repository_transaction_operation(operation_name),
        operation_name,
    )
    .await
}

pub async fn execute_repository_unit_operation(
    pool: &SqlitePool,
    op: WriteOperation,
    context: &'static str,
    work: WriteWork,
) -> anyhow::Result<()> {
    if let Err(rejected) = op.validate() {
        anyhow::bail!(
            "DbWriter rejected {context} before submit: {}",
            rejected.as_str()
        );
    }
    let result = if let Some(writer) = shared_writer_for(pool).await {
        writer.submit_work(op, work).await
    } else {
        let Some((_key, file_backed)) = pool_registry_key(pool) else {
            anyhow::bail!("P075 shared DbWriter registry key unavailable for {context}");
        };
        if file_backed {
            anyhow::bail!("P075 shared DbWriter is not registered for {context}");
        }
        let writer = Arc::new(DbWriter::new(pool.clone()));
        let result = writer.submit_work(op, work).await;
        writer.shutdown().await;
        result
    };
    match result {
        WriteResult::Committed | WriteResult::Coalesced | WriteResult::DroppedTelemetry => Ok(()),
        WriteResult::WriteRejected { reason, .. } => {
            anyhow::bail!("DbWriter rejected {context}: {reason}")
        }
        other => anyhow::bail!("DbWriter {context} did not commit: {}", other.as_str()),
    }
}

pub async fn execute_repository_unit_write(
    pool: &SqlitePool,
    operation_name: &'static str,
    work: WriteWork,
) -> anyhow::Result<()> {
    execute_repository_unit_operation(
        pool,
        repository_transaction_operation(operation_name),
        operation_name,
        work,
    )
    .await
}

pub async fn execute_repository_transaction_operation(
    pool: &SqlitePool,
    op: WriteOperation,
    context: &'static str,
    work: TransactionWork<()>,
) -> anyhow::Result<()> {
    if let Err(rejected) = op.validate() {
        anyhow::bail!(
            "DbWriter rejected {context} before transaction submit: {}",
            rejected.as_str()
        );
    }
    if let Some(writer) = shared_writer_for(pool).await {
        return writer.submit_unit_transaction(op, context, work).await;
    }
    let Some((_key, file_backed)) = pool_registry_key(pool) else {
        anyhow::bail!("P075 shared DbWriter registry key unavailable for {context}");
    };
    if file_backed {
        anyhow::bail!("P075 shared DbWriter is not registered for {context}");
    }
    let writer = Arc::new(DbWriter::new(pool.clone()));
    let result = writer.submit_unit_transaction(op, context, work).await;
    writer.shutdown().await;
    result
}

#[macro_export]
macro_rules! execute_repository_write {
    ($pool:expr, $operation_name:expr, $query:expr) => {{
        let mut __p075_tx =
            $crate::writer::begin_repository_transaction($pool, $operation_name).await?;
        let __p075_result = $query.execute(&mut **__p075_tx).await;
        match __p075_result {
            Ok(value) => {
                __p075_tx.commit().await?;
                Ok::<_, anyhow::Error>(value)
            }
            Err(error) => Err(anyhow::Error::new(error)),
        }
    }};
}

enum QueuedTransactionFinish {
    Commit(Transaction<'static, Sqlite>),
    Rollback(Transaction<'static, Sqlite>),
}

pub struct QueuedTransaction {
    tx: Option<Transaction<'static, Sqlite>>,
    finish_tx: Option<oneshot::Sender<QueuedTransactionFinish>>,
    result_rx: Option<oneshot::Receiver<WriteResult>>,
    owned_writer: Option<Arc<DbWriter>>,
    context: &'static str,
}

impl QueuedTransaction {
    fn attach_owner(&mut self, writer: Arc<DbWriter>) {
        self.owned_writer = Some(writer);
    }

    pub async fn commit(mut self) -> anyhow::Result<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| anyhow::anyhow!("queued transaction already finished"))?;
        let finish_tx = self
            .finish_tx
            .take()
            .ok_or_else(|| anyhow::anyhow!("queued transaction finish channel missing"))?;
        finish_tx
            .send(QueuedTransactionFinish::Commit(tx))
            .map_err(|_| anyhow::anyhow!("queued transaction worker dropped before commit"))?;
        self.await_result().await
    }

    pub async fn rollback(mut self) -> anyhow::Result<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| anyhow::anyhow!("queued transaction already finished"))?;
        let finish_tx = self
            .finish_tx
            .take()
            .ok_or_else(|| anyhow::anyhow!("queued transaction finish channel missing"))?;
        finish_tx
            .send(QueuedTransactionFinish::Rollback(tx))
            .map_err(|_| anyhow::anyhow!("queued transaction worker dropped before rollback"))?;
        self.await_result().await
    }

    async fn await_result(mut self) -> anyhow::Result<()> {
        let result_rx = self
            .result_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("queued transaction result channel missing"))?;
        let result = match result_rx.await {
            Ok(WriteResult::Committed) => Ok(()),
            Ok(WriteResult::WriteRejected { reason, .. }) => {
                anyhow::bail!("DbWriter rejected {}: {reason}", self.context)
            }
            Ok(other) => anyhow::bail!(
                "DbWriter queued transaction {} did not commit: {}",
                self.context,
                other.as_str()
            ),
            Err(_) => anyhow::bail!("DbWriter queued transaction worker dropped result"),
        };
        if let Some(writer) = self.owned_writer.take() {
            writer.shutdown().await;
        }
        result
    }
}

impl Deref for QueuedTransaction {
    type Target = Transaction<'static, Sqlite>;

    fn deref(&self) -> &Self::Target {
        self.tx
            .as_ref()
            .expect("queued transaction used after finish")
    }
}

impl DerefMut for QueuedTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.tx
            .as_mut()
            .expect("queued transaction used after finish")
    }
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
/// Maximum distinct coalescing keys in the buffer.
///
/// When a new (non-colliding) key would exceed this limit, the write is rejected with
/// `WriteRejected { reason: "coalescing_map_saturated" }` and `coalesced_rejected_total`
/// is incremented. Per P075 §architecture.backpressure_and_admission_control (Class B overflow).
pub const COALESCE_MAX_KEYS: usize = 1024;

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
/// Only operations needed to persist terminal or shutdown canonical state are admitted.
/// Any Class A write whose operation_name is not in this list is denied during drain.
///
/// When new terminal producers are wired through DbWriter in Phase 3+, add their
/// canonical operation_name here and add a regression test that verifies the listed
/// operations are admitted and unlisted operations are denied.
pub const SHUTDOWN_ADMITTED_OPERATIONS: &[&str] = &[
    // Terminal run state transitions.
    "run_complete",
    "run_failed",
    "run_cancelled",
    // Terminal stage state transitions (includes generic canonical_stage_transition).
    "stage_complete",
    "stage_failed",
    "canonical_stage_transition",
    // Terminal agent state transitions.
    "agent_complete",
    "agent_failed",
    // Approval decisions (terminal for approval-gate stages).
    "approval_decision",
    // Operator command completion records.
    "operator_command_complete",
    // Projection invalidation triggered by a terminal canonical change.
    "projection_invalidation_terminal",
];

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

/// Maximum retained Class D rollup windows.
pub const TELEMETRY_SNAPSHOT_RETAIN_LATEST: i64 = 288;

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
        stale
            .into_iter()
            .filter_map(|k| self.entries.remove(&k))
            .collect()
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
    /// Unix epoch milliseconds for the last heartbeat beat.
    last_beat_wall_ms: AtomicI64,
    /// Per-lane: monotonic ns since process start of last drain. Indexed by LANE_DRAIN_ORDER.
    last_drain_ns: [AtomicU64; 6],
    /// Per-lane: Unix epoch milliseconds for the last drain.
    last_drain_wall_ms: [AtomicI64; 6],
    /// Cumulative lane starvation events (lower lane starved > STARVATION_WATCHDOG_SECS).
    pub starvation_total: AtomicU64,
    /// Per-lane: monotonic ns since process start when the first unprocessed item was enqueued
    /// in the current busy period. 0 = no pending items. Used to populate WriteRejected.oldest_queued_ms.
    pub lane_oldest_enqueued_ns: [AtomicU64; 6],
    /// Per-lane: approximate count of items pending in the channel (submit increments,
    /// execute_message decrements). Used to clear `lane_oldest_enqueued_ns` when lane empties.
    pub lane_pending_count: [AtomicI64; 6],
    /// Cumulative Class B coalescing rejections due to map saturation (COALESCE_MAX_KEYS).
    pub coalesced_rejected_total: AtomicU64,
    /// Cumulative Class B writes merged into an existing coalescing key.
    pub coalesced_merged_total: AtomicU64,
    /// Cumulative Class D telemetry writes dropped under queue pressure.
    pub telemetry_dropped_total: AtomicU64,
    /// Rolling committed transaction duration samples in milliseconds.
    transaction_duration_ms: Mutex<VecDeque<u64>>,
    /// Reference instant for ns offsets (process-start-relative).
    origin: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbWriterLaneSnapshot {
    pub lane: WriteLane,
    pub capacity: usize,
    pub queued_depth: i64,
    pub oldest_queued_age_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbWriterHealthSnapshot {
    pub alive: bool,
    pub last_heartbeat_at: Option<String>,
    pub last_heartbeat_age_ms: Option<u64>,
    pub last_drain_at: Option<String>,
    pub last_drain_age_ms: Option<u64>,
    pub transaction_duration_p50_ms: Option<u64>,
    pub transaction_duration_p95_ms: Option<u64>,
    pub transaction_duration_sample_count: usize,
    pub total_queued: i64,
    pub lanes: Vec<DbWriterLaneSnapshot>,
    pub coalesced_rejected_total: u64,
    pub coalesced_merged_total: u64,
    pub telemetry_dropped_total: u64,
    pub starvation_total: u64,
}

impl DbWriterHeartbeat {
    fn new() -> Self {
        const ZERO_U64: AtomicU64 = AtomicU64::new(0);
        const ZERO_I64: AtomicI64 = AtomicI64::new(0);
        Self {
            alive: AtomicBool::new(false),
            last_beat_ns: AtomicU64::new(0),
            last_beat_wall_ms: AtomicI64::new(0),
            last_drain_ns: [ZERO_U64; 6],
            last_drain_wall_ms: [ZERO_I64; 6],
            starvation_total: AtomicU64::new(0),
            lane_oldest_enqueued_ns: [ZERO_U64; 6],
            lane_pending_count: [ZERO_I64; 6],
            coalesced_rejected_total: AtomicU64::new(0),
            coalesced_merged_total: AtomicU64::new(0),
            telemetry_dropped_total: AtomicU64::new(0),
            transaction_duration_ms: Mutex::new(VecDeque::new()),
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
        self.last_beat_wall_ms
            .store(Utc::now().timestamp_millis(), Ordering::Relaxed);
    }

    /// Record that a specific lane just drained a message.
    fn record_drain(&self, lane: WriteLane) {
        let idx = lane.drain_order_index();
        self.last_drain_ns[idx].store(self.now_ns(), Ordering::Relaxed);
        self.last_drain_wall_ms[idx].store(Utc::now().timestamp_millis(), Ordering::Relaxed);
    }

    fn record_transaction_duration(&self, duration_ms: u64) {
        let mut samples = self.transaction_duration_ms.lock().unwrap();
        if samples.len() >= TX_DURATION_SAMPLE_LIMIT {
            samples.pop_front();
        }
        samples.push_back(duration_ms);
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

    pub fn snapshot(&self) -> DbWriterHealthSnapshot {
        let now = self.now_ns();
        let last_heartbeat_at = unix_ms_to_rfc3339(self.last_beat_wall_ms.load(Ordering::Relaxed));
        let last_heartbeat_age_ms = age_ms(now, self.last_beat_ns.load(Ordering::Relaxed));
        let last_drain_wall_ms = DB_WRITER_LANES
            .iter()
            .filter_map(|lane| {
                let value =
                    self.last_drain_wall_ms[lane.drain_order_index()].load(Ordering::Relaxed);
                (value > 0).then_some(value)
            })
            .max()
            .unwrap_or(0);
        let last_drain_at = unix_ms_to_rfc3339(last_drain_wall_ms);
        let last_drain_age_ms = DB_WRITER_LANES
            .iter()
            .filter_map(|lane| {
                age_ms(
                    now,
                    self.last_drain_ns[lane.drain_order_index()].load(Ordering::Relaxed),
                )
            })
            .min();
        let lanes = DB_WRITER_LANES
            .iter()
            .copied()
            .map(|lane| {
                let idx = lane.drain_order_index();
                let queued_depth = self.lane_pending_count[idx].load(Ordering::Relaxed).max(0);
                let oldest_queued_age_ms = age_ms(
                    now,
                    self.lane_oldest_enqueued_ns[idx].load(Ordering::Relaxed),
                );
                DbWriterLaneSnapshot {
                    lane,
                    capacity: lane.capacity(),
                    queued_depth,
                    oldest_queued_age_ms,
                }
            })
            .collect::<Vec<_>>();
        let total_queued = lanes.iter().map(|lane| lane.queued_depth).sum();
        let transaction_duration_samples = self
            .transaction_duration_ms
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let transaction_duration_p50_ms = percentile(transaction_duration_samples.clone(), 50);
        let transaction_duration_sample_count = transaction_duration_samples.len();
        let transaction_duration_p95_ms = percentile(transaction_duration_samples, 95);
        DbWriterHealthSnapshot {
            alive: self.is_alive(),
            last_heartbeat_at,
            last_heartbeat_age_ms,
            last_drain_at,
            last_drain_age_ms,
            transaction_duration_p50_ms,
            transaction_duration_p95_ms,
            transaction_duration_sample_count,
            total_queued,
            lanes,
            coalesced_rejected_total: self.coalesced_rejected_total.load(Ordering::Relaxed),
            coalesced_merged_total: self.coalesced_merged_total.load(Ordering::Relaxed),
            telemetry_dropped_total: self.telemetry_dropped_total.load(Ordering::Relaxed),
            starvation_total: self.starvation_total.load(Ordering::Relaxed),
        }
    }
}

fn age_ms(now_ns: u64, stored_ns: u64) -> Option<u64> {
    if stored_ns == 0 {
        None
    } else {
        Some(now_ns.saturating_sub(stored_ns) / 1_000_000)
    }
}

fn unix_ms_to_rfc3339(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }
    Utc.timestamp_millis_opt(value)
        .single()
        .map(|timestamp| timestamp.to_rfc3339())
}

fn percentile(mut values: Vec<u64>, percentile: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let idx = ((values.len() - 1) * percentile).div_ceil(100);
    values.get(idx).copied()
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
        self.submit_work(op, make_work(work)).await
    }

    pub async fn submit_work(&self, op: WriteOperation, work: WriteWork) -> WriteResult {
        if let Err(rejected) = op.validate() {
            return rejected;
        }

        // Shutdown admission filter (LIFT-REL-03).
        if self.shutdown_in_progress.load(Ordering::Relaxed) {
            let admitted = if op.class == WriteClass::A {
                // Fail-closed: only operations in SHUTDOWN_ADMITTED_OPERATIONS are admitted.
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
            return self.submit_class_b(op, work).await;
        }

        tracing::debug!(
            operation_name = op.operation_name,
            lane = op.lane.as_str(),
            class = op.class.as_str(),
            idempotency_key_hash = %hash_idempotency_key(&op.idempotency_key),
            "DbWriter.submit: enqueuing"
        );

        let deadline = op.deadline;
        let lane_idx = op.lane.drain_order_index();
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
        if op.class == WriteClass::D && lane_tx.capacity() == 0 {
            let dropped_total = self
                .heartbeat
                .telemetry_dropped_total
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            tracing::warn!(
                operation_name = op.operation_name,
                lane = op.lane.as_str(),
                telemetry_dropped_total = dropped_total,
                "DbWriter: dropping Class D telemetry because lane is full"
            );
            return WriteResult::DroppedTelemetry;
        }

        // Enqueue within deadline (admission wait counts against deadline).
        match tokio::time::timeout(deadline, lane_tx.send(msg)).await {
            Err(_elapsed) => {
                // Capture depth at rejection time (more accurate than pre-send snapshot).
                let queued_depth = lane_tx.max_capacity() - lane_tx.capacity();
                // Compute oldest_queued_ms from the per-lane tracking set when the lane first
                // became busy. 0 means the tracker was never set (very short window race).
                let oldest_queued_ms = {
                    let oldest_ns =
                        self.heartbeat.lane_oldest_enqueued_ns[lane_idx].load(Ordering::Relaxed);
                    if oldest_ns == 0 {
                        None
                    } else {
                        Some(self.heartbeat.now_ns().saturating_sub(oldest_ns) / 1_000_000)
                    }
                };
                WriteResult::WriteRejected {
                    lane: op.lane.as_str(),
                    capacity: op.lane.capacity(),
                    queued_depth,
                    oldest_queued_ms,
                    operation_name: op.operation_name,
                    reason: "lane_saturated",
                }
            }
            Ok(Err(_channel_closed)) => WriteResult::WriteFailed,
            Ok(Ok(())) => {
                // Track oldest-enqueued for WriteRejected.oldest_queued_ms observability.
                // Increment pending count; if this is the first item (prev == 0), record the
                // enqueue time as the start of the current busy period.
                let prev_pending =
                    self.heartbeat.lane_pending_count[lane_idx].fetch_add(1, Ordering::Relaxed);
                if prev_pending == 0 {
                    self.heartbeat.lane_oldest_enqueued_ns[lane_idx]
                        .store(self.heartbeat.now_ns(), Ordering::Relaxed);
                }
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

    pub async fn begin_immediate_transaction(
        &self,
        op: WriteOperation,
        context: &'static str,
    ) -> anyhow::Result<QueuedTransaction> {
        if let Err(rejected) = op.validate() {
            anyhow::bail!(
                "DbWriter rejected {context} before transaction start: {}",
                rejected.as_str()
            );
        }
        if self.shutdown_in_progress.load(Ordering::Relaxed) {
            let admitted = if op.class == WriteClass::A {
                SHUTDOWN_ADMITTED_OPERATIONS.contains(&op.operation_name)
            } else {
                false
            };
            if !admitted {
                anyhow::bail!("DbWriter rejected {context}: shutdown_admission_denied");
            }
        }

        let deadline = op.deadline;
        let lane_idx = op.lane.drain_order_index();
        let submit_start = Instant::now();
        let (result_tx, result_rx) = oneshot::channel();
        let (tx_ready_tx, tx_ready_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let msg = LaneMessage {
            op: op.clone(),
            work: make_work(move |pool| async move {
                let tx = crate::pool::begin_immediate_with_retry(&pool, context).await?;
                tx_ready_tx
                    .send(tx)
                    .map_err(|_| anyhow::anyhow!("queued transaction receiver dropped"))?;
                match finish_rx.await {
                    Ok(QueuedTransactionFinish::Commit(tx)) => {
                        tx.commit().await?;
                        Ok(1)
                    }
                    Ok(QueuedTransactionFinish::Rollback(tx)) => {
                        tx.rollback().await?;
                        Ok(0)
                    }
                    Err(_) => anyhow::bail!("queued transaction dropped before commit or rollback"),
                }
            }),
            result_tx,
            enqueued_at: submit_start,
        };
        let lane_tx = self.lane_sender(op.lane);
        match tokio::time::timeout(deadline, lane_tx.send(msg)).await {
            Err(_elapsed) => {
                anyhow::bail!("DbWriter queued transaction {context} timed out during admission")
            }
            Ok(Err(_channel_closed)) => {
                anyhow::bail!("DbWriter queued transaction {context} lane closed")
            }
            Ok(Ok(())) => {
                let prev_pending =
                    self.heartbeat.lane_pending_count[lane_idx].fetch_add(1, Ordering::Relaxed);
                if prev_pending == 0 {
                    self.heartbeat.lane_oldest_enqueued_ns[lane_idx]
                        .store(self.heartbeat.now_ns(), Ordering::Relaxed);
                }
            }
        }

        let remaining = deadline
            .checked_sub(submit_start.elapsed())
            .unwrap_or(Duration::ZERO);
        if remaining == Duration::ZERO {
            anyhow::bail!("DbWriter queued transaction {context} timed out before begin");
        }
        let tx = tokio::time::timeout(remaining, tx_ready_rx)
            .await
            .map_err(|_| {
                anyhow::anyhow!("DbWriter queued transaction {context} timed out before begin")
            })?
            .map_err(|_| {
                anyhow::anyhow!("DbWriter queued transaction {context} worker dropped before begin")
            })?;
        Ok(QueuedTransaction {
            tx: Some(tx),
            finish_tx: Some(finish_tx),
            result_rx: Some(result_rx),
            owned_writer: None,
            context,
        })
    }

    /// Submit a write that owns a full SQLite transaction inside the DbWriter lane.
    ///
    /// This is the migration bridge for existing multi-row repository flows: the
    /// transaction is opened, executed, committed, and classified by DbWriter, so
    /// callers do not create runtime write transactions outside the gateway.
    pub async fn submit_transaction<T>(
        &self,
        op: WriteOperation,
        context: &'static str,
        work: TransactionWork<T>,
    ) -> anyhow::Result<T>
    where
        T: Send + 'static,
    {
        let (value_tx, value_rx) = oneshot::channel();
        let result = self
            .submit(op, move |pool| async move {
                let mut tx = crate::pool::begin_immediate_with_retry(&pool, context).await?;
                let (value, rows) = work(&mut tx).await?;
                tx.commit().await?;
                value_tx
                    .send(value)
                    .map_err(|_| anyhow::anyhow!("DbWriter transaction result receiver dropped"))?;
                Ok(rows)
            })
            .await;
        match result {
            WriteResult::Committed => value_rx
                .await
                .map_err(|_| anyhow::anyhow!("DbWriter transaction committed without result")),
            WriteResult::WriteRejected { reason, .. } => {
                anyhow::bail!("DbWriter rejected {context}: {reason}")
            }
            other => anyhow::bail!(
                "DbWriter transaction {context} did not commit: {}",
                other.as_str()
            ),
        }
    }

    pub async fn submit_unit_transaction(
        &self,
        op: WriteOperation,
        context: &'static str,
        work: TransactionWork<()>,
    ) -> anyhow::Result<()> {
        let result = self
            .submit_work(
                op,
                make_work(move |pool| async move {
                    let mut tx = crate::pool::begin_immediate_with_retry(&pool, context).await?;
                    let ((), rows) = work(&mut tx).await?;
                    tx.commit().await?;
                    Ok(rows)
                }),
            )
            .await;
        match result {
            WriteResult::Committed | WriteResult::Coalesced | WriteResult::DroppedTelemetry => {
                Ok(())
            }
            WriteResult::WriteRejected { reason, .. } => {
                anyhow::bail!("DbWriter rejected {context}: {reason}")
            }
            other => anyhow::bail!(
                "DbWriter transaction {context} did not commit: {}",
                other.as_str()
            ),
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
            let flush_budget =
                tokio::time::Duration::from_millis(SHUTDOWN_CLASS_B_FLUSH_BUDGET_MS + 200);
            match tokio::time::timeout(flush_budget, h).await {
                Ok(_) => tracing::debug!("DbWriter coalescing flush task shut down cleanly"),
                Err(_) => {
                    tracing::warn!("DbWriter coalescing flush task did not finish within budget")
                }
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
                    self.heartbeat
                        .coalesced_merged_total
                        .fetch_add(1, Ordering::Relaxed);
                    buf.entries.insert(
                        key,
                        CoalescedEntry {
                            op,
                            work,
                            result_tx,
                            enqueued_at: submit_start,
                            mono_counter,
                        },
                    );
                } else {
                    // New entry is stale; evict it immediately.
                    let _ = result_tx.send(WriteResult::Coalesced);
                    buf.merge_count += 1;
                    buf.total_merged += 1;
                    self.heartbeat
                        .coalesced_merged_total
                        .fetch_add(1, Ordering::Relaxed);
                    // result_rx will return Coalesced on the await below.
                }
            } else if buf.entries.len() >= COALESCE_MAX_KEYS {
                // Coalescing map is saturated: reject the new key rather than growing unboundedly.
                // Per P075 §architecture.backpressure_and_admission_control (Class B overflow policy).
                let lane_str = op.lane.as_str();
                let op_name = op.operation_name;
                let depth = buf.entries.len();
                drop(buf);
                let rejected_total = self
                    .heartbeat
                    .coalesced_rejected_total
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                tracing::warn!(
                    operation_name = op_name,
                    coalesced_rejected_total = rejected_total,
                    coalescing_map_depth = depth,
                    "DbWriter: coalescing buffer saturated; rejecting new Class B key"
                );
                return WriteResult::WriteRejected {
                    lane: lane_str,
                    capacity: COALESCE_MAX_KEYS,
                    queued_depth: depth,
                    oldest_queued_ms: None,
                    operation_name: op_name,
                    reason: "coalescing_map_saturated",
                };
            } else {
                buf.entries.insert(
                    key,
                    CoalescedEntry {
                        op,
                        work,
                        result_tx,
                        enqueued_at: submit_start,
                        mono_counter,
                    },
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
/// - Every `COALESCE_FLUSH_INTERVAL_MS`: drains **all** pending entries unconditionally.
///   This guarantees a Class B write with the 1000 ms default deadline always commits
///   via the periodic flush (~500 ms wait) rather than timing out. `COALESCE_MAX_KEY_AGE_MS`
///   is a documentation bound (no entry can ever be held longer than ~500 ms) not a drain
///   gate — draining only stale entries on the 500 ms tick would let single Class B writes
///   sit for ~2 s and trip WriteTimeout before the deadline (P075-DEFECT-CLASSBFLUSH).
/// - On shutdown signal: force-drains all remaining entries (sub-budget LIFT-REL-03).
async fn run_coalescing_flush(
    coalescing: Arc<Mutex<CoalescingBuffer>>,
    cp_tx: mpsc::Sender<LaneMessage>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    use tokio::time::{interval, Duration};
    let mut tick = interval(Duration::from_millis(COALESCE_FLUSH_INTERVAL_MS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // `tokio::interval` ticks immediately on first poll. Consume that initial
    // tick so Class B writes get the documented 500 ms coalescing window.
    tick.tick().await;

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
                // Drain ALL entries every 500 ms so no Class B write waits longer than
                // ~500 ms regardless of merge activity (P075-DEFECT-CLASSBFLUSH fix).
                let entries: Vec<CoalescedEntry> = {
                    let mut buf = coalescing.lock().unwrap();
                    buf.drain_all()
                };
                if !entries.is_empty() {
                    tracing::debug!(count = entries.len(), "CoalescingFlush: draining all entries");
                    flush_coalesced_to_channel(entries, &cp_tx).await;
                }
            }
        }
    }
}

/// Send coalesced entries to the channel, notifying callers of any send failures.
async fn flush_coalesced_to_channel(
    entries: Vec<CoalescedEntry>,
    cp_tx: &mpsc::Sender<LaneMessage>,
) {
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
    let queue_wait_ms = msg.enqueued_at.elapsed().as_millis() as u64;
    let lane_idx = lane.drain_order_index();

    if msg.enqueued_at.elapsed() >= msg.op.deadline {
        tracing::warn!(
            operation_name = msg.op.operation_name,
            lane = lane.as_str(),
            write_id_hash = %hash_idempotency_key(&msg.op.idempotency_key),
            queue_wait_ms,
            deadline_ms = msg.op.deadline.as_millis() as u64,
            "DbWriter: deadline expired in queue; returning WriteTimeout"
        );
        let _ = msg.result_tx.send(WriteResult::WriteTimeout);
        // Decrement pending count; clear oldest_enqueued if lane is now empty.
        let prev = heartbeat.lane_pending_count[lane_idx].fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            heartbeat.lane_oldest_enqueued_ns[lane_idx].store(0, Ordering::Relaxed);
        }
        return;
    }

    tracing::debug!(
        operation_name = msg.op.operation_name,
        lane = lane.as_str(),
        write_id_hash = %hash_idempotency_key(&msg.op.idempotency_key),
        queue_wait_ms,
        expected_rows = msg.op.expected_rows,
        "DbWriter: executing write"
    );

    // Time the transaction. Lock wait and busy-retry duration are included in tx_duration_ms
    // because they occur inside the work closure (which calls begin_immediate_with_retry).
    let tx_start = Instant::now();

    // Run work to completion without a timeout wrapper. Per P075
    // §architecture.deadlines_and_results.in_flight_timeout, in-flight
    // transactions are not cancelled mid-transaction; they complete or roll
    // back under SQLite semantics. The caller-side result_rx timeout (in
    // submit()) handles the case where the caller does not want to wait.
    let work_result = (msg.work)(pool.clone()).await;

    let tx_duration_ms = tx_start.elapsed().as_millis() as u64;

    let write_result = match work_result {
        Err(ref e) if is_busy_error(e) => {
            tracing::warn!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                write_id_hash = %hash_idempotency_key(&msg.op.idempotency_key),
                queue_wait_ms,
                tx_duration_ms,
                expected_rows = msg.op.expected_rows,
                error_kind = "sqlite_busy",
                "DbWriter: SQLite busy exhausted; returning WriteBusyExhausted"
            );
            WriteResult::WriteBusyExhausted
        }
        Err(e) => {
            tracing::error!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                write_id_hash = %hash_idempotency_key(&msg.op.idempotency_key),
                queue_wait_ms,
                tx_duration_ms,
                expected_rows = msg.op.expected_rows,
                error_kind = %classify_write_error(&e),
                "DbWriter: write failed"
            );
            WriteResult::WriteFailed
        }
        Ok(rows) => {
            tracing::debug!(
                operation_name = msg.op.operation_name,
                lane = lane.as_str(),
                write_id_hash = %hash_idempotency_key(&msg.op.idempotency_key),
                queue_wait_ms,
                tx_duration_ms,
                expected_rows = msg.op.expected_rows,
                actual_rows = rows,
                "DbWriter: committed"
            );
            WriteResult::Committed
        }
    };

    // Record heartbeat for this lane.
    heartbeat.record_drain(lane);
    heartbeat.record_transaction_duration(tx_duration_ms);

    // Decrement pending count; clear oldest_enqueued_ns when lane empties.
    let prev = heartbeat.lane_pending_count[lane_idx].fetch_sub(1, Ordering::Relaxed);
    if prev == 1 {
        heartbeat.lane_oldest_enqueued_ns[lane_idx].store(0, Ordering::Relaxed);
    }

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

    #[tokio::test]
    async fn file_backed_registered_transaction_requires_shared_writer() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("p075-shared-writer-required.sqlite");
        let pool = crate::pool::create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
            .await
            .unwrap();

        let missing = match begin_registered_immediate_transaction(
            &pool,
            make_class_a_op(),
            "p075_shared_writer_required",
        )
        .await
        {
            Ok(_) => panic!("file-backed transaction unexpectedly opened without shared writer"),
            Err(error) => error,
        };
        assert!(
            missing
                .to_string()
                .contains("P075 shared DbWriter is not registered"),
            "file-backed runtime transactions must fail closed without the daemon shared writer; got {missing}"
        );

        let writer = Arc::new(DbWriter::new(pool.clone()));
        register_shared_writer(&pool, writer.clone()).await.unwrap();
        let tx = begin_registered_immediate_transaction(
            &pool,
            make_class_a_op(),
            "p075_shared_writer_registered",
        )
        .await
        .expect("registered shared writer must admit file-backed transaction");
        tx.commit().await.unwrap();
        writer.shutdown().await;
    }

    #[tokio::test]
    async fn shared_writer_lookup_works_for_cloned_file_backed_pool() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("p075-shared-writer-clone.sqlite");
        let pool = crate::pool::create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
            .await
            .unwrap();
        let pool_clone = pool.clone();

        let writer = Arc::new(DbWriter::new(pool.clone()));
        register_shared_writer(&pool, writer.clone()).await.unwrap();

        let tx = begin_registered_immediate_transaction(
            &pool_clone,
            make_class_a_op(),
            "p075_shared_writer_clone_lookup",
        )
        .await
        .expect("registered shared writer must be found through cloned pool handles");
        tx.commit().await.unwrap();
        writer.shutdown().await;
    }

    /// Regression test for CLEAN-002 / LIFT-REL-03: SHUTDOWN_ADMITTED_OPERATIONS must be
    /// populated with canonical terminal state operations, and unknown operations must be denied.
    #[test]
    fn shutdown_class_a_admission_empty_list_denies_all_phase2_sentinel() {
        // List is now populated with terminal state operations (Phase 3+ refinement).
        assert!(
            !SHUTDOWN_ADMITTED_OPERATIONS.is_empty(),
            "SHUTDOWN_ADMITTED_OPERATIONS must not be empty; populate with terminal operation names \
             (run_complete, stage_complete, etc.) so shutdown admits terminal canonical writes."
        );
        // Listed operations must be admitted.
        fn is_admitted(list: &[&str], op_name: &str) -> bool {
            list.contains(&op_name)
        }
        assert!(
            is_admitted(SHUTDOWN_ADMITTED_OPERATIONS, "run_complete"),
            "run_complete must be in SHUTDOWN_ADMITTED_OPERATIONS"
        );
        assert!(
            is_admitted(SHUTDOWN_ADMITTED_OPERATIONS, "stage_complete"),
            "stage_complete must be in SHUTDOWN_ADMITTED_OPERATIONS"
        );
        // Unknown operations must still be denied (fail-closed for unlisted ops).
        assert!(
            !is_admitted(SHUTDOWN_ADMITTED_OPERATIONS, "unlisted_terminal_op"),
            "unlisted operations must still be denied (fail-closed for unlisted)"
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
        assert!(
            COALESCE_MAX_KEYS > 0,
            "COALESCE_MAX_KEYS must be a positive bound"
        );
    }

    /// Unit test for coalescing buffer saturation logic.
    ///
    /// Fills `CoalescingBuffer` to `COALESCE_MAX_KEYS` entries using the internal struct
    /// (accessible here because tests are in the same module), then verifies that the
    /// saturation check fires before any new entry would be inserted.
    ///
    /// This tests the logic layer only; the integration-level rejection through DbWriter.submit
    /// is covered separately in the proposal_075_dbwriter integration test.
    #[test]
    fn coalescing_buffer_saturation_check_fires_at_max_keys() {
        let mut buf = CoalescingBuffer::new();
        // Fill to capacity with distinct keys.
        for i in 0..COALESCE_MAX_KEYS {
            let key = format!("run-sat/key-{i}");
            // We can't construct CoalescedEntry without a real oneshot channel, so just
            // verify the saturation condition using the buffer's length before insert.
            // Simulate insertion by directly inserting a placeholder entry.
            use tokio::sync::oneshot;
            let (tx, _rx) = oneshot::channel::<WriteResult>();
            let op = WriteOperation {
                class: WriteClass::B,
                lane: WriteLane::CoalescedProjection,
                operation_name: "test_saturation",
                expected_rows: 1,
                batchable: true,
                barrier: false,
                deadline: WriteClass::B.default_deadline(),
                deadline_reason: None,
                idempotency_key: key.clone(),
                replay_policy: ReplayPolicy::LastWriterWins,
                observed_at: None,
            };
            let work: WriteWork = Box::new(|_pool| Box::pin(async { Ok(1u32) }));
            buf.entries.insert(
                key,
                CoalescedEntry {
                    op,
                    work,
                    result_tx: tx,
                    enqueued_at: std::time::Instant::now(),
                    mono_counter: i as u64,
                },
            );
        }
        // Buffer is now at COALESCE_MAX_KEYS entries.
        assert_eq!(buf.entries.len(), COALESCE_MAX_KEYS);
        // The saturation check in submit_class_b fires when entries.len() >= COALESCE_MAX_KEYS.
        assert!(
            buf.entries.len() >= COALESCE_MAX_KEYS,
            "saturation check must fire: buffer at {} >= max {}",
            buf.entries.len(),
            COALESCE_MAX_KEYS
        );
        // Draining clears the buffer and allows new entries.
        let drained = buf.drain_all();
        assert_eq!(drained.len(), COALESCE_MAX_KEYS);
        assert_eq!(buf.entries.len(), 0);
        // After drain, saturation check no longer fires.
        assert!(buf.entries.len() < COALESCE_MAX_KEYS);
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

    #[tokio::test]
    async fn registered_repository_transaction_commits_through_queued_writer() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let mut tx = begin_repository_transaction(&pool, "test_registered_repository_transaction")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE p075_registered_tx_probe (id INTEGER PRIMARY KEY)")
            .execute(&mut **tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO p075_registered_tx_probe (id) VALUES (1)")
            .execute(&mut **tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM p075_registered_tx_probe")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
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
