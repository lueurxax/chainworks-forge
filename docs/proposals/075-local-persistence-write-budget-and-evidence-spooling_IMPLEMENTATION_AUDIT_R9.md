# Proposal 075 Implementation Audit R9

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit date | 2026-05-09 |
| Audit mode | `auto` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree on `main` |
| Audited HEAD | `a43cfdf91435a0356f81483c4e336e3d4c55cdb8` (`Consolidate orchestration recovery and P075 closeout fixes`) |
| Compare base | Implicit current tree; `HEAD` equals `origin/main` merge-base `a43cfdf91435a0356f81483c4e336e3d4c55cdb8` |
| Worktree status before report | Clean: `## main...origin/main` |
| Proposal state | Approved proposal artifact; implementation closeout tracked in reference docs and audit evidence |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High |

## Implementation Target

The audit inspected the current `main` implementation at `a43cfdf91435a0356f81483c4e336e3d4c55cdb8`. The worktree was clean before this report was written. No implementation files, proposal files, reference docs, migrations, tests, or configs were modified as part of the audit.

## Prior Proposal-Review Reuse

Reviewer selection reuse: Not reused.

`discover_prior_review.py` returned no prior proposal-review artifacts for P075:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

Prior `IMPLEMENTATION_AUDIT` reports were intentionally ignored for reviewer selection per the audit skill. R8 findings were used only as historical implementation context.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P075 changes Rust control-plane persistence, repository boundaries, writer APIs, and crate-level ownership. |
| `rust_reliability_reviewer` | P075 is centered on deadlines, queueing, coalescing, shutdown, backpressure, idempotency, and overload behavior. |
| `api_contract_reviewer` | P075 exposes typed GraphQL and MCP storage diagnostics, storageHealth payloads, units, freshness, and error shapes. |
| `observability_rollout_reviewer` | P075 includes storageHealth, write pressure snapshots, diagnostics export, migration behavior, gates, and rollout evidence. |
| `chainworks_execution_truth_reviewer` | P075 protects durable Run/Stage/Agent/Approval/artifact/recovery truth from SQLite event-stream drift. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | No macOS UI surface was audited in this implementation slice. |
| `apple_arch_reviewer` | Current evidence is Rust control-plane/backend, not Swift app architecture. |
| `product_reviewer` | Product metrics and decision checkpoints are not the central acceptance risk in this implementation audit. |
| `rust_performance_reviewer` | Performance matters to P075, but the remaining blocker is missing rollup/retention behavior rather than measured hot-path performance. |
| `security_reviewer` | The audited delta did not introduce a new public auth, unsafe, secret, or privacy boundary beyond existing path validation and spool tests. |

## Contract Summary

Primary area: Rust control-plane local persistence.

Backend/service scope: service, worker, API, data, rollout, and observability.

P075 commits the implementation to:

- Route non-test runtime writes through bounded DbWriter lanes or a source-controlled temporary bypass allowlist.
- Implement WriteOperation metadata, write classes A/B/C/D, deadlines, replay policies, and typed WriteResult outcomes.
- Keep Class A canonical state serialized and prioritized ahead of low-value writes.
- Coalesce Class B projection/state writes by stable key, with mandatory flush behavior and merge/drop counters.
- Spool high-volume evidence bytes to files and persist compact EvidenceSpoolRef metadata in SQLite.
- Recover or report orphaned evidence files and surface manual reconciliation diagnostics.
- Roll up Class D telemetry with bounded memory/samples, periodic summary, TTL, and droppable counters.
- Expose typed GraphQL and MCP storage diagnostics with units, freshness, thresholds, degraded/unavailable cases, and evidence spool summaries.
- Include storageHealth, storage write pressure, and evidence spool summary in diagnostics/export surfaces.
- Gate direct-write bypasses, high-volume evidence-in-SQL regressions, Class B coalescing, and telemetry behavior through `scripts/test-gate.sh proposal-075`.

Primary service flows audited:

1. Repository mutation -> DbWriter classification -> lane scheduling -> SQLite transaction outcome.
2. Projection rebuild -> production Class B coalescing -> summary table materialization.
3. Evidence producer -> file spool fsync/checksum -> EvidenceSpoolRef pointer -> reader/reconcile diagnostics.
4. Storage diagnostics caller -> GraphQL/MCP storageHealth -> typed health, lane, pressure, spool, and error payloads.
5. Telemetry producer -> Class D/drop policy -> storageHealth counters and write-pressure rollup evidence.

## Fidelity Inventory

Matches:

- Production projection rebuilds now enter `execute_projection_write`, which creates a `WriteOperation`, overrides a stable projection idempotency key, and calls `execute_repository_transaction_operation` (`control-plane/crates/db/src/repos/projections.rs:13`).
- Repository transaction classification maps `projections.*` operations to Class B and storage/scheduler telemetry operations to Class D (`control-plane/crates/db/src/writer.rs:201`).
- DbWriter routes Class B through `submit_class_b`, and the coalescing path increments `coalesced_merged_total` when writes merge or stale writes are superseded (`control-plane/crates/db/src/writer.rs:1083`, `control-plane/crates/db/src/writer.rs:1402`).
- DbWriter drops Class D when the telemetry lane is full and increments `telemetry_dropped_total` (`control-plane/crates/db/src/writer.rs:1109`).
- storageHealth now exposes `droppedTelemetryTotal`, `coalescedMergedTotal`, live write-pressure counters, and telemetry rollup budget fields (`control-plane/crates/db/src/repos/storage_health.rs:160`, `control-plane/crates/db/src/repos/storage_health.rs:379`).
- The proposal gate now runs the production Class B coalescing test and the Class D drop-counter storageHealth test (`scripts/test-gate.sh:5489`).
- `./scripts/test-gate.sh proposal-075` passed on the audited tree.

Divergences:

- Class D telemetry has constants and surfaced counters, but the audit found no production rollup accumulator/producer that periodically writes bounded storage write-pressure snapshots, no enforcement of memory/sample caps beyond constants, and no TTL or latest-288 retention purge for `storage_write_pressure_snapshots`.
- The migration itself still says producers and retention-purge logic are wired later and the table remains empty until then (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:7`).
- The gate proves Class D drop counters, but not duplicate metric-window merging, bounded in-memory/local-spool aggregation, periodic summary rows, memory/sample cap enforcement, or TTL retention.

Ambiguities / Evidence Gaps:

- No live daemon run was performed; validation is code inspection plus canonical proposal gate evidence.
- The implementation may intentionally rely on live DbWriter heartbeat for some write-pressure diagnostics, but that does not satisfy the proposal's explicit persistent rollup/TTL contract.
- No benchmark or load profile was run for the write-pressure budget; the audit relies on tests and source inspection for behavioral conformance.

## Requirement Summary

| Requirement | Status | Evidence |
|---|---|---|
| REQ-001 Runtime write boundary and bypass allowlist | Implemented | Gate scan, registry, writer helper paths |
| REQ-002 WriteOperation metadata, classes, lanes, results, deadlines | Implemented | `write_class`, `writer`, registry, tests |
| REQ-003 Class A serialization, priority, deadlines, and shutdown admission | Implemented | Writer lane order, shutdown filter, tests |
| REQ-004 Deadline/accounting and WriteResult behavior | Implemented | Writer code, `proposal-075` gate |
| REQ-005 Idempotency and replay policies by class | Implemented | WriteOperation validation, class mapping, tests |
| REQ-006 Class B coalescing for production projections | Implemented | Production helper, coalescing code, integration test |
| REQ-007 Class D telemetry rollup budget, drop counters, TTL | Partially Implemented | Drop counters exist; rollup producer/cap/retention missing |
| REQ-008 Evidence spooling and metadata pointer discipline | Implemented | Spool repo, checksum/path tests, gate |
| REQ-009 Orphan recovery and manual reconcile diagnostics | Implemented | Startup and MCP tests, gate |
| REQ-010 GraphQL/MCP storage diagnostics and typed errors | Implemented | Schema/tests, MCP tests, gate |
| REQ-011 Diagnostics export and reference/readback integration | Implemented | Gate and source checks for diagnostics bundle/readbacks |
| REQ-012 Proposal gate proves required behavior | Partially Implemented | Gate passes but does not prove full Class D rollup/TTL |
| REQ-013 P078 foundation without SQLite event-stream drift | Partially Implemented | Barrier/evidence discipline exists; telemetry rollup discipline incomplete |

## Detailed Requirement Audit

### REQ-001 Runtime Write Boundary And Bypass Allowlist

Status: Implemented.

Proposal source: Acceptance criteria lines 715-716 and gate proof line 710.

Evidence types: code, config, tests-run.

Evidence references:

- `scripts/test-gate.sh` reported: `P075 fail-closed registry check passed: 5 bypasses, 122 operations, 113 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`.
- `repository_transaction_operation` classifies repository transaction writes by operation name (`control-plane/crates/db/src/writer.rs:201`).

Implementation mapping: Runtime repository transaction helpers route through the shared DbWriter where registered, and the canonical gate scans for direct runtime bypasses.

Gap / note: No gap found in this audit.

### REQ-002 WriteOperation Metadata, Classes, Lanes, Results, Deadlines

Status: Implemented.

Proposal source: Write classes lines 79-123, default deadlines lines 232-247, idempotency lines 249-255.

Evidence types: code, tests-run.

Evidence references:

- `repository_transaction_operation` maps Class B and Class D operation names into lanes, deadlines, batchability, and replay policy (`control-plane/crates/db/src/writer.rs:201`).
- `./scripts/test-gate.sh proposal-075` ran the db write-class and writer test suites successfully.

Implementation mapping: `WriteOperation` validation, class-specific defaults, lane capacities, and WriteResult variants are implemented and covered by the proposal gate.

Gap / note: Class D metadata exists, but full rollup behavior is tracked separately under REQ-007.

### REQ-003 Class A Serialization, Priority, Deadlines, And Shutdown Admission

Status: Implemented.

Proposal source: Class A behavior lines 79-91; shutdown protocol lines 257-266.

Evidence types: code, tests-run.

Evidence references:

- Lane drain order prioritizes `CriticalBarrier` first (`control-plane/crates/db/src/writer.rs:606`).
- Shutdown admission rejects non-Class A writes and fails closed to the admitted-operation list (`control-plane/crates/db/src/writer.rs:1063`).
- Proposal gate ran Class A priority and shutdown-admission tests.

Implementation mapping: DbWriter priority lanes and shutdown admission enforce the Class A barrier path.

Gap / note: No gap found in this audit.

### REQ-004 Deadline Accounting And WriteResult Behavior

Status: Implemented.

Proposal source: Deadlines and results lines 223-247.

Evidence types: code, tests-run.

Evidence references:

- Submission deadline accounting starts before admission/queue wait (`control-plane/crates/db/src/writer.rs:1096`).
- `submit_unit_transaction` preserves `Committed`, `Coalesced`, and `DroppedTelemetry` success semantics for the expected classes (`control-plane/crates/db/src/writer.rs:1309`).
- Proposal gate ran enqueue-to-commit deadline, timeout, busy retry, and WriteResult tests.

Implementation mapping: Queue admission, transaction work, and result mapping preserve the proposal's distinct outcomes.

Gap / note: No gap found in this audit.

### REQ-005 Idempotency And Replay Policies By Class

Status: Implemented.

Proposal source: Idempotency and replay lines 249-255.

Evidence types: code, tests-run.

Evidence references:

- Production projection helper overrides the default idempotency key with per-run/projection keys (`control-plane/crates/db/src/repos/projections.rs:13`).
- `storage_health.insert_write_pressure_snapshot` uses a window-based idempotency key (`control-plane/crates/db/src/repos/storage_health.rs:43`).
- `scheduler.record_db_writer_wait_observation` uses an operation/time idempotency key (`control-plane/crates/db/src/repos/scheduler.rs:398`).

Implementation mapping: The explicit Class B and Class D repository paths now populate stable operation-level keys instead of relying only on the operation name.

Gap / note: Duplicate Class D metric-window merge behavior is not proven because the rollup producer itself is incomplete; this is captured in REQ-007.

### REQ-006 Class B Coalescing For Production Projections

Status: Implemented.

Proposal source: Class B behavior lines 92-104; coalescing lines 275-293; acceptance line 721.

Evidence types: code, tests-found, tests-run.

Evidence references:

- `execute_projection_write` builds the transaction operation and calls `execute_repository_transaction_operation` (`control-plane/crates/db/src/repos/projections.rs:13`).
- `rebuild_run_summary` and `rebuild_stage_summaries` use stable run/projection idempotency keys (`control-plane/crates/db/src/repos/projections.rs:337`, `control-plane/crates/db/src/repos/projections.rs:445`).
- DbWriter routes Class B into `submit_class_b` (`control-plane/crates/db/src/writer.rs:1083`).
- The production integration test spawns eight concurrent `projections::rebuild_run_summary` calls and asserts a merged coalescing heartbeat (`control-plane/crates/db/tests/integration.rs:2083`).
- `./scripts/test-gate.sh proposal-075` ran and passed that integration test.

Implementation mapping: The previous R8 blocker is closed: production projection writes now exercise the Class B transaction coalescing path instead of bypassing it through a direct repository-write helper.

Gap / note: No current blocker found for production Class B projection coalescing.

### REQ-007 Class D Telemetry Rollup Budget, Drop Counters, TTL

Status: Partially Implemented.

Proposal source: Class D behavior lines 119-123; Class D replay line 255; rollup budget lines 597-603; gate proof line 709; acceptance line 722.

Evidence types: code, migration, tests-found, tests-run.

Evidence references:

- Class D classification exists for storage write-pressure snapshots and scheduler wait observations (`control-plane/crates/db/src/writer.rs:217`).
- Constants for memory cap, max samples, flush cadence, and TTL exist (`control-plane/crates/db/src/writer.rs:590`).
- DbWriter increments `telemetry_dropped_total` when the telemetry lane is full (`control-plane/crates/db/src/writer.rs:1109`).
- storageHealth exposes drop counters and budget constants (`control-plane/crates/db/src/repos/storage_health.rs:160`, `control-plane/crates/db/src/repos/storage_health.rs:185`).
- The drop-counter test fills the telemetry lane, gets `DroppedTelemetry`, and verifies storageHealth readbacks (`control-plane/crates/db/tests/proposal_075_dbwriter.rs:154`).
- The migration says storage write-pressure snapshot producers and retention-purge logic are later work and the table is empty until then (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:7`).
- Source search found no production caller for `insert_write_pressure_snapshot` outside its definition; the only source call in `mcp-server/src/tools/storage.rs` is inside `#[cfg(test)]` (`control-plane/crates/mcp-server/src/tools/storage.rs:351`).
- Source search found no retention purge for `storage_write_pressure_snapshots`; only constants and storageHealth surfacing reference `TELEMETRY_SNAPSHOT_TTL_HOURS`.

