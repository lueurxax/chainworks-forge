# Proposal 075: Local Persistence Write Budget, Evidence Spooling, and SQLite Pressure Control

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Approved proposal artifact from P075 run; implementation closeout is tracked in reference docs and audit evidence |
| Revision | `p075-refined-2026-05-01-r2-9f1a2b7c` |
| Source idea | Implement Proposal 075: Local Persistence Write Budget and Evidence Spooling |
| Source artifact | `.chainworks/runs/4aeb45a9-b5e1-4891-b08a-60d994204083/proposals/approved/proposal.md` |
| Primary area | Rust control plane local persistence |
| Depends on | Rust control plane, SQLite persistence in WAL mode with bounded BEGIN IMMEDIATE retry, Artifact store, P038 run compaction, P073 stabilization freeze |
| Related | `P061`: Foundation for command write serialization, BEGIN IMMEDIATE retry, and existing scheduler/db contention readbacks. P075 reuses begin_immediate_with_retry rather than adding a second retry primitive.<br>`P066`: Related input only. P075 does not expand provider toolchain cache mapping.<br>`P078`: Downstream consumer. P075 reserves barrier and evidence discipline but does not implement side-effect ledger, settlement, release reconciliation, effect retries, or effect readbacks. |

---

## Summary

- **Decision:** Keep SQLite as canonical local state, but stop using it as a high-volume runtime event stream.
- **Design:** Route runtime writes through a bounded, priority-laned DbWriter; classify writes into four classes (A barrier, B coalesced, C evidence-spool metadata, D telemetry rollup); spool high-volume evidence to files with compact metadata; coalesce non-critical state with mandatory periodic flushes; roll up telemetry; expose typed storageHealth via GraphQL and parity MCP diagnostics; gate direct-write bypasses with a checked-in allowlist that requires retirement criteria.
- **Review Focus:** This revision resolves the first review pass blockers by specifying queue bounds and overflow policy, per-class deadlines and typed WriteResult variants, idempotency and replay contracts, graceful shutdown drain order and SIGKILL recovery, in-scope startup orphan recovery, mandatory Class B periodic flush, exact GraphQL/MCP schemas with units and freshness, migration constraints and indexes, forward-only/downgrade behavior, initial numeric warn/critical thresholds, and rollout kill-switches surfaced in storageHealth.

## Problem

- **Failure Mode:** Important canonical transitions can wait behind low-value updates, SQLite can drift into a runtime log sink, and operators lose the ability to distinguish normal load from write-lock incidents.

### Context

- The daemon is local-first: one Rust daemon, one SQLite database, local artifact storage, ACP runtimes, GraphQL projections, and MCP control.
- SQLite remains a good compact canonical state database for the operator app, but it has one practical writer even in WAL mode.
- P061 already reduces contention for important command transitions. P075 generalizes write discipline across runtime evidence, projection invalidation, telemetry, and future durable layers.

### Write Pressure Sources

- ACP stream chunks
- model text deltas
- transcript fragments
- tool stdout and stderr
- raw tool traces
- repeated runtime events
- session health pings
- projection invalidations
- subscription freshness markers
- telemetry counters
- future side-effect settlement evidence owned by P078

## Goals

- Route all non-test runtime write transactions through DbWriter or a source-controlled temporary bypass allowlist with owner and retirement criteria.
- Require every WriteOperation to declare class, lane, operation name, expected row count, batchability, barrier semantics, deadline, idempotency key, and replay policy.
- Keep high-volume raw evidence on local files and persist only compact SQLite metadata pointers.
- Coalesce repeated non-critical projection and status updates with mandatory bounded flushes.
- Make write pressure observable through typed GraphQL storageHealth, structured logs, optional MCP diagnostics, and the proposal-075 gate.
- Preserve SQLite as canonical local state while keeping write transactions short, bounded, and free of filesystem, provider, network, and ACP waits.
- Give P078 a safe barrier/evidence foundation without implementing P078 semantics.
- Provide rollout kill-switches for half-migrated producer paths.

## Non-Goals

- No durable side-effect ledger, release settlement, reconciliation loop, effect retry semantics, effect ids, or effect GraphQL/MCP readbacks. Those belong to P078.
- No expansion of P066 provider toolchain cache mapping.
- No replacement of SQLite with another database or event store.
- No SwiftUI or AppKit mutation/control changes.
- No remote service, sidecar, distributed queue, or external observability dependency.
- No workflow YAML, agent catalog, protobuf, or OpenAPI contract changes.
- No long-running filesystem, ACP, provider, network, artifact discovery, or checkpoint wait inside SQLite write transactions.

## Current System Context

- control-plane/crates/db/src/pool.rs already configures file-backed SQLite pools, WAL, busy timeout, and bounded BEGIN IMMEDIATE retry logging.
- docs/reference/rust-control-plane.md documents single-writer command mutation semantics, no I/O inside transactions, and p95 command latency goals.
- P061 surfaces scheduler health and DB contention readbacks through GraphQL and MCP.
- Artifact contents already live as filesystem objects while SQLite stores artifact metadata.
- Runtime facts and transcript attribution exist as compact state and references, but not as a complete high-volume evidence spool discipline.

## Architecture

### Write Classes

1. A
   - **Class:** A
   - **Name:** barrier
   - **Behavior:** Synchronous and durable before the system proceeds. Highest priority. Never intentionally dropped.
   - **SQLite Payload:** Compact canonical rows only.

   ##### Examples

   - run, stage, and agent canonical transitions
   - approval decisions
   - operator command completion
   - projection invalidation after canonical change
   - future P078 intent/executing/settled records
2. B
   - **Class:** B
   - **Name:** coalesced_state
   - **Behavior:** May be merged, replaced, delayed briefly, and flushed at boundaries or mandatory cadence.
   - **SQLite Payload:** Latest summary per coalescing key.

   ##### Examples

   - runtime status pings
   - stage progress text
   - session health summaries
   - projection freshness
   - bridge-pool heartbeat
