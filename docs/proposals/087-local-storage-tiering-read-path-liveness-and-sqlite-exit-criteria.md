# Proposal 087: Local Storage Tiering, Read-Path Liveness, and SQLite Exit Criteria

| Field | Value |
|---|---|
| Date | 2026-05-08 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Implemented SQLite write-discipline / evidence-spooling baseline, [durable side-effect ledger](../reference/rust-control-plane.md#durable-side-effect-ledger), UI Action Boundary reference |
| Related | P038 Run Compaction, P079 Contract-Aware Output Repair, P080 Continuous Stale Execution Reconciliation, P086 Agent Work Continuation |
| Scope | Formalize Plan A for local storage: keep SQLite as compact canonical state DB, move high-volume evidence to file spool, serve hot reads from materialized/in-memory projections, add MCP/read-path liveness rules, and define explicit exit criteria for moving parts of storage elsewhere. |
| Goal | Avoid turning SQLite into a choke point while preserving the local single-process architecture. Prevent future durability and recovery proposals from increasing write pressure or blocking MCP/GraphQL read surfaces. |

---

## 1. Current repository status

This proposal is written against the current `main` branch state.

Observed current state:

- `docs/ROADMAP.md` treats provider toolchain cache mapping as a completed/reference prerequisite and says the local persistence write-budget / evidence-spooling infrastructure is implemented and remains the persistence safety baseline.
- the durable side-effect ledger implementation depends on the implemented local persistence write-budget contract rather than active proposal text.
- `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` also depends on the implemented write-budget contract and requires spooled continuation evidence.
- `control-plane/crates/db/src/writer.rs` already contains the DbWriter implementation with six priority lanes, write classes, coalescing, WAL policy, shutdown drain rules, and evidence metadata lane.
- `control-plane/crates/db/src/evidence_spool.rs` exists and should remain the path for high-volume evidence bytes.
- There is no active P075 proposal in the current proposal directory. The write-budget baseline is now implemented/reference infrastructure, not an active workstream.

Therefore this proposal must not say “implement P075.”
It says:

> Use the implemented write-budget baseline, then add read-path liveness, storage tiering, and exit criteria.

---

## 2. Why this proposal exists

The local Rust control plane intentionally uses:

- one local server process,
- SQLite,
- local artifact/evidence files,
- ACP runtimes,
- GraphQL for UI reads/subscriptions,
- MCP for external control.

This is still the desired shape.

However, recent work exposed a serious risk:

> even if writes are disciplined, MCP and GraphQL can still become slow or stuck if read paths are heavy, maintenance work holds request handlers, or SQLite remains the universal source for every hot read and evidence detail.

Timeouts alone are not a strategy.
Moving everything to another database immediately is also premature.

Plan A is a middle path:

> keep SQLite for compact canonical state, move high-volume evidence to files, and serve hot read surfaces from materialized/in-memory projections with explicit storage health and exit criteria.

This proposal turns Plan A into an implementation contract.

---

## 3. Core decision

The system adopts a three-tier local storage model.

## 3.1 Tier 1 — SQLite canonical compact state

SQLite stores only compact authoritative state and metadata:

- ideas,
- runs,
- stages,
- agent executions,
- approvals,
- work items,
- command journal summaries,
- side-effect intent / settlement compact rows,
- artifact metadata,
- projection cursors,
- storage health snapshots,
- compact read models.

SQLite must not store high-volume event streams.

## 3.2 Tier 2 — file-backed evidence and artifact store

The file system stores large or high-volume data:

- ACP transcripts,
- tool traces,
- stdout/stderr,
- model stream deltas,
- release readback payloads,
- continuation evidence,
- reconciliation reports,
- raw runtime event bundles,
- large reports and artifacts.

SQLite stores only:

- path,
- checksum,
- size,
- artifact kind,
- ownership IDs,
- provenance,
- timestamps.

## 3.3 Tier 3 — hot read projections

Hot read surfaces should not perform deep scans or N+1 attachment passes.

They should read from:

- materialized projection rows,
- in-memory read cache,
- cached summaries,
- precomputed health snapshots.

The server may rebuild those projections from SQLite + file metadata after restart.

---

## 4. Non-goals

This proposal does **not**:

- migrate canonical state to Postgres;
- introduce RocksDB;
- replace SQLite;
- undo DbWriter;
- change durable side-effect ledger semantics;
- change the UI action boundary;
- add new UI write controls;
- add new ACP provider families;
- implement run compaction itself.

This proposal defines when SQLite remains acceptable and when future migration becomes justified.

---

## 5. Storage ownership rules

## 5.1 SQLite may own

SQLite may own:

- canonical run/stage/agent/approval rows;
- compact side-effect rows;
- command journal summaries;
- artifact metadata pointers;
- projection invalidation cursors;
- compact projection tables;
- storage health snapshots.

## 5.2 SQLite must not own

SQLite must not own:

- one row per ACP stream chunk;
- one row per tool-progress event;
- raw stdout/stderr bytes;
- long transcripts;
- large runtime event payloads;
- full model output streams;
- repeated telemetry samples;
- large reconciliation payloads;
- large compaction bundles.

## 5.3 File store owns

The file store owns:

- raw evidence,
- raw logs,
- transcript bundles,
- tool trace bundles,
- high-volume runtime evidence,
- large reports,
- compaction archive bundles.

---

## 6. Read-path liveness rules

## 6.1 `runs.list` must be projection-only

`runs.list` must remain fast and must not perform deep enrichment.

Rules:

- no N+1 artifact/report attachment passes;
- no filesystem scans;
- no transcript reads;
- no compaction archive inspection;
- no side-effect evidence readback;
- no implementation self-assessment attachment unless pre-materialized in the projection.

If a caller needs detail, it must use:

- `runs.get`,
- `reports.get`,
- `artifacts.get`,
- or a specific diagnostic tool.

## 6.2 MCP read tools must not hold long work

MCP read tools should return quickly.

Long-running maintenance tools must return an operation id or accepted status instead of holding the request until all work completes.

Required behavior:

- short read tools have strict time budgets;
- long operations are asynchronous;
- liveness/health tools fail fast with typed degraded status;
- one stuck tool must not block all future control-plane reads.

## 6.3 GraphQL subscriptions read projections

GraphQL live surfaces must be driven by projections and compact state changes.

Subscriptions must not stream raw evidence chunks.

Examples:

- run status update;
- stage status update;
- approval inbox update;
- runtime health update;
- compaction status update;
- continuation status update.

Raw evidence remains available through artifact/report detail reads.

---

## 7. Hot read projection set

Initial required hot projections:

## 7.1 `ActiveRunIndex`

Used for:

- `runs.list`,
- UI runs home,
- MCP runs list,
- quick run status.

Contains:

- run id,
- title,
- status,
- current stage id,
- current stage label,
- blocked reason,
- approval pending flag,
- last updated at,
- compact summary fields.

## 7.2 `ApprovalInboxProjection`

Used for:

- UI approval inbox,
- GraphQL approval subscriptions.

Contains:

- approval id,
- run id,
- stage id,
- status,
- title/context summary,
- artifact/report pointers,
- decision state.

## 7.3 `RuntimeHealthProjection`

Used for:

- runtime status panel,
- MCP runtime health,
- storage health summary.

Contains:

- ACP runtime family,
- active sessions,
- degraded flags,
- write pressure flags,
- side-effect unresolved count,
- continuation active count.

## 7.4 `StorageHealthProjection`

Used for:

- storage health,
- operator diagnostics,
- regression gates.

Contains:

- DbWriter alive,
- queue depths by lane,
- p95/p99 write wait if available,
- WAL size,
- checkpoint status,
- evidence spool pending/orphan count,
- projection lag.

## 7.5 `ArtifactNoiseProjection`

Used for:

- P038 compaction readiness,
- run inspectability warnings.

Contains:

- artifact count per run,
- superseded count,
- duplicate candidate count,
- archive-eligible count,
- compaction recommended flag.

---

## 8. In-memory projection cache

The server may hold in-memory read caches for hot projections.

Rules:

- caches are derived, never canonical;
- caches rebuild from SQLite after restart;
- stale caches must return explicit freshness/degraded metadata;
- cache rebuild must not block MCP liveness;
- updates come from projection invalidation events or compact canonical changes.

Suggested caches:

- active runs cache,
- approval inbox cache,
- runtime health cache,
- storage health cache,
- side-effect unresolved cache,
- continuation status cache.

---

## 9. Interaction with implemented write-budget baseline

This proposal does not replace DbWriter.

It adds read-path and storage-tiering rules around it.

Existing write-budget baseline remains:

- Class A barrier writes,
- Class B coalesced state writes,
- Class C evidence metadata,
- Class D telemetry rollups,
- evidence bytes spooled to files.

This proposal adds:

- hot projection ownership,
- read-path liveness,
- MCP read budgets,
- storage exit criteria.

---

## 10. Interaction with the Durable Side-Effect Ledger

The durable side-effect ledger must use Plan A as follows:

- `side_effects` rows are compact SQLite state;
- `side_effect_attempts` rows are compact attempt metadata;
- stdout/stderr/readback payloads go to files;
- reconciliation reports are artifacts;
- `effects.inspect` reads compact state first;
- `effects.reconcile` may perform readback but must not block generic MCP read tools;
- unresolved side-effect count is projected into hot read state.

The durable ledger must not create high-volume SQLite event rows.

---

## 11. Interaction with P086

P086 continuation must use Plan A as follows:

- continuation metadata in SQLite is compact;
- ACP transcript evidence is file-spooled;
- tool traces are file-spooled;
- worktree readback is an artifact;
- continuation status is projected;
- `agents.continuation_status` reads compact projection first.

P086 must not write one row per transcript or tool event.

---

## 12. Interaction with P038

P038 compaction must use Plan A as follows:

- compaction is a maintenance operation, not a hot read path;
- compaction writes use DbWriter / implemented write-budget contract;
- compaction evidence and archive bundles go to file store;
- compaction updates projections rather than forcing UI to deep-scan archives;
- `runs.list` shows compact status from projection, not by scanning artifacts.

---

## 13. MCP liveness requirements

Add a mandatory MCP liveness gate.

Minimum gate:

1. initialize;
2. tools/list;
3. `runs.list`;
4. `runtime.health`;
5. `storage.health`;
6. one simple resource/artifact metadata read.

All must return within configured read budgets even if:

- DbWriter is degraded,
- evidence spool has orphan candidates,
- a maintenance operation is running,
- a prior long operation is still in progress.

Long-running tools must not block the request loop.

If the MCP transport is single-request serialized, long-running tools must be converted to accepted-operation style or moved off the blocking path.

---

## 14. Storage health and metrics

Expose metrics through GraphQL readback and MCP diagnostics.

Required metrics:

- DbWriter alive flag;
- queue depth per write lane;
- write rejection count by lane;
- p50/p95/p99 write wait where available;
- p50/p95/p99 transaction duration where available;
- WAL size;
- checkpoint duration;
- checkpoint failed/stalled count;
- evidence spool bytes written;
- evidence spool orphan count;
- projection lag count;
- hot cache rebuild duration;
- read query latency for `runs.list`;
- MCP liveness gate duration.

---

## 15. Storage exit criteria

SQLite remains acceptable only if the system meets defined SLOs after Plan A hardening.

## 15.1 Warning thresholds

Open a storage review if any of these persist across real runs:

- p95 write-lock wait > 200 ms;
- p99 write-lock wait > 1 s;
- repeated `SQLITE_BUSY` retry exhaustion;
- WAL exceeds warning threshold for long periods;
- `runs.list` p95 > 500 ms after projection-only implementation;
- MCP liveness gate intermittently fails;
- projection lag repeatedly exceeds configured threshold.

## 15.2 Critical thresholds

Open a storage migration proposal if any of these persist after fixes:

- Class A barrier writes are regularly delayed by lower-priority work;
- WAL checkpoint starvation occurs during normal operation;
- `runs.list` or approval inbox cannot meet SLO from projections/cache;
- compaction/reconciliation makes ordinary reads unavailable;
- DbWriter queue saturation becomes common;
- evidence spool metadata still creates unacceptable DB pressure.

## 15.3 Migration options

If exit criteria are hit, evaluate:

### Option B1 — SQLite canonical + RocksDB/event store

Use SQLite for canonical relational state.
Use RocksDB or another embedded event store for high-volume runtime events / telemetry.

### Option B2 — SQLite canonical + stronger file spool

Keep SQLite and improve file-based event spool, if evidence volume is the only issue.

### Option C — Postgres canonical state

Move canonical state to Postgres if the remaining problem is relational write concurrency or query contention that cannot be solved locally.

No migration decision should be made without metrics.

---

## 16. Implementation phases

## Phase 1 — Audit and metrics

- add storage health snapshot;
- add read latency metrics for `runs.list`;
- add MCP liveness gate;
- expose DbWriter / WAL / projection lag readback;
- identify heavy list/detail paths.

## Phase 2 — Read-path liveness

- make `runs.list` projection-only;
- remove N+1 enrichments from list tools;
- add operation-id pattern for long maintenance tools;
- ensure health tools fail fast.

## Phase 3 — Hot projections

- implement `ActiveRunIndex`;
- implement `ApprovalInboxProjection`;
- implement `StorageHealthProjection`;
- add cache rebuild on startup;
- add projection freshness metadata.

## Phase 4 — Exit criteria gate

- add `proposal-087` gate;
- fail if MCP liveness/read SLOs regress;
- fail if high-volume evidence rows are introduced;
- fail if `runs.list` performs detail attachments.

## Phase 5 — Storage decision checkpoint

- run real workflows;
- inspect metrics;
- decide whether Plan A is sufficient;
- if not, open storage migration proposal.

---

## 17. Tests and gates

## 17.1 Required tests

- `runs.list` does not read artifact files;
- `runs.list` does not perform N+1 detail attachment;
- MCP liveness gate passes while maintenance operation is running;
- evidence spool writes do not create high-volume SQLite rows;
- projection cache can rebuild after daemon restart;
- storage health returns degraded status instead of hanging;
- compaction status is projection-backed;
- side-effect unresolved count is projection-backed.

## 17.2 Static checks

Fail if:

- a hot list tool reads transcripts or tool traces;
- a new SQLite table stores raw stream chunks;
- a GraphQL/MCP hot read does filesystem scans;
- a maintenance tool blocks ordinary read liveness;
- new code bypasses the implemented write-budget baseline without allowlist justification.

---

## 18. Acceptance criteria

P087 is complete when:

1. SQLite is explicitly limited to compact canonical state and metadata.
2. High-volume evidence is file-spooled.
3. `runs.list` is projection-only and meets read budget.
4. MCP liveness gate passes during normal operation.
5. GraphQL hot reads use projections/cache, not deep scans.
6. Storage health exposes write pressure, WAL, spool, and projection lag.
7. Storage exit criteria are documented and enforced by a gate.
8. the durable side-effect ledger, P038, and P086 can rely on the storage tiering contract without increasing SQLite pressure unexpectedly.

---

## 19. Final recommendation

Plan A remains the right near-term architecture:

> SQLite for compact canonical state, file store for evidence, hot projections for UI/MCP reads.

But Plan A is only valid if it is enforced.

The system should not pretend SQLite can serve as a universal database, event stream, evidence store, and hot dashboard source all at once.

This proposal keeps the local single-process architecture while giving the system a clear escape hatch if metrics prove SQLite is no longer sufficient.