Implementation mapping: Drop behavior and counters are now real, and a storage write-pressure table/helper exists. The rollup accumulator/producer/retention behavior promised by P075 is not implemented or not wired into production.

Gap / note: The proposal requires telemetry to be aggregated in memory or local spool, periodically summarized, bounded by memory/sample caps, merged by time window, retained by TTL, and unable to starve Class A. The current implementation proves drop counters and Class A priority, but not the rollup/TTL half of that contract.

### REQ-008 Evidence Spooling And Metadata Pointer Discipline

Status: Implemented.

Proposal source: Class C behavior lines 105-118; evidence spooling and acceptance lines 718-719.

Evidence types: code, tests-run.

Evidence references:

- Proposal gate ran evidence spool repository tests, spool file checksum/fsync ordering tests, high-volume fake stream tests, and engine failed-stage evidence packet tests.
- Gate output confirmed `evidence_spool_refs` test coverage and engine producer adoption passed.

Implementation mapping: High-volume evidence is stored as files with compact metadata pointers, with path, checksum, ownership, and summary bounds covered by tests.

Gap / note: No gap found in this audit.

### REQ-009 Orphan Recovery And Manual Reconcile Diagnostics

Status: Implemented.

Proposal source: hard crash recovery lines 268-274; tests lines 689-690; acceptance line 720.