3. C
   - **Class:** C
   - **Name:** evidence_spool_metadata
   - **Behavior:** Raw bytes are written, checksummed, and fsynced to files first. SQLite receives one compact metadata pointer.
   - **SQLite Payload:** EvidenceSpoolRef metadata only.

   ##### Examples

   - ACP chunks grouped into logical transcript objects
   - tool stdout and stderr files
   - raw tool traces
   - runtime event bundles
   - model deltas
   - large delivery readbacks
4. D
   - **Class:** D
   - **Name:** telemetry_rollup
   - **Behavior:** Aggregated in memory or local spool and periodically summarized. Lowest priority and droppable with counters.
   - **SQLite Payload:** Bounded rollup rows or health snapshot fields.

   ##### Examples

   - write-lock wait distribution
   - busy retry count
   - queue depth samples
   - subscription reconnects
   - tool-call counts
   - evidence bytes

### DbWriter

- **Location Decision:** Implement DbWriter in control-plane/crates/db/src/writer.rs. Engine-facing constructors receive DbWriter for writes and read pools/repositories for reads.
- **Fairness:** critical_barrier and operator_command are always polled before lower lanes. Lower lanes make progress by weighted draining when no higher lane is over deadline or above warning depth. A starvation watchdog logs and increments lane_starvation_total if a lower lane is unable to drain for 5 s while higher lanes are not saturated.
- **Heartbeat:** DbWriter emits a 1 Hz heartbeat with last_drain_at per lane, exposed in storageHealth.writer.lastHeartbeatAt and lastDrainAt; missed heartbeats surface as DbWriterHealth.alive=false.

#### Priority Lanes

1. Critical Barrier
   - **Lane:** critical_barrier
   - **Capacity:** `1024`

   ###### Classes

   - A
2. Operator Command
   - **Lane:** operator_command
   - **Capacity:** `512`

   ###### Classes

   - A
3. Projection Invalidation
   - **Lane:** projection_invalidation
   - **Capacity:** `2048`

   ###### Classes

   - A
   - B
4. Coalesced Projection
   - **Lane:** coalesced_projection
   - **Capacity:** `4096`

   ###### Classes

   - B
5. Evidence Metadata
   - **Lane:** evidence_metadata
   - **Capacity:** `2048`

   ###### Classes

   - C
6. Telemetry Rollup
   - **Lane:** telemetry_rollup
   - **Capacity:** `1024`

   ###### Classes

   - D

#### Transaction Rules

- Use the existing P061 bounded BEGIN IMMEDIATE retry path. Do not add an independent retry primitive.
- No provider calls, filesystem scans, network work, artifact discovery, checkpoint waits, or ACP runtime waits inside a transaction.
- Class A and B payloads must remain compact and may not contain raw evidence bytes.
- Class C file write, checksum, fsync(file), and fsync(parent directory on first creation) must complete before metadata enqueue.
- Record queue wait, lock wait, busy retries, transaction duration, expected rows, actual rows, class, lane, and operation name.

### Backpressure And Admission Control

- **Global Rule:** All lanes are bounded MPSC queues. Overflow is visible as typed WriteRejected or drop counters rather than unbounded memory growth.

#### Per Class Policy

1. A
   - **Class:** A
   - **Overflow:** Never drop. If the lane is saturated, wait until the write deadline; if not admitted, return WriteRejected with lane, capacity, queued_depth, oldest_queued_ms, and operation name.
2. B
   - **Class:** B
   - **Overflow:** Replace by coalescing key when possible and increment merged_count. If the coalescing map itself is saturated, reject the newest value with WriteRejected and increment coalesced_rejected_total.
3. C
   - **Class:** C
   - **Overflow:** Never drop metadata silently. If metadata queue admission misses its deadline, return WriteRejected; the already-fsynced file remains orphan-recoverable through the startup sweep.
4. D
   - **Class:** D
   - **Overflow:** Drop newest or oldest rollup sample according to metric type, increment dropped_total, and never block Class A or C.

#### Metrics

- queued_depth by lane
- oldest_queued_ms by lane
- rejected_total by class and lane
- dropped_total by class and lane
- coalesced_merged_total
- coalesced_rejected_total
- lane_starvation_total

### Deadlines And Results

- **Caller Override:** Callers may request a shorter or longer deadline. Class A deadlines above 5000 ms require an explicit operation reason recorded in logs and metrics.
- **Deadline Scope:** Deadline is enqueue to commit and includes admission wait, queue wait, lock wait, busy retry, SQL execution, and commit.
- **Queued Timeout:** Queued items are removed and callers receive WriteTimeout.
- **In Flight Timeout:** In-flight transactions are not cancelled mid-transaction. They complete or roll back under SQLite semantics; the caller receives the terminal WriteResult when possible.
- **Busy Retry Exhaustion:** BEGIN IMMEDIATE retry exhaustion returns WriteBusyExhausted, increments busy_retry_exhausted_total, and is not collapsed into a generic DB error.
- **Caller Cancellation:** Dropping a DbWriter request before admission removes it from the queue. Dropping after transaction start does not cancel the transaction; the writer records the result by write_id for post-cancel observability.

#### Default Deadlines Ms

- **Class A Barrier:** `2000`
- **Class B Coalesced:** `1000`
- **Class C Metadata After File Fsync:** `5000`
- **Class D Telemetry:** `1000`

#### Write Result Variants

- Committed
- Coalesced
- DroppedTelemetry
- WriteRejected
- WriteTimeout
- WriteBusyExhausted
- WriteFailed

### Idempotency And Replay

- **Contract:** Every WriteOperation declares idempotency_key and replay_policy on the WriteOperation type itself.
- **Class A:** Use natural keys plus IF NOT EXISTS or UPSERT where the domain permits. If a barrier mutation is not naturally idempotent, the caller must declare replay_policy=caller_guarded and provide a duplicate-application test.
- **Class B:** Use the coalescing key as the idempotency key with last-writer-wins semantics ordered by monotonic observed_at where available.
- **Class C:** Use the unique key (run_id, relative_path). Retries use INSERT OR IGNORE or UPSERT only when checksum and size match; mismatches return WriteFailed and increment evidence_metadata_conflict_total.
- **Class D:** Use metric bucket key and time window; duplicate rollups merge by additive counters or max gauges.

### Shutdown Protocol

#### Graceful Shutdown Order

