# Proposal 075 Implementation Audit R7

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Proposal state | Approved / active implementation closeout target |
| Audit mode | `auto` implementation audit |
| Audit timestamp | `2026-05-09T12:52:29+03:00` |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Branch | `cw/implement-proposal-075-local-p/4aeb45a9` |
| Audited HEAD | `343c8690c4dd226ac18ba370bfa6d7c6c9506407` |
| Compare base | `70b03d1af641bbcb76a745449cc18f9a8fddea4c` (`origin/main` merge-base) |
| Worktree status before report | Clean |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High |

## Prior Review Reuse

| Item | Result |
|---|---|
| Discovery command | `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Prior proposal-review artifacts | None found |
| Prior implementation audits | Present, but ignored for reviewer selection per skill instructions |
| Reviewer-selection reuse | Not reused |

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Rust persistence gateway, repository boundary, writer ownership, SQLite transaction ownership |
| `rust_reliability_reviewer` | Backpressure, queueing, deadlines, idempotency, replay, shutdown, duplicate application |
| `api_contract_reviewer` | GraphQL `storageHealth`, MCP storage diagnostics, enum/unit/readback parity |
| `observability_rollout_reviewer` | Fail-closed gate, baseline evidence, health readbacks, rollout bypass retirement |

Rejected close alternatives:

- `macos_ui_reviewer`: P075 only adds thin read-only diagnostics data plumbing; no new macOS UI workflow is central to this audit.
- `apple_arch_reviewer`: Swift changes are diagnostic bundle/client readback integration, not the primary persistence boundary.
- `rust_security_reviewer`: auth capability surfaces exist and tests pass, but no broad new security boundary is introduced beyond operator-only diagnostics; API/OPS cover the contract risk here.
- `product_reviewer`: no central product metric or decision checkpoint beyond rollout health gates.

## Proposal Contract Summary

P075 commits to keeping SQLite as canonical local state while routing non-test runtime writes through a bounded, priority-laned `DbWriter`, classifying every write operation, spooling high-volume evidence to files with compact metadata, coalescing repeated non-critical state, rolling up telemetry, exposing typed GraphQL/MCP storage diagnostics, and failing the proposal gate on unapproved direct runtime write bypasses.

Primary proposal anchors:

- Goals: all non-test runtime write transactions route through `DbWriter` or a source-controlled bypass allowlist; every `WriteOperation` declares class, lane, operation name, expected rows, batchability, barrier, deadline, idempotency key, and replay policy (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`).
- Idempotency: caller-guarded Class A mutations require duplicate-application tests (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:250`).
- Gate: fail on unlisted runtime direct-write owners (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:411`).
- Acceptance criteria: all non-test runtime writes route through `DbWriter`/allowlist, barriers are serialized/prioritized/measured, evidence spools to files, projection invalidations are coalesced, telemetry rolls up, diagnostics expose typed units, and P075 gates fail on unapproved direct bypasses or raw stream chunks (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:713`).

Platform/product scope: Rust control-plane data/persistence, worker, API diagnostics, and rollout/observability scope. macOS app involvement is read-only diagnostics packaging/client readback.

## Primary Service Flows

1. Runtime writes enter a bounded `DbWriter` lane, commit through the P061 `BEGIN IMMEDIATE` retry primitive, and surface queue/lock/transaction pressure through `storageHealth`.
2. High-volume evidence producers write, checksum, fsync, and rename files before enqueueing compact `evidence_spool_refs` metadata.
3. Projection/status writes coalesce by stable key and flush by interval, merge count, max age, terminal boundary, and shutdown.
4. Telemetry and write pressure roll up without starving Class A barriers and expose drop/reject counters.
5. Operators read storage diagnostics through GraphQL, MCP, diagnostics bundles, and gate/baseline artifacts.

## Fidelity Inventory

### Matches

- `WriteClass`, `WriteLane`, deadlines, lane capacities, and `WriteResult` variants exist in `control-plane/crates/db/src/write_class.rs`.
- `DbWriter` implements bounded lane channels, priority drain order, Class B coalescing support, heartbeat snapshots, shutdown admission/drain behavior, and transaction duration samples in `control-plane/crates/db/src/writer.rs`.
- Evidence spool migrations, path validation, file-before-metadata helpers, status readbacks, orphan sweep, and one-row-per-logical-object tests exist under `control-plane/crates/db`.
- GraphQL typed `storageHealth` and MCP `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, and `storage.reconcile_evidence_orphans` surfaces exist.
- `./scripts/test-gate.sh proposal-075` passed on audited HEAD and reports: `5 bypasses, 122 operations, 119 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`.

### Divergences

- The generic repository transaction bridge creates a fresh `DbWriter` per transaction, so most repository calls are not flowing through the daemon's single shared bounded writer.
- Several runtime paths still construct local writers from a pool rather than using the injected daemon writer.
- Evidence metadata helpers submit Class C/C-style operations to a writer but then re-enter repository helpers that open a separate registered Class A transaction inside the work closure.
- No production Class B operation is registered or observed, and the only Class D registry row found is not used by runtime code.
- `DroppedTelemetry` exists as a result variant, but the writer uses generic lane send/timeout behavior for Class D and `storageHealth` hard-codes dropped telemetry counters to zero.
- Caller-guarded duplicate-application evidence is a Markdown matrix, not executable operation-specific duplicate-application tests.

### Ambiguities / Evidence Gaps

- The gate proves a clean direct-write text scan, but it does not prove all writes share one writer instance or that registry class values match the transaction that actually opens SQLite.
- The audit did not run a live daemon workload; the canonical P075 gate and file-backed canary passed.
- Producer inventory says several evidence kinds are reserved/validated rather than currently active producers.

## Requirement Summary

| Requirement | Status | Evidence |
|---|---|---|
| REQ-001 Route non-test runtime writes through `DbWriter` or permanent allowlist | Partially Implemented | Direct scan clean and temporary bypasses retired, but registered/runtime helper paths create local writers instead of the shared daemon writer. |
| REQ-002 Every write operation declares class/lane/deadline/idempotency/replay and caller-guarded duplicate proof | Partially Implemented | Registry exists and gate checks rows; duplicate proof is mostly non-executable matrix documentation and class adoption is inconsistent. |
| REQ-003 Bounded priority-laned writer serializes/prioritizes/measures barriers system-wide | Partially Implemented | Component exists and tests pass; system-wide use is fragmented by per-call/local writers. |
| REQ-004 Evidence spool schema, path rules, statuses, file-before-metadata ordering, orphan recovery | Implemented | Migrations, validators, repository tests, high-volume fake stream, startup sweep, and MCP summary coverage pass. |
| REQ-005 High-volume evidence avoids row-per-chunk SQLite persistence | Implemented | Failed-stage evidence producer and ACP transcript path use spool pointers; raw-evidence gate scan passed. |
| REQ-006 Projection/status coalescing with mandatory flushes | Partially Implemented | Class B coalescing component and tests exist; no production Class B registry entry or non-test runtime use was found. |
| REQ-007 Telemetry rollup with memory cap, TTL, drop counters, and no priority over barriers | Partially Implemented | Constants/readbacks exist; runtime Class D rollup/drop behavior is not wired and counters are hard-coded zero. |
| REQ-008 GraphQL `storageHealth` typed units/freshness/thresholds/kill-switch state | Implemented | Typed schema tests and live writer heartbeat tests passed. |
| REQ-009 MCP storage diagnostics and reconcile tool parity | Implemented | MCP storage diagnostics tests passed, including typed errors and operator-only dispatch. |
| REQ-010 Forward migration, downgrade/re-upgrade/orphan behavior, docs/reference truth | Implemented | P075 migrations, startup sweep, baseline/reference docs, and gate evidence are present. |
| REQ-011 Proposal gate fails on unapproved direct writes/raw evidence and records baseline | Partially Implemented | Gate passed and direct scan is stronger, but misses shared-writer ownership, class consistency, real duplicate tests, and B/D production adoption. |
| REQ-012 Diagnostics export and thin Apple client readback | Implemented | Swift diagnostics bundle/client additions are present and out of the critical persistence path. |

## Detailed Requirement Audit

### REQ-001: Runtime Write Gateway

- Proposal source: Goals and Repository Boundary.
- Status: Partially Implemented.
- Evidence types: code, tests-run, config.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:205` creates `begin_registered_immediate_transaction`.
  - `control-plane/crates/db/src/writer.rs:216` constructs `Arc::new(DbWriter::new(pool.clone()))` per registered transaction.
  - `control-plane/crates/db/src/writer.rs:234` wraps repository writes through that helper.
  - `control-plane/crates/db/write-bypass-allowlist.toml:19` lists only permanent migration/test/startup-repair bypasses.
  - `./scripts/test-gate.sh proposal-075` passed with runtime direct SQL scan clean.
- Gap / note: raw direct writes were removed from the scanned runtime surfaces, but most repository transactions are now routed through short-lived writer instances rather than the daemon's shared writer.

### REQ-002: WriteOperation Registry and Replay Proof

- Proposal source: Goals and Idempotency And Replay.
- Status: Partially Implemented.
- Evidence types: code, config, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-operation-registry.toml:1` defines the operation registry contract.
  - `scripts/test-gate.sh:5567` validates required registry fields.
  - `scripts/test-gate.sh:5593` rejects missing/generic caller-guarded duplicate proof paths.
  - `docs/evidence/p075/operation-duplicate-application-matrix.md:5` lists caller-guarded rows.
- Gap / note: the matrix rows say "Covered by the owning repository/engine transaction test path plus `./scripts/test-gate.sh proposal-075`" but do not name or run operation-specific duplicate-application tests.

### REQ-003: DbWriter Lanes, Priority, Deadlines, Shutdown, Measurement

- Proposal source: DbWriter, Backpressure And Admission Control, Deadlines And Results, Shutdown Protocol.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:990` queues a transaction-start work item through a writer lane.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:70` tests Class A before Class D when both are queued.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:376` tests enqueue-to-commit deadline accounting.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:284` and `:312` test shutdown admission.
- Gap / note: component-level behavior is covered, but runtime repository/local writes are not all submitted to the same writer instance, so system-level priority/backpressure/measurement remains incomplete.

### REQ-004: Evidence Spooling and Orphan Recovery

- Proposal source: Evidence Spooling, Evidence Spool Ref Contract, Orphan Recovery.
- Status: Implemented.
- Evidence types: code, migration, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/migrations/046_p075_evidence_spool_refs.sql`.
  - `control-plane/crates/db/src/evidence_spool.rs`.
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs`.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:436` tests checksum/fsync ordering.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:523` tests startup orphan recovery.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:614` tests one metadata row per logical object.
- Gap / note: core spooling behavior is implemented. The writer-class routing issue for metadata is tracked under REQ-003/REQ-007 findings.

### REQ-005: High-Volume Evidence Producer Discipline

- Proposal source: Evidence Spooling and Gate Must Prove.
- Status: Implemented.
- Evidence types: code, tests-run, config.
- Evidence references:
  - `docs/evidence/p075/producer-inventory.md`.
  - `control-plane/crates/engine/src/evidence.rs`.
  - `control-plane/crates/engine/src/executor.rs:6422` records ACP transcript spool producer operation.
  - `./scripts/test-gate.sh proposal-075` raw-evidence scan passed.
- Gap / note: active high-volume producers are either spooled or inventoried as reserved/no-current-producer.

### REQ-006: Coalescing

- Proposal source: Coalescing and Projection Invalidation.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:909` routes Class B through a coalescing buffer.
  - `control-plane/crates/db/src/writer.rs:1183` implements Class B last-writer-wins.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:688` tests 64-merge/500 ms coalescing behavior.
  - `rg 'class = "B"' control-plane/crates/db/write-operation-registry.toml` found no Class B registry rows.
- Gap / note: the coalescing component exists, but the implementation does not prove production projection/status writes actually use it.

### REQ-007: Telemetry Rollup

- Proposal source: Telemetry Metrics And Thresholds and Rollup Budget.
- Status: Partially Implemented.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:430` defines telemetry budget constants.
  - `control-plane/crates/db/src/write_class.rs:306` defines `DroppedTelemetry`.
  - `control-plane/crates/db/src/writer.rs:935` uses generic lane send/timeout behavior for non-Class-B writes.
  - `control-plane/crates/db/src/repos/storage_health.rs:152` reports `droppedTelemetryTotal: 0`.
  - `control-plane/crates/db/src/repos/storage_health.rs:433` reports per-lane `droppedTotal: 0`.
- Gap / note: the runtime drop/merge behavior and counters promised for Class D are not implemented beyond constants/readback placeholders.

### REQ-008: GraphQL Storage Health

- Proposal source: GraphQL Contract.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references:
  - `control-plane/crates/graphql-server/src/schema.rs:562` exposes `storage_health`.
  - `control-plane/crates/graphql-server/src/types/storage.rs`.
  - `control-plane/crates/graphql-server/src/schema.rs:3969` tests typed `storageHealth`.
  - `control-plane/crates/graphql-server/src/schema.rs:4030` tests live `DbWriter` heartbeat readback.

### REQ-009: MCP Storage Diagnostics

- Proposal source: MCP Contract.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:47` defines `storage.health`.
  - `control-plane/crates/mcp-server/src/tools/storage.rs:79` defines `storage.evidence_spool_summary`.
  - `control-plane/crates/mcp-server/src/tools/storage.rs:95` defines `storage.reconcile_evidence_orphans`.
  - `./scripts/test-gate.sh proposal-075` ran MCP storage diagnostics and typed error contract tests successfully.

### REQ-010: Migration and Recovery Documentation

- Proposal source: Data And Schema Changes, Rollout, Compatibility.
- Status: Implemented.
- Evidence types: migration, docs, tests-run.
- Evidence references:
  - `control-plane/crates/db/migrations/046_p075_evidence_spool_refs.sql`.
  - `control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql`.
  - `control-plane/crates/db/migrations/048_p075_evidence_path_constraints.sql`.
  - `control-plane/crates/daemon/src/storage_startup.rs`.
  - `docs/evidence/p075/phase1-baseline.md`.

### REQ-011: Fail-Closed Gate and Baseline Evidence

- Proposal source: Canonical Gate Commands and Gate Must Prove.
- Status: Partially Implemented.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `scripts/test-gate.sh:5515` starts P075 fail-closed registry checks.
  - `scripts/test-gate.sh:5637` scans runtime direct write sites.
  - `scripts/test-gate.sh:5672` enforces baseline evidence markers.
  - `docs/evidence/p075/phase1-baseline.md` contains numeric baseline values.
  - Gate passed on audited HEAD.
- Gap / note: the gate catches raw direct writes and generic duplicate proof paths, but it does not catch the shared-writer, class-adoption, or non-executable duplicate-proof gaps found in this audit.

### REQ-012: Diagnostics Export and Thin Apple Client

- Proposal source: UX And UI Notes / Diagnostics.
- Status: Implemented.
- Evidence types: code, tests-found.
- Evidence references:
  - `Chainworks Forge/Support/DiagnosticsBundle.swift`.
  - `Chainworks Forge/Support/DaemonLifecycleClient.swift`.
  - `Chainworks ForgeTests/DiagnosticsBundleTests.swift`.
  - `Chainworks ForgeTests/DaemonLifecycleClientTests.swift`.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Partial | Not Ready | Shared writer and B/D class semantics are incomplete | High |
| Rust architecture | Partial | Not Ready | Per-call/local writer instances fragment the bounded gateway | High |
| Rust reliability | Partial | Not Ready | Duplicate/replay evidence and telemetry/coalescing adoption are incomplete | High |
| API contract | Implemented | Ready with local risk | GraphQL/MCP schemas are present; pressure values can still omit local writer load | Medium |
| Observability/rollout | Partial | Not Ready | Gate passes while missing key implementation invariants | High |

## Routed Specialist Findings

### ARCH-001: Runtime writes use short-lived/local writers instead of one shared bounded writer

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-003, REQ-008, REQ-011
- Evidence types: code, tests-run
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:216` constructs `Arc::new(DbWriter::new(pool.clone()))` inside `begin_registered_immediate_transaction`.
  - `control-plane/crates/db/src/writer.rs:237` makes `execute_repository_write!` call `begin_repository_transaction`, which calls the per-transaction helper.
  - `control-plane/crates/db/src/repos/runs.rs:20` and `control-plane/crates/db/src/repos/agent_executions.rs:36` show pool-based repository APIs using the registered helper.
  - `control-plane/crates/engine/src/executor.rs:770`, `control-plane/crates/engine/src/cancellation.rs:60`, and `control-plane/crates/mcp-server/src/tools/reports.rs:195` create local writers at runtime.
  - The daemon creates one shared writer at `control-plane/crates/daemon/src/main.rs:261` and injects its heartbeat into GraphQL/MCP at `:351` and `:372`.
- Why it matters: P075's bounded lanes, queue depths, oldest queued age, priority ordering, starvation watchdog, and `storageHealth.writer` readbacks only work as a system invariant if runtime writes share the same writer boundary. A fresh writer per transaction gives each write its own empty queue and heartbeat, leaving SQLite's lock as the real serializer and hiding load from the daemon writer health.
- Recommended action: Make runtime write APIs accept/use the daemon `Arc<DbWriter>` or a transaction context derived from it. Remove `DbWriter::new(pool.clone())` from repository helpers and runtime paths except composition roots/tests. Add a gate check that rejects `DbWriter::new(pool.clone())` in non-test runtime modules outside explicit daemon construction.
- Acceptance criteria: All non-test runtime transactions enter the same live daemon writer; `storageHealth.writer.lanes[*].queuedDepth`, oldest age, rejection, starvation, and transaction duration reflect repository/producer writes under load.

### ARCH-002: Class C evidence metadata helpers re-enter a separate Class A transaction

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-003, REQ-004, REQ-011
- Evidence types: code
- Evidence references:
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs:904` builds a Class C `p075_evidence_spool_ref_insert` operation for `insert_via_dbwriter`.
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs:919` then opens `begin_registered_immediate_transaction` inside the submitted work and passes `class_a_operation(...)`.
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs:957` builds a Class C `p075_evidence_spool_ref_insert_idempotent` operation.
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs:974` calls `insert_idempotent`, which opens another registered Class A transaction at `:690`.
- Why it matters: Class C evidence metadata is supposed to enqueue on the `evidence_metadata` lane after file fsync. The current helper has an outer Class C lane but the actual SQLite transaction is opened by a new registered Class A writer, causing split accounting and defeating a clean class/lane-to-transaction mapping.
- Recommended action: Execute the SQL transaction directly inside the shared writer's Class C work item, or use a `DbWriter::submit_transaction`-style API that opens the transaction inside the same writer/lane without invoking `begin_registered_immediate_transaction`.
- Acceptance criteria: `p075_evidence_spool_ref_*` metadata operations open and commit their SQLite transactions exactly once inside the shared writer's `EvidenceMetadata` lane, with no nested local writer and no Class A reclassification.

### REL-001: Class B and Class D behavior is not adopted by production write paths

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-006, REQ-007, REQ-011
- Evidence types: code, config, tests-run
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:909` and `:1183` implement Class B coalescing support.
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:688` tests coalescing behavior.
  - `rg 'class = "B"' control-plane/crates/db/write-operation-registry.toml` found no Class B registry rows.
  - `rg 'WriteClass::B' control-plane/crates/{engine,daemon,graphql-server,mcp-server}/src control-plane/crates/db/src/repos` found only GraphQL enum mapping, not runtime producer use.
  - `control-plane/crates/db/write-operation-registry.toml:50` declares `p075_storage_write_pressure_snapshot_insert` as Class D, but `rg` found it only in the registry and tests.
  - `control-plane/crates/db/write-operation-registry.toml:848` registers the actual storage health snapshot write as Class A caller-guarded.
- Why it matters: P075 explicitly requires projection invalidations/coalesced state to merge/flush and telemetry to roll up without starving barriers. The component tests prove the writer can do this, but the audited runtime implementation does not prove any production path uses Class B and does not wire the Class D telemetry rollup operation.
- Recommended action: Register and route real projection/status invalidation writes as Class B with last-writer-wins keys. Route write pressure/telemetry rollup through Class D, not Class A repository transactions.
- Acceptance criteria: The registry contains production Class B operations used by projection/status write paths, and production telemetry writes submit Class D operations exercised by tests and read back through `storageHealth`.

### REL-002: Telemetry drop counters and `DroppedTelemetry` are placeholders

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-007, REQ-008
- Evidence types: code
- Evidence references:
  - `control-plane/crates/db/src/write_class.rs:306` defines `WriteResult::DroppedTelemetry`.
  - `control-plane/crates/db/src/writer.rs:935` sends all non-Class-B writes through the same bounded lane send/timeout path, returning `WriteRejected` on timeout at `:953`; no Class D drop-newest/drop-oldest path is implemented there.
  - `control-plane/crates/db/src/repos/storage_health.rs:152` hard-codes `droppedTelemetryTotal` to `0`.
  - `control-plane/crates/db/src/repos/storage_health.rs:428` to `:433` hard-code lane `droppedTotal` to `0`.
- Why it matters: P075 requires Class D telemetry to be droppable with counters and never block Class A or C. A result variant plus zero-valued readbacks do not provide overload semantics or operator evidence.
- Recommended action: Implement Class D drop policy and counters in `DbWriter`, expose live per-lane and total dropped telemetry counts, and add saturation tests that prove telemetry drops rather than blocking/rejecting important writes.
- Acceptance criteria: A saturated telemetry lane returns `DroppedTelemetry` or documented drop behavior with incremented counters, and `storageHealth.writer.droppedTelemetryTotal` / lane `droppedTotal` reflect the event.

### REL-003: Caller-guarded duplicate-application proof is documentation, not tests

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-011
- Evidence types: config, docs, tests-run
- Evidence references:
  - `control-plane/crates/db/write-operation-registry.toml:63` starts many `caller_guarded` operations pointing to `docs/evidence/p075/operation-duplicate-application-matrix.md#...`.
  - `docs/evidence/p075/operation-duplicate-application-matrix.md:5` defines a table of operations.
  - `docs/evidence/p075/operation-duplicate-application-matrix.md:7` and following rows use the generic proof text "Covered by the owning repository/engine transaction test path plus `./scripts/test-gate.sh proposal-075` registry and direct-write scan."
  - `scripts/test-gate.sh:5593` rejects missing/generic paths but only checks that matrix links are operation-specific strings.
- Why it matters: The proposal requires duplicate-application tests for caller-guarded operations. A matrix that restates "operation natural key" and references the gate does not prove that retrying the operation cannot double-apply the mutation.
- Recommended action: Replace matrix-only entries with executable operation-specific test paths, or augment the matrix gate to require a real test symbol/file that applies the operation twice and verifies no duplicate state transition, count, or side effect.
- Acceptance criteria: Every `caller_guarded` registry row points to an executable duplicate-application test, and the P075 gate fails if the referenced test path does not exist or is not run by the gate.

### OPS-001: P075 gate passes while missing core implementation invariants

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-002, REQ-003, REQ-006, REQ-007, REQ-011
- Evidence types: code, tests-run, inference
- Evidence references:
  - `scripts/test-gate.sh:5637` scans direct SQL write patterns and passed clean.
  - `scripts/test-gate.sh:5593` checks caller-guarded duplicate proof path shape.
  - The same gate passed despite ARCH-001, ARCH-002, REL-001, REL-002, and REL-003 evidence above.
- Why it matters: P075's gate is the closeout enforcement mechanism. If it green-lights local writer fragmentation, class/lane mismatch, documentation-only duplicate proofs, and absent B/D production adoption, it can no longer support a readiness verdict even when the test command passes.
- Recommended action: Extend the gate to verify shared-writer construction boundaries, registry-vs-code class consistency, real duplicate test existence/execution, production Class B adoption when projection writes exist, and live Class D drop/readback behavior.
- Acceptance criteria: Introducing a per-call `DbWriter::new`, a matrix-only duplicate proof, a Class C operation that opens a Class A nested transaction, or a missing production Class B/D path causes `./scripts/test-gate.sh proposal-075` to fail.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Canonical proposal gate on audited HEAD | Pass | `./scripts/test-gate.sh proposal-075` passed on `343c8690c4dd226ac18ba370bfa6d7c6c9506407`. |
| Direct runtime SQL scan | Pass | Gate reported runtime direct SQL scan clean. |
| Temporary rollout bypass retirement | Pass | Gate reported `0 temporary rollout bypasses`; allowlist has 5 permanent infrastructure entries. |
| Core writer component tests | Pass | DB writer, registry, allowlist, evidence spool refs, DB full regression, and P075 DB integration tests passed. |
| GraphQL/MCP diagnostics tests | Pass | Typed `storageHealth`, live heartbeat, MCP parameter/error tests passed. |
| Shared writer integration | Fail | Repository/runtime helpers still create local writer instances. |
| Operation duplicate tests | Fail | Caller-guarded entries mostly point to a documentation matrix, not executable duplicate-application tests. |
| Class B production adoption | Fail | No production Class B registry/runtime use found. |
| Class D telemetry/drop behavior | Fail | Class D operation unused; drop counters are placeholders. |
| UI runtime/screenshot validation | Not applicable | P075 is not a UI mutation proposal. |

## Verification Log

| Command / inspection | Result |
|---|---|
| `git status --short --branch` | Clean before report; branch `cw/implement-proposal-075-local-p/4aeb45a9`. |
| `git rev-parse HEAD` | `343c8690c4dd226ac18ba370bfa6d7c6c9506407`. |
| `git merge-base HEAD origin/main` | `70b03d1af641bbcb76a745449cc18f9a8fddea4c`. |
| `python3 .../discover_prior_review.py .../075-local-persistence-write-budget-and-evidence-spooling.md` | No prior proposal-review artifacts found. |
| `python3 .../report_path.py .../075-local-persistence-write-budget-and-evidence-spooling.md` | Selected this R7 report path. |
| `git diff --stat 70b03d1..HEAD` | 109 files changed, 22175 insertions, 1185 deletions. |
| `git diff --stat ed3c891a..HEAD` | 37 files changed, 1800 insertions, 661 deletions since R6 head. |
| `rg 'DbWriter::new\(pool\.clone\(\)\)' ...` | Found daemon composition root and multiple runtime/local helper constructors. |
| `rg 'class = "B"' control-plane/crates/db/write-operation-registry.toml` | No Class B registry rows found. |
| `rg 'WriteClass::B' control-plane/crates/{engine,daemon,graphql-server,mcp-server}/src control-plane/crates/db/src/repos` | Only GraphQL enum mapping found, no runtime producer use. |
| `rg 'p075_storage_write_pressure_snapshot_insert' control-plane/crates` | Found only registry/tests, not runtime use. |
| `./scripts/test-gate.sh proposal-075` | Passed. Final gate line: `P075 fail-closed registry check passed: 5 bypasses, 122 operations, 119 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`; then `Proposal 075 gate passed`. |

## Final Verdict

Overall conformance is Partial and implementation readiness is Not Ready.

The branch is materially closer than R6: the canonical P075 gate passes, the direct-write scan is stronger and clean, temporary rollout bypasses are retired, and evidence spooling/diagnostics have substantial passing coverage. The remaining blockers are implementation-boundary blockers, not missing test execution: runtime writes are still not consistently owned by one shared `DbWriter`; Class C metadata can re-enter a nested Class A transaction; Class B/D production behavior is not adopted; caller-guarded duplicate proof is mostly documentation; and the gate does not catch those invariants.

Recommended next actions:

1. Replace per-call/local writer construction with shared daemon writer ownership for all runtime repository/producer writes.
2. Remove nested `begin_registered_immediate_transaction` from evidence metadata `*_via_dbwriter` helpers.
3. Wire production Class B projection/status writes and Class D telemetry rollup/drop counters.
4. Replace caller-guarded matrix-only proof paths with executable duplicate-application tests.
5. Extend `./scripts/test-gate.sh proposal-075` so the above regressions fail closed.