Evidence types: code, tests-run.

Evidence references:

- `./scripts/test-gate.sh proposal-075` passed daemon startup orphan sweep wiring tests.
- The proposal gate also ran MCP typed error tests for reconcile/maintenance-disabled/stale/unavailable paths.

Implementation mapping: Startup recovery and manual reconcile diagnostics are present and tested through the canonical proposal gate.

Gap / note: No gap found in this audit.

### REQ-010 GraphQL/MCP Storage Diagnostics And Typed Errors

Status: Implemented.

Proposal source: diagnostics lines 670-675; tests lines 693-694; acceptance line 723.

Evidence types: code, schema, tests-run.

Evidence references:

- storageHealth JSON includes writer, WAL, projections, evidence spool, writePressure, telemetryRollup, killSwitches, and thresholds (`control-plane/crates/db/src/repos/storage_health.rs:160`).
- GraphQL typed storageHealth contract tests passed in the proposal gate.
- MCP storage typed diagnostics and typed error contract tests passed in the proposal gate.

Implementation mapping: GraphQL and MCP surfaces expose the typed storage diagnostics contract, with units/freshness and degraded error paths covered by tests.

Gap / note: The storageHealth surface exposes telemetry rollup fields, but the underlying persistent Class D rollup producer/retention remains incomplete under REQ-007.

### REQ-011 Diagnostics Export And Reference/Readback Integration

Status: Implemented.

Proposal source: diagnostics export line 675; tests line 696.

Evidence types: code, tests-run.

Evidence references:

- `./scripts/test-gate.sh proposal-075` passed diagnostics/export-related checks and the typed storage diagnostic tests.

Implementation mapping: Diagnostics bundle/readback integration for storageHealth and evidence spool summaries is present enough for the canonical gate.

Gap / note: Storage write-pressure snapshot history is only as complete as REQ-007's rollup producer.

### REQ-012 Proposal Gate Proves Required Behavior

Status: Partially Implemented.

Proposal source: canonical gate lines 698-711; acceptance line 726.

Evidence types: tests-run, code.

Evidence references:

- The canonical proposal gate passed on the audited tree.
- The gate now explicitly runs the production Class B coalescing integration test and Class D drop-counter test (`scripts/test-gate.sh:5489`).
- The gate includes static checks for production projection transaction helpers, Class D helper use, and drop-counter readbacks (`scripts/test-gate.sh:5699`).
- The gate does not assert a production Class D rollup producer, memory/sample cap enforcement, duplicate bucket merge, periodic snapshot creation, snapshot TTL purge, or latest-288 retention behavior.

Implementation mapping: The gate now covers the prior R8 Class B and Class D drop-counter holes, but not the full telemetry rollup behavior P075 says the gate must prove.

Gap / note: A passing gate is necessary evidence here but not sufficient to declare P075 ready.

### REQ-013 P078 Foundation Without SQLite Event-Stream Drift

Status: Partially Implemented.

Proposal source: write class examples lines 87-91; acceptance line 725.

Evidence types: code, tests-run.

Evidence references:

- Barrier write classification and evidence spool discipline are implemented and gated.
- The Class D rollup discipline remains incomplete, leaving part of the future durable-layer pressure-control foundation unfinished.