- On shutdown signal, stop accepting new Class B, C, and D writes immediately.
- Continue accepting only Class A writes needed to persist shutdown or terminal canonical state.
- Drain Class A lanes first within a 5000 ms shutdown budget.
- Force-flush Class B coalescing buffers in one bounded pass with a 2000 ms sub-budget.
- Skip Class D rollups by default. A single best-effort telemetry flush may run only if Class A drain completed and budget remains.
- On drain timeout, log queue snapshots, unflushed coalescing keys, and evidence orphan candidates.

#### Hard Crash Recovery

- Class A relies on SQLite rollback and WAL recovery.
- Class C ordering makes metadata-without-bytes impossible by construction (metadata enqueue happens only after fsync(file) and fsync(parent_dir)).
- Bytes-without-metadata are allowed and recovered or garbage-collected by the startup orphan sweep.
- Class B may lose unflushed intermediate values but must flush at max age during normal operation.

### Coalescing

- **Key:** (run_id, surface, projection_kind) for projections; producer-specific stable keys for runtime status and session health.
- **Mandatory Flush:** Flush every 500 ms or every 64 merges globally, whichever happens first. Flush cadence is independent of producer signals so coalesced state cannot remain unflushed even if no boundary fires.
- **Max Key Age Ms:** `2000`

#### Flush Boundaries

- terminal run, stage, or agent transition
- operator command boundary
- projection materialization boundary
- daemon graceful shutdown force-flush

#### Metrics

- coalesced_merged_total
- coalesced_dropped_total
- coalesced_flush_age_ms (p50/p95)
- coalesced_keys_pending

### Evidence Spooling

- **Module Decision:** Path construction, atomic file creation, checksum, fsync, and orphan sweep live in an evidence spool module near artifact handling. SQLite metadata types and repositories live in control-plane/crates/db.
- **Layout:** {artifact_root}/evidence/runs/{run_id}/stages/{stage_id}/agents/{agent_id}/{transcripts|tool-traces|stdout|stderr|receipts|runtime-events|model-deltas|delivery-readbacks}/
- **Reserved P078 Layout:** {artifact_root}/effects/{effect_id}/ is reserved naming only. P075 does not create effect ids, expose effect readbacks, migrate effect records, run GraphQL or MCP semantics for the effects directory, or treat effect files as ledger truth.
- **Logical Object Boundary:** A producer writes one file per logical transcript segment, tool invocation output, runtime event bundle, or bounded model/readback bundle. It must not write one SQLite row per stream chunk.

#### File Ordering

- write to deterministic temp path under the final directory
- compute checksum while writing or immediately after close
- fsync(file)
- rename atomically to final name
- fsync(parent directory) when creating or renaming entries
- enqueue Class C metadata write

#### Failure Behavior

- File write, checksum, or fsync failure fails or degrades the producer before any metadata is written.
- Metadata failure after file fsync leaves an orphan-safe file with enough name structure for the startup sweep.
- Checksum mismatch rejects metadata insertion and marks the file for inspection.
- Readers never treat metadata as valid if the file is missing or checksum mismatched.

#### Orphan Recovery

- **Scope:** In P075.
- **Startup Sweep:** On daemon start, a bounded low-priority sweep walks evidence spool roots for active, completed, and abandoned runs, cross-checks evidence_spool_refs, and records orphan counts without blocking startup-critical paths.
- **Active Run Behavior:** For intact files with parseable ownership and matching checksum, insert recovery metadata with producer_operation=recovery_sweep and status=recovered_orphan.
- **Terminal Run Behavior:** For terminal runs, schedule deletion after a 7 day grace period unless an operator runs manual reconcile first.
- **Bounds:** Sweep is chunked by run, capped to 1000 files or 64 MB per pass by default, and resumes later via low-priority maintenance work.
- **Diagnostics:** storage.evidence_spool_summary reports orphan_files, orphan_bytes, recovered_files, checksum_mismatch_files, and pending_delete_files.

### Evidence Spool Ref Contract

- **Metadata Version:** `1`
- **Id Format:** evsp_ plus lower-case hex or UUID-derived stable identifier. The same logical file retry must preserve the same idempotency key even if id is regenerated before INSERT OR IGNORE.
- **Content Type:** Optional MIME-style string for reader hints only. It does not change trust or execution behavior.

#### Required Fields

- id
- metadata_version
- run_id
- kind
- relative_path
- size_bytes
- checksum_algorithm
- checksum
- producer_operation
- created_at

#### Optional Fields

- stage_execution_id
- stage_id
- agent_execution_id
- agent_id
- content_type
- summary_json

#### Kind Enum

- transcript
- tool_trace
- stdout
- stderr
- receipt
- runtime_event
- model_delta
- delivery_readback

#### Checksum Algorithm Enum

- sha256

#### Relative Path Rules

- Path is relative to artifact_root.
- Reject absolute paths.
- Reject parent traversal segments such as '..'.
- Reject empty path segments.
- Reject platform-specific separator ambiguity before insertion.
- Normalize to forward slash in persisted metadata.

#### Summary JSON

- **Max Bytes:** `8192`
- **Schema:** Bounded object for small facts such as line_count, chunk_count, truncated, first_timestamp, last_timestamp, and producer labels. Raw evidence text is forbidden.

#### Ownership Semantics

- run_id is always required.
- stage_execution_id identifies the concrete execution when available; stage_id is copied for stable grouping and may be absent for run-level evidence.
- agent_execution_id identifies the concrete agent execution when available; agent_id is copied for stable grouping and may be absent for stage-level evidence.
- Readers group by execution ids first and stable ids second.

#### Reader Statuses

- available
- legacy_absent
- missing_file
- checksum_mismatch
- recovered_orphan
- pending_delete

### Repository Boundary

#### Allowed Direct Write Owners

- DbWriter
- schema migrations and preflight code
- isolated tests and fixtures
- narrow startup repair and maintenance code listed in the allowlist

#### Allowlist Contract

- **Format:** checked-in TOML at control-plane/crates/db/write-bypass-allowlist.toml
- **Gate Behavior:** The proposal-075 gate diffs direct write call sites against the allowlist and fails on unlisted runtime write owners, on entries missing retirement data, or on entries whose expires_after_phase has passed.

##### Required Fields

- id
- owner
- reason
- scope
- path_pattern
- allowed_context
- retirement_criteria
- expires_after_phase

### Projection Invalidation

#### Policy

- Canonical state changes enqueue one invalidation as part of or immediately after the barrier transaction.
- Repeated invalidations for (run_id, surface, projection_kind) are coalesced.
- Materialization may run asynchronously.
- GraphQL exposes projection lag and freshness honestly via ProjectionStorageHealth.
- Subscriptions emit stable state changes and freshness changes, not every internal invalidation.

### WAL Checkpoint Policy

- **Auto Checkpoint:** Keep SQLite auto-checkpoint configured explicitly rather than implicit default. Initial target is 1000 frames unless current DB configuration already sets another documented value.
- **Passive Checkpoint:** A low-priority maintenance task may request PASSIVE checkpoint when WAL exceeds 128 MB and no Class A write is waiting.
- **Truncate Checkpoint:** TRUNCATE checkpoint is allowed only on graceful shutdown after Class A drain, or via explicit maintenance command.

#### Metrics

- wal_size_bytes
- checkpoint_duration_ms
- checkpoint_kind
- checkpoint_blocked_by_writer

## Data And Schema Changes

- **Forward Migration:** Forward-only. Existing databases migrate without losing artifact metadata. New tables start empty and GraphQL reports migration_empty until producers write spool metadata.
- **Downgrade Behavior:** Downgrade to a pre-P075 daemon retains evidence files inert on disk and ignores evidence_spool_refs. Re-upgrade runs the orphan sweep and reconciles intact files. Operators should not expect pre-P075 binaries to index or delete P075 evidence files.
- **Legacy Runs:** Runs created before P075 report legacy_absent for spool metadata. This is distinct from spool failure.

### Migrations

1. Evidence Spool Refs
   - **Table:** evidence_spool_refs

   ##### Columns

   - id TEXT PRIMARY KEY
   - metadata_version INTEGER NOT NULL DEFAULT 1
   - run_id TEXT NOT NULL
   - stage_execution_id TEXT
   - stage_id TEXT
   - agent_execution_id TEXT
   - agent_id TEXT
   - kind TEXT NOT NULL
   - relative_path TEXT NOT NULL
   - size_bytes INTEGER NOT NULL
   - checksum_algorithm TEXT NOT NULL
   - checksum TEXT NOT NULL
   - producer_operation TEXT NOT NULL
   - content_type TEXT
   - summary_json TEXT
   - created_at TEXT NOT NULL
   - status TEXT NOT NULL DEFAULT 'available'

   ##### Constraints

   - CHECK(metadata_version = 1)
   - CHECK(size_bytes >= 0)
   - CHECK(kind IN ('transcript','tool_trace','stdout','stderr','receipt','runtime_event','model_delta','delivery_readback'))
   - CHECK(checksum_algorithm IN ('sha256'))
   - CHECK(length(relative_path) > 0)
   - CHECK(summary_json IS NULL OR length(summary_json) <= 8192)

   ##### Indexes

   - idx_evidence_spool_refs_run_created on (run_id, created_at)
   - idx_evidence_spool_refs_stage_execution on (stage_execution_id)
   - idx_evidence_spool_refs_agent_execution on (agent_execution_id)
   - idx_evidence_spool_refs_kind on (kind)
   - uniq_evidence_spool_refs_run_relative_path UNIQUE on (run_id, relative_path)
2. Storage Write Pressure Snapshots
   - **Table:** storage_write_pressure_snapshots
   - **Purpose:** Compact optional storage health history when existing health surfaces cannot represent storage-specific semantics cleanly.
   - **Retention:** Keep 24 hours by default or the latest 288 five-minute windows, whichever is smaller.

   ##### Columns

   - id TEXT PRIMARY KEY
   - window_start TEXT NOT NULL
   - window_end TEXT NOT NULL
   - payload_json TEXT NOT NULL
   - created_at TEXT NOT NULL

## GraphQL Contract

- **Decision:** Add storageHealth as the additive P075 GraphQL surface. Do not rename or overload schedulerHealthSummary. Existing dbWriterContentionSummary remains stable and may share the same internal metric source.
- **Auth Scope:** Same operator diagnostics/read scope used for existing daemon health and scheduler health readbacks.
- **Freshness:** storageHealth.updatedAt, staleAfterMs, and isStale are mandatory. Initial staleAfterMs is 5000.
- **Subscription Behavior:** Storage health subscription/readback snapshots are throttled to 1 Hz by default and emit changes only when a threshold band changes or a snapshot is explicitly requested.

### Schema Delta

- extend type Query { storageHealth: StorageHealth! }
- type StorageHealth { updatedAt: DateTime!, staleAfterMs: Int!, isStale: Boolean!, dbState: StorageDbState!, writer: DbWriterHealth!, wal: WalHealth!, projections: ProjectionStorageHealth!, evidenceSpool: EvidenceSpoolSummary!, killSwitches: StorageKillSwitchState!, thresholds: [StorageHealthThreshold!]! }
- enum StorageDbState { HEALTHY DEGRADED STALE MIGRATION_EMPTY LEGACY_ABSENT }
- type DbWriterHealth { alive: Boolean!, lastHeartbeatAt: DateTime, lastDrainAt: DateTime, totalQueued: Int!, lanes: [DbWriterLaneHealth!]!, writeLockWaitP50Ms: Float, writeLockWaitP95Ms: Float, transactionDurationP95Ms: Float, busyRetryRatePerMinute: Float!, busyRetryExhaustedTotal: Int!, rejectedTotal: Int!, droppedTelemetryTotal: Int! }
- type DbWriterLaneHealth { lane: String!, capacity: Int!, queuedDepth: Int!, queuedDepthRatio: Float!, oldestQueuedAgeMs: Int, rejectedTotal: Int!, droppedTotal: Int! }
- type WalHealth { available: Boolean!, unavailableReason: String, sizeBytes: Int, warnSizeBytes: Int!, criticalSizeBytes: Int!, lastCheckpointAt: DateTime, checkpointDurationP95Ms: Float }
- type ProjectionStorageHealth { pendingInvalidations: Int!, projectionLagMs: Int, coalescedKeysPending: Int!, coalescedMergedTotal: Int!, coalescedFlushAgeP95Ms: Float }
- type EvidenceSpoolSummary { enabled: Boolean!, filesWrittenTotal: Int!, bytesWrittenTotal: Int!, metadataRowsTotal: Int!, orphanFiles: Int!, orphanBytes: Int!, recoveredFiles: Int!, checksumMismatchFiles: Int!, pendingDeleteFiles: Int! }
- type StorageKillSwitchState { dbWriterBypassClasses: [WriteClass!]!, coalescingDisabledKeys: [String!]!, evidenceSpoolDisabledKinds: [EvidenceKind!]! }
- enum WriteClass { A B C D }
- enum EvidenceKind { TRANSCRIPT TOOL_TRACE STDOUT STDERR RECEIPT RUNTIME_EVENT MODEL_DELTA DELIVERY_READBACK }
- type StorageHealthThreshold { metric: String!, warn: Float!, critical: Float!, unit: String!, action: String! }