Implementation mapping: P078 can rely on much of the barrier/evidence foundation, but telemetry rollup remains an incomplete part of the persistence pressure contract.

Gap / note: This requirement should be upgraded only after REQ-007 is closed.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Class D rollup/TTL incomplete | High |
| Rust architecture | Pass with caveat | Good transaction helper routing; Class D producer missing | High |
| Rust reliability | Partial | Drop counters exist, but rollup retention and cap behavior are absent | High |
| API contract | Pass with caveat | Typed surfaces exist; writePressure history can be empty without a producer | Medium |
| Observability / rollout | Not Ready | Gate and diagnostics do not prove full telemetry rollup contract | High |
| Execution truth | Pass with caveat | Canonical/evidence paths protected; telemetry pressure foundation incomplete | Medium |
| Readiness | Not Ready | Major P075 acceptance gap remains | High |

## Routed Specialist Findings

### OPS-001: Class D telemetry rollup is surfaced but not implemented as a bounded producer with retention

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-007, REQ-012, REQ-013

Evidence types: proposal, code, migration, tests-found, tests-run.

Evidence references:

- P075 defines Class D as aggregated in memory/local spool and periodically summarized, with bounded rollup rows or health snapshot fields (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:119`).
- P075 sets memory cap, max samples, flush cadence, snapshot TTL, and shutdown semantics (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:597`).
- P075 acceptance requires telemetry to be rolled up with memory cap and TTL (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:721`).
- The implementation defines budget constants and storageHealth fields (`control-plane/crates/db/src/writer.rs:590`, `control-plane/crates/db/src/repos/storage_health.rs:185`).
- The implementation has a helper to insert a write-pressure snapshot (`control-plane/crates/db/src/repos/storage_health.rs:43`), but source search found no production producer calling it; the visible `mcp-server` call is test-only (`control-plane/crates/mcp-server/src/tools/storage.rs:351`).
- The migration states producers and retention-purge logic are wired later and the table is empty until then (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:7`).
- Source search found no retention purge for `storage_write_pressure_snapshots`; only constants and surfaced fields reference the TTL.

Why it matters:

P075's pressure-control model relies on Class D being more than a drop counter. Without a bounded accumulator/producer and retention purge, the system cannot prove telemetry is rolled up, cannot enforce the stated memory/sample budget, cannot preserve bounded write-pressure history for diagnostics/export, and cannot satisfy the acceptance criterion that telemetry has TTL-backed rollup discipline.

Recommended action:

Implement a production Class D telemetry rollup path that aggregates by metric bucket/time window, enforces `TELEMETRY_MEMORY_CAP_BYTES` and `TELEMETRY_MAX_SAMPLES`, flushes on `TELEMETRY_FLUSH_CADENCE_MS`, writes bounded storage write-pressure snapshots through DbWriter, and purges snapshots older than `TELEMETRY_SNAPSHOT_TTL_HOURS` or outside the latest-288 window.

Acceptance criteria:

- A production daemon/scheduler/storage component periodically calls the Class D snapshot writer from non-test code.
- Tests prove additive counter/max-gauge merge semantics for duplicate time windows.
- Tests prove memory cap and max-sample enforcement.
- Tests prove TTL/latest-288 retention.
- storageHealth, MCP, and diagnostics export read back produced snapshots, not just live heartbeat fields.

### READY-001: The P075 gate passes without proving the full Class D rollup contract

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-007, REQ-012

Evidence types: proposal, code, tests-run.

Evidence references:

- P075 says the gate must prove telemetry is rolled up and cannot starve Class A (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:703`).
- The current gate passed and now covers production Class B coalescing plus Class D drop counters (`scripts/test-gate.sh:5489`).
- The static gate checks require helper usage and drop-counter strings, but do not check a rollup producer, memory/sample caps, periodic snapshots, TTL, or retention (`scripts/test-gate.sh:5699`).

Why it matters:

A green `proposal-075` gate can currently coexist with the missing rollup producer/retention behavior in OPS-001. That makes the gate too weak to serve as the final readiness contract for P075 closeout.

Recommended action:

Extend the gate with an integration test for the full Class D lifecycle: submit telemetry through the production producer, force a flush, assert bounded snapshot rows and merged payload fields, exceed cap/sample thresholds, trigger retention, and confirm Class A work is not starved under Class D pressure.

Acceptance criteria:

- `./scripts/test-gate.sh proposal-075` fails if production Class D telemetry is only a drop counter.
- The gate proves memory cap, max samples, flush cadence, TTL/latest-288 retention, and duplicate-window merge behavior.
- The gate keeps the existing production Class B coalescing and direct-write bypass checks.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Same-tree canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-075` passed on `a43cfdf91435a0356f81483c4e336e3d4c55cdb8` |
| Production Class B coalescing regression | Passed | `p075_projection_rebuild_uses_production_class_b_coalescing` passed |
| Class D drop-counter regression | Passed | `class_d_telemetry_drop_counter_is_observable_via_storage_health` passed |
| Direct runtime DB bypass gate | Passed | Gate reported runtime direct SQL scan clean |
| Evidence spool file/pointer discipline | Passed | Proposal gate evidence spool tests passed |
| GraphQL/MCP typed diagnostics | Passed | Proposal gate storageHealth and MCP diagnostics tests passed |
| Class D bounded rollup producer | Failed | No production producer found |
| Class D memory/sample cap enforcement | Failed | Constants exist; enforcement not found |
| Class D TTL/latest-288 retention | Failed | No purge found; migration says future wiring |
| Live runtime daemon validation | Not run | Not required for this read-only audit; no runtime claim made |

## Verification Log

Commands and inspections performed:

- `git status --short --branch` -> `## main...origin/main`
- `git log -1 --oneline --decorate` -> `a43cfdf9 (HEAD -> main, origin/main, origin/HEAD) Consolidate orchestration recovery and P075 closeout fixes`
- `git merge-base HEAD origin/main` -> `a43cfdf91435a0356f81483c4e336e3d4c55cdb8`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` -> this R9 report path
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` -> no artifacts
- `rg` searches for `insert_write_pressure_snapshot`, `StorageWritePressureSnapshot::new`, `record_db_writer_wait_observation`, `TELEMETRY_*`, `storage_write_pressure_snapshots`, and retention/deletion logic
- Focused reads of `writer.rs`, `projections.rs`, `storage_health.rs`, `scheduler.rs`, `storage.rs`, `047_p075_storage_write_pressure_snapshots.sql`, P075 tests, and `scripts/test-gate.sh`
- `./scripts/test-gate.sh proposal-075` -> passed

Notable gate evidence:

- `P075: production Class B projections use coalescing and Class D telemetry exposes drop counters`
- `p075_projection_rebuild_uses_production_class_b_coalescing ... ok`
- `class_d_telemetry_drop_counter_is_observable_via_storage_health ... ok`
- `P075 fail-closed registry check passed: 5 bypasses, 122 operations, 113 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`
- `Proposal 075 gate passed`

## Final Verdict

P075 is substantially improved from R8. The prior production Class B projection coalescing blocker is closed, and Class D drop counters are now real and visible through storageHealth. The canonical proposal gate passes on the audited tree.

The implementation is still not ready for closeout because P075's explicit Class D rollup contract is only partially implemented. The current tree exposes constants, live heartbeat counters, and a snapshot insert helper, but lacks a production bounded rollup producer, memory/sample cap enforcement, periodic persisted snapshot generation, and TTL/latest-288 retention. The gate also does not fail on that gap.

Recommended next actions:

1. Implement the production Class D rollup producer/accumulator and retention purge.
2. Extend `proposal-075` gate coverage to prove rollup merge, cap, cadence, TTL, and non-starvation behavior.
3. Re-run `./scripts/test-gate.sh proposal-075` on the same tree and perform a follow-up audit after the Class D lifecycle is covered end to end.