### Nullability And Units

- Fields with unavailable platform data are nullable and paired with available=false or unavailableReason.
- Durations use milliseconds and field names end in Ms.
- Sizes use bytes and field names end in Bytes.
- Rates state their unit in the field name, for example busyRetryRatePerMinute.

### Legacy And Migration Readbacks

- legacy_absent means a run predates evidence_spool_refs and has no spool metadata.
- migration_empty means the table exists but no producer has written spool metadata.
- spool failures are represented by missing_file, checksum_mismatch, or orphan counts, not by legacy_absent.
- WAL unavailable is represented by WalHealth.available=false with unavailableReason, while the rest of storageHealth remains usable.

## MCP Contract

- **Decision:** P075 includes versioned MCP diagnostics for parity with GraphQL. They are diagnostics/maintenance surfaces, not UI mutation controls.
- **Naming:** External dot names are storage.health, storage.write_pressure, storage.evidence_spool_summary, and storage.reconcile_evidence_orphans. Codex-compatible tool identifiers may map dots to underscores while preserving the display name.
- **Error Semantics:** Tools return typed unavailable, stale, unauthorized, invalid_input, and maintenance_disabled errors. GraphQL and MCP must agree on units and enum string values.

### Capabilities

1. Storage.Health
   - **Tool:** storage.health
   - **Capability:** CapabilityToolId.storage.health
   - **Principal:** operator diagnostics read
   - **Output:** StorageHealth JSON shape equivalent to GraphQL storageHealth.

   ##### Input Schema

   - **Includethresholds:** boolean optional, default true
2. Storage.Write Pressure
   - **Tool:** storage.write_pressure
   - **Capability:** CapabilityToolId.storage.write_pressure
   - **Principal:** operator diagnostics read
   - **Output:** Windowed queue, wait, retry, transaction, rejection, drop, and WAL metrics with the same units as GraphQL.

   ##### Input Schema

   - **Windowseconds:** integer optional, default 300, min 30, max 3600
   - **Includelanes:** boolean optional, default true
3. Storage.Evidence Spool Summary
   - **Tool:** storage.evidence_spool_summary
   - **Capability:** CapabilityToolId.storage.evidence_spool_summary
   - **Principal:** operator diagnostics read
   - **Output:** EvidenceSpoolSummary plus per-kind counts and byte totals.

   ##### Input Schema

   - **Runid:** string optional
   - **Includeorphans:** boolean optional, default true
4. Storage.Reconcile Evidence Orphans
   - **Tool:** storage.reconcile_evidence_orphans
   - **Capability:** CapabilityToolId.storage.reconcile_evidence_orphans
   - **Principal:** operator maintenance
   - **Output:** Counts for scanned, recovered, checksum_mismatch, scheduled_delete, skipped, and errors.

   ##### Input Schema

   - **Runid:** string optional
   - **Dryrun:** boolean optional, default true
   - **Maxfiles:** integer optional, default 1000

## Telemetry Metrics And Thresholds

- **Baseline:** Before Phase 2, capture current write-lock wait p50/p95, busy retry rate, command latency p50/p95, WAL size, and direct write inventory under a canned workload. Store the baseline under docs/evidence or the run artifact area and compare each rollout phase against it.

### Rollup Budget

- **Memory Cap Bytes:** `1048576`
- **Max Samples:** `10000`
- **Flush Cadence Ms:** `5000`
- **Snapshot Ttl Hours:** `24`
- **Shutdown Semantics:** Abandon Class D by default on shutdown. Perform one best-effort flush only after Class A drain if time remains.

### Structured Logs

- Every barrier write logs operation_name, write_id, idempotency_key_hash, class, lane, wait_ms, lock_wait_ms, tx_ms, retries, expected_rows, actual_rows, result.
- Busy retry and queue warning logs include lane, queued_depth, oldest_queued_ms, wal_size_bytes when available, and kill_switch_state.
- Evidence spool failures log run_id, kind, relative_path hash or redacted path, size_bytes, checksum_algorithm, and recovery status.

### Initial Thresholds

1. Queued Depth Ratio By Lane
   - **Metric:** queued_depth_ratio_by_lane
   - **Warn:** `0.5`
   - **Critical:** `0.8`
   - **Action:** Inspect producer rate; disable affected coalescing key or evidence kind via kill-switch if needed.
2. Oldest Queued Age Ms Class A
   - **Metric:** oldest_queued_age_ms_class_a
   - **Warn:** `500`
   - **Critical:** `1500`
   - **Action:** Inspect lock holder and busy retries; consider DbWriter bypass for affected class during rollout.
3. Write Lock Wait P95 Ms
   - **Metric:** write_lock_wait_p95_ms
   - **Warn:** `100`
   - **Critical:** `500`
   - **Action:** Inspect long readers, WAL growth, and direct write bypasses.
4. Class A Transaction Duration P95 Ms
   - **Metric:** class_a_transaction_duration_p95_ms
   - **Warn:** `50`
   - **Critical:** `200`
   - **Action:** Audit transaction body for prohibited I/O or large payloads.
5. Busy Retry Rate Per Minute
   - **Metric:** busy_retry_rate_per_minute
   - **Warn:** `5`
   - **Critical:** `30`
   - **Action:** Check write contention and checkpoint behavior.
6. WAL Size Bytes
   - **Metric:** wal_size_bytes
   - **Warn:** `134217728`
   - **Critical:** `536870912`
   - **Action:** Allow PASSIVE checkpoint when no Class A write is waiting; schedule maintenance if sustained.
7. Checkpoint Duration P95 Ms
   - **Metric:** checkpoint_duration_p95_ms
   - **Warn:** `250`
   - **Critical:** `1000`
   - **Action:** Defer non-critical writes and inspect reader pressure.
8. Coalesce Ratio For High Churn Keys
   - **Metric:** coalesce_ratio_for_high_churn_keys
   - **Warn:** `1.5`
   - **Critical:** `1.0`
   - **Action:** Review whether the producer should be Class A, should spool evidence, or should reduce update frequency.
9. Evidence Orphan Bytes
   - **Metric:** evidence_orphan_bytes
   - **Warn:** `10485760`
   - **Critical:** `104857600`
   - **Action:** Run storage.reconcile_evidence_orphans in dry-run, then maintenance reconcile if safe.

## UX And UI Notes

- **Operator Experience:** The operator sees better stability and diagnostics, not new control flows.
- **Warning Style:** Warnings are operational: high queue depth, old queued barrier, sustained lock wait, evidence spool failure, orphan growth, WAL growth, checkpoint delay, or intentional safe-mode kill-switches.

### Swiftui Boundary

- SwiftUI remains a thin GraphQL/MCP client and does not read SQLite or artifact files directly.
- New storage diagnostics are typed, low-churn, read-only, and freshness-aware.
- High-frequency metrics are not placed in the SwiftUI primary state tree.

### Diagnostics

- GraphQL exposes storageHealth as the additive storage surface.
- schedulerHealthSummary and existing dbWriterContentionSummary remain stable for P061 compatibility.
- Storage snapshots are throttled to at most one update per second for UI/subscription consumers unless explicitly polled.
- DaemonDiagnosticsExportCommand and DiagnosticsBundleBuilder include storageHealth, storage write pressure snapshots, and evidence spool summary once implemented.

## Required Tests And Gate Coverage

### Unit And Integration Tests

- DbWriter priority: critical barrier is not starved by queued telemetry or coalesced work.
- Queue overflow behavior per lane returns WriteRejected or increments drop/merge counters according to class policy.
- Deadline expiry returns WriteTimeout and increments metrics; busy retry exhaustion returns WriteBusyExhausted.
- Class A idempotent retry does not double-apply where natural keys permit; caller_guarded operations have explicit duplicate-application tests.
- Caller cancellation removes queued writes and does not cancel in-flight transactions.
- Graceful shutdown drains Class A first, force-flushes Class B once, and abandons Class D unless budget remains.
- Evidence spool metadata round-trips with valid enum, relative path, checksum, size, ownership, and summary_json bounds.
- Evidence file fsync-before-metadata ordering is exercised with failure injection where practical.
- Checksum mismatch and missing file produce reader statuses rather than silent success.
- Startup orphan sweep recovers intact active-run files and schedules terminal-run deletion after grace period.
- High-volume fake stream produces bounded files and one metadata pointer per logical object, not one row per chunk.
- Coalesced projection invalidation merges repeated invalidations and flushes by interval, merge count, max age, and terminal boundary.
- GraphQL storageHealth exposes units, freshness fields, kill-switch state, WAL unavailable behavior, and legacy_absent versus migration_empty.
- MCP storage diagnostics match GraphQL units and enum values.
- Direct write bypass detection reports new runtime write owners unless explicitly allowlisted.
- Diagnostics export includes storageHealth and evidence spool summary.

### Canonical Gate Commands

- ./scripts/test-gate.sh proposal-075
- ./scripts/test-gate.sh p075

### Gate Must Prove

- Barrier writes are serialized, short, deadline-bound, and measured.
- Coalesced writes are batched or superseded safely and cannot remain unflushed indefinitely under normal operation.
- Evidence spooling stores file pointers, checksums, size, kind, ownership, and status with valid path rules.
- High-volume evidence is not persisted as stream chunks in SQLite.
- Telemetry is rolled up and cannot starve Class A.
- Direct DB write bypasses are removed or explicitly whitelisted for migrations, tests, startup repair, or temporary rollout with retirement criteria.
- storageHealth and MCP diagnostics provide objective warn/critical threshold readbacks.

## Acceptance Criteria

- All non-test runtime writes route through DbWriter or a source-controlled temporary bypass allowlist with owner, reason, scope, allowed context, and retirement criteria.
- Write classes, lanes, deadlines, queue capacities, overflow policies, idempotency keys, replay policies, and WriteResult variants are implemented and documented.
- Barrier writes are serialized, prioritized, short, measured, and not starved by telemetry or coalesced projection work.
- High-volume runtime evidence spools to files with EvidenceSpoolRef metadata, not row-per-chunk SQLite inserts.
- Evidence spool readers handle available, legacy_absent, missing_file, checksum_mismatch, recovered_orphan, and pending_delete states.
- Startup orphan recovery and manual reconcile diagnostics exist.
- Projection invalidations are coalesced with mandatory periodic and max-age flushes.
- Telemetry is rolled up with memory cap, TTL, structured logs, and no shutdown priority over barriers.
- storageHealth GraphQL and MCP diagnostics expose typed units, freshness, thresholds, kill-switch state, and degraded/unavailable cases.
- Forward-only migration and downgrade/re-upgrade behavior are documented.
- P078 can later use the barrier/evidence discipline without SQLite becoming an event stream.
- The proposal-075 and p075 gates fail on unapproved direct runtime write bypasses or high-volume raw evidence persisted directly into SQLite.

## Risks And Mitigations

1. DbWriter becomes a visible bottleneck.
   - **Risk:** DbWriter becomes a visible bottleneck.
   - **Mitigation:** SQLite already has one practical writer; P075 makes the bottleneck explicit, bounded, prioritized, measured, and switchable during rollout.
2. Queue bounds reject important work under load.
   - **Risk:** Queue bounds reject important work under load.
   - **Mitigation:** Class A is never dropped and has a deadline-based WriteRejected result; lower classes absorb load through coalescing or drops with metrics; warn/critical thresholds catch saturation early.
3. Evidence files and metadata diverge.
   - **Risk:** Evidence files and metadata diverge.
   - **Mitigation:** Use file-before-metadata ordering, checksum, fsync(file)+fsync(parent_dir), deterministic paths, unique metadata keys, startup orphan sweep, and manual reconcile tooling.
4. Coalescing hides meaningful intermediate state.
   - **Risk:** Coalescing hides meaningful intermediate state.
   - **Mitigation:** Only declared Class B writes can coalesce. Barrier transitions remain lossless. Mandatory periodic flush and max key age make final state visible even without producer signals.
5. Metrics add write pressure.
   - **Risk:** Metrics add write pressure.
   - **Mitigation:** Roll up in memory, cap memory at 1 MiB and 10000 samples, flush on low-priority Class D, and abandon telemetry on shutdown if it would delay barriers.
6. Kill-switches preserve legacy risk too long.
   - **Risk:** Kill-switches preserve legacy risk too long.
   - **Mitigation:** Expose switch state in storageHealth, require owner/retirement criteria in the allowlist, and fail the gate when temporary bypasses outlive their phase.
7. Forward-only migration complicates downgrade.
   - **Risk:** Forward-only migration complicates downgrade.
   - **Mitigation:** Document downgrade as inert retained files plus re-upgrade reconciliation; do not rely on older daemons to interpret P075 evidence.

## Review Feedback Resolution

- **Source Review Pass Id:** p075-proposal-review-pass-1
- **Aggregate Score Before Revision:** `7.84`

### Blocking Items Resolved

- AC-001: Added exact storageHealth GraphQL schema delta with names, types, units, freshness, auth, WAL-unavailable behavior, and P061 compatibility.
- AC-002: Defined MCP diagnostic and maintenance tools, input schemas, output parity, capability ids, principal classes, dot-name mapping, and error semantics.
- AC-003: Added versioned EvidenceSpoolRef contract with enums, metadata_version, path rules, ownership semantics, summary bounds, content type behavior, and reader statuses.
- AC-004: Added migration constraints, indexes, legacy_absent versus migration_empty semantics, WAL unavailable behavior, and forward-only downgrade/re-upgrade behavior.
- R-01: Added bounded lanes, capacities, per-class overflow policy, WriteRejected behavior, lane_starvation_total, and rejected/dropped metrics.
- R-02: Added per-class deadlines, caller override rules, deadline scope, queued timeout, in-flight behavior, and typed WriteTimeout.
- R-03: Added idempotency key and replay policy requirements declared on the WriteOperation type for all classes.
- R-04: Added shutdown drain order, budgets, partial flush semantics, hard crash recovery expectations, and telemetry abandonment rule.
- R-05: Moved orphan recovery into P075 through bounded startup sweep and manual reconcile diagnostics.
- R-06: Made Class B periodic flush mandatory with interval, merge count, max key age, and metrics independent of producer signals.
- OBS-1: Added starter numeric thresholds and operator actions.
- OBS-2: Added rollout kill-switches and storageHealth visibility.
- OBS-3: Declared forward-only migration and downgrade behavior with re-upgrade reconciliation.

### Non Blocking Items Resolved

- AC-005: Declared no workflow YAML, agent catalog, protobuf, or OpenAPI changes.
- AC-006 and OBS-8: Chose a checked-in TOML bypass allowlist with owner, reason, scope, path pattern, context, retirement, and phase expiry.
- AC-007: Clarified reserved effects path is non-operational in P075.
- R-07: Added WriteBusyExhausted result.
- R-08: Added caller cancellation behavior.
- R-09: Added fsync(file) and fsync(parent directory) ordering.
- R-10 and OBS-5: Reconciled telemetry shutdown behavior and added memory cap, cadence, and TTL.
- R-11: Added WAL checkpoint policy.
- OBS-4: Added threshold/action table.
- OBS-6: Added DbWriter heartbeat and lastDrainAt.
- OBS-7: Added orphan counts and manual reconcile MCP tool.
- OBS-9: Added pre-Phase-2 baseline capture.
- OBS-10: Resolved naming by adding storageHealth without overloading schedulerHealthSummary.
- OBS-11: Added structured log fields.
- OBS-12: Added per-producer canary protocol.
- APPLE-1 and APPLE-2: Added typed, low-churn, freshness-aware Apple client read-contract notes and snapshot throttling.
- MAC-1: Added diagnostics export integration requirement.
- MAC-2: Evidence kind enum and producer operation remain human-readable in readbacks.
- MAC-3: Reinforced keeping high-frequency metrics out of SwiftUI primary state.

### Disagreements

- MCP diagnostics were previously optional. This revision chooses to include versioned read diagnostics and one maintenance reconcile tool in P075 because reviewers found named-but-unspecified optional tools ambiguous. This increases scope slightly but removes contract uncertainty.
- Per-write-class DbWriter bypass is intentionally temporary and visible in storageHealth. It is accepted as rollout safety despite the long-term goal of removing direct runtime writes; the gate requires retirement criteria and fails when expires_after_phase passes.

## Rollout

- **Strategy:** Phased rollout behind implementation gates and runtime kill-switches, not a user-facing feature flag.

### Kill Switches

1. DbWriter.Bypass Classes
   - **Name:** dbwriter.bypass_classes
   - **Scope:** temporary rollout only
   - **Behavior:** Route selected classes back to documented legacy write paths when available. Current state is exposed in storageHealth.killSwitches.
2. DbWriter.Coalescing Disabled Keys
   - **Name:** dbwriter.coalescing_disabled_keys
   - **Scope:** per coalescing key or prefix
   - **Behavior:** Persist Class B updates without coalescing for a problematic key while keeping metrics visible.
3. Evidence Spool.Disabled Kinds
   - **Name:** evidence_spool.disabled_kinds
   - **Scope:** per producer kind
   - **Behavior:** Disable spooling for a producer kind during rollout; producer must fall back to previous bounded behavior or fail explicitly.

### Phases

- Phase 1: Add types, schema, allowlist contract, baseline capture, and tests with no behavior change.
- Phase 2: Implement DbWriter, heartbeat, deadlines, backpressure, idempotency contract, metrics, and route one low-risk write path through it.
- Phase 3: Add evidence spool and startup orphan sweep; prove fake high-volume stream creates files plus one metadata row per logical object.
- Phase 4: Canary one high-volume ACP/transcript producer with spooling enabled; soak until queue depth, orphan count, and command latency stay below warn thresholds.
- Phase 5: Convert remaining ACP/tool/transcript/runtime-event producers one kind at a time with rollback switches available.
- Phase 6: Add storageHealth GraphQL and MCP diagnostics; include storage fields in DaemonDiagnosticsExportCommand and DiagnosticsBundleBuilder.
- Phase 7: Tighten proposal-075 gate from inventory mode to fail-closed mode for unapproved direct runtime writes and row-per-chunk evidence writes.
- Phase 8: Retire temporary bypass allowlist entries and publish docs/reference/local-persistence-write-budget.md or equivalent.

### Canary Protocol

- Enable one producer kind for one run or controlled workload.
- Soak for at least 30 minutes or one representative workflow batch.
- Promote only if Class A p95 latency, write-lock wait, queue depth, WAL size, and orphan bytes remain below warn thresholds.
- Rollback by disabling the producer kind or bypassing the affected write class, then record the kill-switch state in storageHealth.

### Compatibility

- Existing GraphQL fields remain stable.
- New diagnostics are additive.
- Legacy runs without spool metadata report legacy_absent.
- Existing artifact files remain valid.
- The bypass allowlist is source-controlled and reviewed in PRs.

## Open Questions

1. Entry
   - **Question:** Should initial numeric thresholds be tightened after the Phase 1 baseline?
   - **Current Resolution:** Yes. Thresholds in this proposal are starter warn/critical values. Phase 1 captures baseline evidence and may lower or raise them with reviewable rationale before fail-closed rollout.
2. Entry
   - **Question:** Which exact current ACP/runtime producer should be the first canary in the implementation branch?
   - **Current Resolution:** After fake-stream proof, choose the transcript/model-delta producer with the highest observed row-per-chunk pressure and lowest semantic coupling. The proposal requires a single-producer canary before broad conversion.

## Rollout Contract V1

```json
{
  "schema_version": "rollout_contract_v1",
  "applicability": "required",
  "gate_aliases": [
    "proposal-075",
    "p075"
  ],
  "commands": {
    "allowlist": [
      "./scripts/test-gate.sh proposal-075",
      "./scripts/test-gate.sh p075"
    ],
    "commentary": "Use the repository gate wrapper for P075 rollout validation; raw build, SQLite, or daemon commands are outside this contract."
  },
  "migrations": {
    "not_applicable": false,
    "description": "Forward-only storage migration evidence must cover evidence_spool_refs and storage_write_pressure_snapshots creation, legacy_absent versus migration_empty readbacks, downgrade as inert retained files, re-upgrade orphan sweep reconciliation, and no loss of existing artifact metadata."
  },
  "metrics": {
    "adoption_metric": "storage_health_rollout_pass_rate",
    "operational_metrics": [
      "queued_depth_ratio_by_lane",
      "oldest_queued_age_ms_class_a",
      "write_lock_wait_p95_ms",
      "class_a_transaction_duration_p95_ms",
      "busy_retry_rate_per_minute",
      "wal_size_bytes",
      "checkpoint_duration_p95_ms",
      "coalesce_ratio_for_high_churn_keys",
      "evidence_orphan_bytes",
      "storage_health_rollout_pass_rate",
      "dbwriter_bypass_allowlist_retirement_total",
      "direct_runtime_write_bypass_total",
      "evidence_spool_metadata_conflict_total",
      "evidence_spool_orphan_recovered_total",
      "storage_reconcile_evidence_orphans_total"
    ]
  },
  "readback_lanes": [
    "run_report",
    "mcp",
    "release_receipt",
    "graphql"
  ],
  "readback_fields": [
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps"
  ],
  "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p075-full-surface.fixture.json",
  "operator_report_fields": [
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps"
  ],
  "hold_conditions": [
    "Class A p95 latency, write-lock wait, queue depth, WAL size, or orphan bytes exceed critical thresholds during canary.",
    "Evidence spool writes raw stream chunks to SQLite instead of one metadata row per logical object.",
    "storageHealth GraphQL and MCP diagnostics disagree on units, freshness, threshold bands, or kill-switch state.",
    "Direct runtime write owners are not routed through DbWriter or the source-controlled temporary bypass allowlist.",
    "Startup orphan sweep cannot recover intact active-run evidence files or reports checksum mismatches as success.",
    "Temporary bypass allowlist entries lack owner, reason, scope, allowed context, retirement criteria, or exceed their rollout phase."
  ],
  "rollback_disposition": {
    "mode": "disable_p075_storage_routing_keep_diagnostics",
    "data_loss_risk": "low",
    "steps": [
      "Disable affected DbWriter bypass classes, coalescing keys, or evidence spool producer kinds through rollout kill-switches.",
      "Route affected classes back to documented legacy paths only where the proposal permits a temporary bypass.",
      "Keep storageHealth, MCP diagnostics, evidence_spool_refs, and written evidence files available for audit and reconciliation.",
      "Run storage.evidence_spool_summary and storage.reconcile_evidence_orphans in dry-run before re-enabling producer kinds."
    ]
  },
  "decision_vocabulary": [
    "pass",
    "fail",
    "waived",
    "not_applicable",
    "timeout",
    "cancelled",
    "missing_contract",
    "tamper_detected",
    "stale",
    "release",
    "hold",
    "waive"
  ],
  "negative_fixtures": {
    "missing_storage_readback": "docs/evidence/rollout-contract/negative/p075-missing-storage-readback.json"
  },
  "operator_message": "P075 rollout requires staged DbWriter routing, evidence spooling, storageHealth parity, bypass retirement, and threshold-based canary promotion."
}
```
