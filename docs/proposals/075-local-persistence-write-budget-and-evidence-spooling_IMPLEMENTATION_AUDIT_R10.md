# Proposal 075 Implementation Audit R10

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit date | 2026-05-09 |
| Audit mode | `auto` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree on `main` |
| Audited HEAD | `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab` (`Register DbWriter before steward bootstrap`) |
| Compare base | Implicit current tree; `HEAD` equals `origin/main` merge-base `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab` |
| Worktree status before report | Clean: `## main...origin/main` |
| Proposal state | Approved proposal artifact; implementation closeout tracked in reference docs and audit evidence |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High |

## Implementation Target

This audit inspected the current `main` implementation at `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab`. The worktree was clean before the report was written. The only file written by this audit is this versioned report.

The latest implementation moved beyond R9: it registers the shared DbWriter before steward bootstrap, starts a daemon Class D storage write-pressure rollup task, persists rollup snapshots, exposes latest-window retention metadata, and extends the P075 gate.

## Prior Proposal-Review Reuse

Reviewer selection reuse: Not reused.

`discover_prior_review.py` returned no proposal-review artifacts for P075:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

Prior `IMPLEMENTATION_AUDIT` reports were not reused for reviewer selection. R9 was used only as historical implementation context for checking whether the prior Class D blocker was fixed.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Rust control-plane persistence, repository boundaries, writer APIs, and daemon startup wiring are in scope. |
| `rust_reliability_reviewer` | P075 centers on queues, deadlines, shutdown, coalescing, telemetry drop behavior, replay, and recovery. |
| `api_contract_reviewer` | P075 exposes GraphQL and MCP storage diagnostics, typed units, freshness, and error shapes. |
| `observability_rollout_reviewer` | P075 includes storageHealth, write-pressure snapshots, diagnostics export, migration behavior, gates, and retention. |
| `chainworks_execution_truth_reviewer` | P075 protects durable Run/Stage/Agent/Approval/artifact truth from SQLite event-stream drift. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | No macOS UI implementation surface was audited. |
| `apple_arch_reviewer` | Current implementation evidence is Rust control-plane/backend, not Swift app state. |
| `product_reviewer` | Product metrics and decision checkpoints are not the central remaining risk. |
| `rust_performance_reviewer` | Performance is relevant, but the remaining issue is a semantic rollup/replay contract, not a measured hot-path regression. |
| `security_reviewer` | No new auth, public input, unsafe, secret, or privacy boundary was introduced by the audited delta. |

## Contract Summary

Primary area: Rust control-plane local persistence.

Backend/service scope: service, worker, API, data, rollout, and observability.

P075 commits the implementation to:

- Route non-test runtime writes through DbWriter or a source-controlled bypass allowlist.
- Implement WriteOperation metadata, classes A/B/C/D, deadlines, replay policies, and typed WriteResult outcomes.
- Keep Class A canonical state serialized, prioritized, and deadline-bound.
- Coalesce Class B projection/state writes by stable key, with mandatory flushes and merge/drop counters.
- Spool high-volume evidence bytes to files and keep compact EvidenceSpoolRef metadata in SQLite.
- Recover/report orphaned evidence files and expose manual reconcile diagnostics.
- Roll up Class D telemetry with bounded memory/samples, periodic summaries, TTL/latest-window retention, droppable counters, and duplicate-window merge semantics.
- Expose typed GraphQL and MCP storage diagnostics with units, freshness, thresholds, degraded/unavailable cases, and evidence spool summaries.
- Gate direct-write bypasses, high-volume evidence-in-SQL regressions, Class B coalescing, Class D rollup/drop behavior, and diagnostics parity through `scripts/test-gate.sh proposal-075`.

Primary service flows audited:

1. Repository mutation -> DbWriter classification -> lane scheduling -> SQLite transaction result.
2. Projection rebuild -> production Class B coalescing -> summary materialization.
3. Evidence producer -> file spool fsync/checksum -> EvidenceSpoolRef pointer -> reader/reconcile diagnostics.
4. Daemon heartbeat -> Class D write-pressure rollup -> snapshot persistence/retention -> storageHealth readback.
5. Storage diagnostics caller -> GraphQL/MCP storageHealth -> typed health, pressure, spool, and error payloads.

## Fidelity Inventory

Matches:

- Production projection rebuilds enter `execute_projection_write`, which sets a projection idempotency key and calls the transaction DbWriter helper (`control-plane/crates/db/src/repos/projections.rs:13`).
- `repository_transaction_operation` maps `projections.*` to Class B and storage/scheduler telemetry operations to Class D (`control-plane/crates/db/src/writer.rs:201`).
- DbWriter routes Class B through `submit_class_b`, increments `coalesced_merged_total`, and drops Class D with `telemetry_dropped_total` when the telemetry lane is full (`control-plane/crates/db/src/writer.rs:1083`, `control-plane/crates/db/src/writer.rs:1407`, `control-plane/crates/db/src/writer.rs:1116`).
- The daemon registers the shared DbWriter before steward bootstrap and starts the storage write-pressure rollup task after wiring the writer (`control-plane/crates/daemon/src/main.rs:229`, `control-plane/crates/daemon/src/main.rs:265`).
- `spawn_storage_write_pressure_rollup` runs every `TELEMETRY_FLUSH_CADENCE_MS` and calls `record_live_write_pressure_rollup` (`control-plane/crates/daemon/src/main.rs:565`).
- `insert_write_pressure_snapshot` persists a Class D snapshot and deletes rows older than the TTL or outside the latest-window cap (`control-plane/crates/db/src/repos/storage_health.rs:43`).
- `record_live_write_pressure_rollup` builds a rollup payload from the live DbWriter heartbeat and persists it through the Class D helper (`control-plane/crates/db/src/repos/storage_health.rs:114`).
- storageHealth exposes telemetry rollup limits, latest-window limit, drop counters, and write-pressure payloads (`control-plane/crates/db/src/repos/storage_health.rs:191`, `control-plane/crates/db/src/repos/storage_health.rs:453`).
- `./scripts/test-gate.sh proposal-075` passed on the audited HEAD, including the production Class B test, Class D drop-counter test, and the new Class D rollup producer/retention test.

Divergences:

- P075 requires Class D duplicate rollups to merge by additive counters or max gauges for a metric bucket/time window. The current storage write-pressure table has no unique key for `(metric bucket, window)` or even `(window_start, window_end)`, and `insert_write_pressure_snapshot` uses plain `INSERT` rather than an upsert/merge path (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:10`, `control-plane/crates/db/src/repos/storage_health.rs:71`).
- The registry says the storage snapshot operation uses replay policy `telemetry_merge`, but the implementation does not demonstrate duplicate-window merge behavior (`control-plane/crates/db/write-operation-registry.toml:848`).
- The current rollup window is created as `now - 1ms` to `now` on each flush, so it records a point-in-time heartbeat summary rather than an accumulated flush-period bucket (`control-plane/crates/db/src/repos/storage_health.rs:118`).

Ambiguities / Evidence Gaps:

- No live daemon runtime was started; the audit used source inspection and the canonical proposal gate.
- The canonical gate now proves producer wiring and retention, but not duplicate-window merge semantics or additive/max-gauge replay behavior.

## Requirement Summary

| Requirement | Status | Evidence |
|---|---|---|
| REQ-001 Runtime write boundary and bypass allowlist | Implemented | Gate scan, registry, writer helper paths |
| REQ-002 WriteOperation metadata, classes, lanes, results, deadlines | Implemented | `write_class`, `writer`, registry, gate |
| REQ-003 Class A serialization, priority, deadlines, shutdown admission | Implemented | Writer lane order, shutdown filter, gate |
| REQ-004 Deadline/accounting and WriteResult behavior | Implemented | Writer code and gate |
| REQ-005 Idempotency and replay policies by class | Partially Implemented | Class D declares telemetry merge but does not merge duplicate windows |
| REQ-006 Class B coalescing for production projections | Implemented | Production helper and integration test |
| REQ-007 Class D telemetry rollup budget, drop counters, TTL | Partially Implemented | Producer/retention exist; duplicate-window merge/window semantics incomplete |
| REQ-008 Evidence spooling and metadata pointer discipline | Implemented | Spool repo, checksum/path tests, gate |
| REQ-009 Orphan recovery and manual reconcile diagnostics | Implemented | Startup/MCP tests, gate |
| REQ-010 GraphQL/MCP storage diagnostics and typed errors | Implemented | Schema/MCP tests, gate |
| REQ-011 Diagnostics export and readback integration | Implemented | Gate/source checks |
| REQ-012 Proposal gate proves required behavior | Partially Implemented | Gate passes but does not prove Class D duplicate-window merge semantics |
| REQ-013 P078 foundation without SQLite event-stream drift | Implemented with caveat | Barrier/evidence/rollup pressure controls exist; Class D replay semantics remain caveat |

## Detailed Requirement Audit

### REQ-001 Runtime Write Boundary And Bypass Allowlist

Status: Implemented.

Proposal source: Acceptance criteria lines 715-716 and gate proof line 710.

Evidence types: code, config, tests-run.

Evidence references:

- Gate output: `P075 fail-closed registry check passed: 5 bypasses, 122 operations, 113 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`.
- Repository transaction classification and helper routing remain in `writer.rs` (`control-plane/crates/db/src/writer.rs:201`).

Implementation mapping: Runtime repository transaction flows route through DbWriter or checked allowlist paths.

Gap / note: No gap found.

### REQ-002 WriteOperation Metadata, Classes, Lanes, Results, Deadlines

Status: Implemented.

Proposal source: write classes lines 79-123, deadlines/results lines 223-247, idempotency lines 249-255.

Evidence types: code, tests-run.

Evidence references:

- Class B and Class D repository operation mapping (`control-plane/crates/db/src/writer.rs:201`).
- `./scripts/test-gate.sh proposal-075` passed write-class, writer, registry, and db full regression suites.

Implementation mapping: Write classes, lanes, deadlines, replay policy values, and result variants are present and tested.

Gap / note: Class D replay behavior is separated into REQ-005/REQ-007.

### REQ-003 Class A Serialization, Priority, Deadlines, Shutdown Admission

Status: Implemented.

Proposal source: Class A behavior lines 79-91 and shutdown protocol lines 257-266.

Evidence types: code, tests-run.

Evidence references:

- Lane drain order starts with `CriticalBarrier` (`control-plane/crates/db/src/writer.rs:606`).
- Shutdown admission rejects non-Class A writes and fail-closes to the admitted operation list (`control-plane/crates/db/src/writer.rs:1069`).
- Gate passed Class A priority and shutdown tests.

Implementation mapping: Class A barrier writes are serialized, prioritized, and protected during shutdown.

Gap / note: No gap found.

### REQ-004 Deadline Accounting And WriteResult Behavior

Status: Implemented.

Proposal source: deadlines and results lines 223-247.

Evidence types: code, tests-run.

Evidence references:

- Submission deadline accounting starts before admission/queue wait and result wait uses remaining deadline (`control-plane/crates/db/src/writer.rs:1096`).
- Gate passed timeout, busy retry, and WriteResult tests.

Implementation mapping: The writer preserves distinct committed, coalesced, dropped, rejected, timeout, busy-exhausted, and failed outcomes.

Gap / note: No gap found.

### REQ-005 Idempotency And Replay Policies By Class

Status: Partially Implemented.

Proposal source: idempotency and replay lines 249-255.

Evidence types: code, migration, config, tests-run.

Evidence references:

- Production projection helper uses stable projection idempotency keys (`control-plane/crates/db/src/repos/projections.rs:13`).
- `insert_write_pressure_snapshot` sets a Class D idempotency key from window start/end (`control-plane/crates/db/src/repos/storage_health.rs:52`).
- Registry declares `storage_health.insert_write_pressure_snapshot` as Class D with `telemetry_merge` replay policy (`control-plane/crates/db/write-operation-registry.toml:848`).
- The storage write-pressure table has a primary key on generated `id`, not a unique metric/window key (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:10`).
- Snapshot insert is plain `INSERT`, with no `ON CONFLICT` or update/merge path (`control-plane/crates/db/src/repos/storage_health.rs:71`).

Implementation mapping: Class A/B/C metadata and replay policies are implemented. Class D declares telemetry merge but does not implement duplicate bucket/window merge semantics.

Gap / note: Additive counter/max-gauge merge for duplicate Class D windows remains unimplemented or unproven.

### REQ-006 Class B Coalescing For Production Projections

Status: Implemented.

Proposal source: Class B behavior lines 92-104; coalescing lines 275-293; acceptance line 721.

Evidence types: code, tests-found, tests-run.

Evidence references:

- Production projection writes use `execute_projection_write` (`control-plane/crates/db/src/repos/projections.rs:13`).
- DbWriter routes Class B to `submit_class_b` (`control-plane/crates/db/src/writer.rs:1083`).
- `p075_projection_rebuild_uses_production_class_b_coalescing` passed in the proposal gate.

Implementation mapping: Production projection rebuilds now exercise Class B coalescing and the gate proves it.

Gap / note: No gap found.

### REQ-007 Class D Telemetry Rollup Budget, Drop Counters, TTL

Status: Partially Implemented.

Proposal source: Class D behavior lines 119-123; Class D replay line 255; rollup budget lines 597-603; acceptance line 722.

Evidence types: code, migration, tests-found, tests-run.

Evidence references:

- Class D constants include memory cap, max samples, flush cadence, TTL, and latest-window retention (`control-plane/crates/db/src/writer.rs:590`).
- The daemon spawns the production rollup producer using the DbWriter heartbeat (`control-plane/crates/daemon/src/main.rs:565`).
- `record_live_write_pressure_rollup` builds and persists a live heartbeat rollup (`control-plane/crates/db/src/repos/storage_health.rs:114`).
- `insert_write_pressure_snapshot` enforces payload size and purges TTL/latest-window rows in the same transaction (`control-plane/crates/db/src/repos/storage_health.rs:43`).
- `class_d_rollup_producer_persists_bounded_snapshot_and_purges_retention` passed in the proposal gate.
- No duplicate-window upsert/merge behavior exists in schema or insert code (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:10`, `control-plane/crates/db/src/repos/storage_health.rs:71`).

Implementation mapping: The R9 blocker is substantially fixed: a production rollup producer, retention purge, drop counters, and gate coverage exist. The remaining gap is Class D duplicate-window merge semantics and true bucket replay behavior.

Gap / note: The proposal explicitly requires duplicate rollups to merge by additive counters or max gauges. Current implementation inserts point-in-time snapshots by generated UUID.

### REQ-008 Evidence Spooling And Metadata Pointer Discipline

Status: Implemented.

Proposal source: Class C behavior lines 105-118; evidence spooling and acceptance lines 718-719.

Evidence types: code, tests-run.

Evidence references:

- Gate passed evidence spool repository tests, checksum/fsync ordering tests, high-volume fake stream tests, and engine failed-stage evidence packet tests.

Implementation mapping: High-volume evidence is spooled to files with compact metadata pointers and path/checksum/status validation.

Gap / note: No gap found.

### REQ-009 Orphan Recovery And Manual Reconcile Diagnostics

Status: Implemented.

Proposal source: hard crash recovery lines 268-274; tests lines 689-690; acceptance line 720.

Evidence types: code, tests-run.

Evidence references:

- Gate passed daemon startup orphan sweep wiring and MCP reconcile typed error tests.

Implementation mapping: Startup recovery and manual reconcile diagnostics are present and gated.

Gap / note: No gap found.

### REQ-010 GraphQL/MCP Storage Diagnostics And Typed Errors

Status: Implemented.

Proposal source: diagnostics lines 670-675; tests lines 693-694; acceptance line 723.

Evidence types: code, schema, tests-run.

Evidence references:

- storageHealth includes writer, WAL, projections, evidence spool, writePressure, telemetryRollup, killSwitches, and thresholds (`control-plane/crates/db/src/repos/storage_health.rs:191`).
- Gate passed GraphQL typed storageHealth contract tests and MCP storage diagnostics/typed error tests.

Implementation mapping: GraphQL and MCP expose typed storage diagnostics with units, freshness, thresholds, and degraded error paths.

Gap / note: No gap found.

### REQ-011 Diagnostics Export And Readback Integration

Status: Implemented.

Proposal source: diagnostics export line 675; tests line 696.

Evidence types: code, tests-run.

Evidence references:

- Gate passed diagnostics/export-related checks and storage diagnostic contract tests.

Implementation mapping: storageHealth and evidence spool summaries are wired into the diagnostic/readback surface covered by the proposal gate.

Gap / note: No gap found.

### REQ-012 Proposal Gate Proves Required Behavior

Status: Partially Implemented.

Proposal source: canonical gate lines 698-711; acceptance line 726.

Evidence types: tests-run, code.

Evidence references:

- `./scripts/test-gate.sh proposal-075` passed on audited HEAD.
- The gate now runs production Class B coalescing, Class D drop-counter, and Class D rollup producer/retention tests.
- Static gate checks require rollup lifecycle strings, daemon producer spawn, retention deletion, and direct-write scan checks (`scripts/test-gate.sh:5713`).
- The gate does not prove duplicate Class D metric bucket/window merge behavior or additive counter/max-gauge replay semantics.

Implementation mapping: Gate coverage is much stronger than R9 and covers the former blocker, but still misses one explicit P075 replay requirement.

Gap / note: Add a duplicate-window merge test and static/schema checks for unique bucket/window keys or an equivalent merge mechanism.

### REQ-013 P078 Foundation Without SQLite Event-Stream Drift

Status: Implemented with caveat.

Proposal source: write class examples lines 87-91; acceptance line 725.

Evidence types: code, tests-run.

Evidence references:

- Barrier write classification, evidence spooling, production Class B coalescing, and Class D rollup retention now exist and are gated.

Implementation mapping: The persistence pressure foundation is present for future durable layers. The caveat is that Class D replay merge semantics should be finished before treating the telemetry discipline as fully closed.

Gap / note: This is not the primary readiness blocker by itself; it inherits the Class D replay caveat from REQ-005/REQ-007.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Class D duplicate-window merge semantics missing | High |
| Rust architecture | Pass with caveat | Rollup producer and helper are now well placed; schema lacks merge key | High |
| Rust reliability | Partial | Replay of duplicate telemetry windows is not idempotent/merged | High |
| API contract | Pass | Typed GraphQL/MCP storage diagnostics are covered | Medium |
| Observability / rollout | Partial | Rollup lifecycle exists, but gate misses duplicate merge semantics | High |
| Execution truth | Pass with caveat | Canonical/evidence pressure controls hold; telemetry replay caveat remains | Medium |
| Readiness | Not Ready | One explicit proposal replay requirement remains unresolved | High |

## Routed Specialist Findings

### REL-001: Class D telemetry replay does not merge duplicate metric windows

Reviewer: `rust_reliability_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-005, REQ-007, REQ-012

Evidence types: proposal, code, migration, config, tests-run.

Evidence references:

- P075 says Class D must use metric bucket key and time window, and duplicate rollups merge by additive counters or max gauges (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:249`).
- Registry declares `storage_health.insert_write_pressure_snapshot` as Class D with replay policy `telemetry_merge` (`control-plane/crates/db/write-operation-registry.toml:848`).
- The storage write-pressure table primary key is generated `id`; no unique metric/window key exists (`control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql:10`).
- Snapshot insert uses plain `INSERT`, not upsert/merge (`control-plane/crates/db/src/repos/storage_health.rs:71`).
- `record_live_write_pressure_rollup` creates a point-in-time `now - 1ms` to `now` snapshot, not an accumulated flush-period bucket with duplicate-window merge state (`control-plane/crates/db/src/repos/storage_health.rs:118`).

Why it matters:

The current producer/retention implementation is useful and closes the previous absence of Class D rollup. But replay remains weaker than the declared Class D contract: retrying or duplicating a rollup window can create duplicate rows or fail on repeated generated IDs instead of merging additive counters and max gauges. That undermines the telemetry_merge replay policy and leaves the gate unable to prove idempotent Class D recovery behavior.

Recommended action:

Represent the Class D rollup key explicitly, for example `(metric_bucket, window_start, window_end)`, and persist snapshots with an upsert that merges additive counters and max gauges. If storage write-pressure snapshots intentionally use a single canonical bucket, encode that bucket in schema/payload and make `(bucket, window_start, window_end)` unique. Update the registry duplicate-application evidence to point to the test.

Acceptance criteria:

- Schema or repository code enforces one row per Class D metric bucket/window.
- Duplicate inserts for the same bucket/window merge counters and max gauges rather than inserting another row or failing.
- A test exercises duplicate-window replay through the production Class D helper.
- `./scripts/test-gate.sh proposal-075` fails if the merge/upsert path is removed.

### READY-001: P075 gate still does not prove Class D duplicate-window merge semantics

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-005, REQ-007, REQ-012

Evidence types: proposal, code, tests-run.

Evidence references:

- P075 gate must prove telemetry is rolled up and cannot starve Class A (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:703`).
- The gate passed on the audited tree and now includes production Class B, Class D drop-counter, and Class D rollup producer/retention tests.
- The gate's Class D lifecycle static checks require producer/retention strings but do not require merge/upsert semantics (`scripts/test-gate.sh:5725`).

Why it matters:

The gate can now pass while Class D replay semantics remain incomplete. That makes it too weak as the final closeout gate for the full P075 telemetry contract.

Recommended action:

Extend the P075 gate with a duplicate-window rollup test that proves additive counters and max gauges merge through the production Class D helper.

Acceptance criteria:

- The gate includes a named Class D duplicate-window merge regression.
- The regression fails against plain `INSERT` semantics.
- The gate keeps the existing producer/retention/drop-counter/static lifecycle checks.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Same-tree canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-075` passed on `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab` |
| Production Class B coalescing regression | Passed | `p075_projection_rebuild_uses_production_class_b_coalescing` passed |
| Class D drop-counter regression | Passed | `class_d_telemetry_drop_counter_is_observable_via_storage_health` passed |
| Class D producer/retention regression | Passed | `class_d_rollup_producer_persists_bounded_snapshot_and_purges_retention` passed |
| Direct runtime DB bypass gate | Passed | Runtime direct SQL scan clean |
| Evidence spool file/pointer discipline | Passed | Proposal gate evidence spool tests passed |
| GraphQL/MCP typed diagnostics | Passed | Proposal gate storageHealth and MCP diagnostics tests passed |
| Class D duplicate-window merge semantics | Failed | No unique metric/window key, no upsert/merge, no gate test |
| Live runtime daemon validation | Not run | Not required for this read-only audit; no runtime claim made |

## Verification Log

Commands and inspections performed:

- `git status --short --branch` -> `## main...origin/main`
- `git log -1 --oneline --decorate` -> `83daf7e4 (HEAD -> main, origin/main, origin/HEAD) Register DbWriter before steward bootstrap`
- `git merge-base HEAD origin/main` -> `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` -> R10 report path
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` -> no proposal-review artifacts
- Focused `rg` searches for `insert_write_pressure_snapshot`, `StorageWritePressureSnapshot::new`, `record_live_write_pressure_rollup`, `TELEMETRY_*`, `storage_write_pressure_snapshots`, `ON CONFLICT`, and retention/deletion logic
- Focused reads of `writer.rs`, `projections.rs`, `storage_health.rs`, `scheduler.rs`, `daemon/src/main.rs`, `047_p075_storage_write_pressure_snapshots.sql`, P075 tests, and `scripts/test-gate.sh`
- `./scripts/test-gate.sh proposal-075` -> passed

Notable gate evidence:

- `p075_projection_rebuild_uses_production_class_b_coalescing ... ok`
- `class_d_telemetry_drop_counter_is_observable_via_storage_health ... ok`
- `class_d_rollup_producer_persists_bounded_snapshot_and_purges_retention ... ok`
- `P075 fail-closed registry check passed: 5 bypasses, 122 operations, 113 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`
- `Proposal 075 gate passed`

## Final Verdict

The implementation is materially improved since R9. The prior blocker is closed: Class D now has a production daemon producer, persisted write-pressure snapshots, retention purge, storageHealth readback, and gate coverage. The canonical P075 gate passes on the audited HEAD.

The implementation is still not fully ready for P075 closeout because one explicit replay contract remains incomplete: duplicate Class D rollups for the same metric bucket/time window must merge by additive counters or max gauges. Current storage write-pressure snapshots are inserted by generated UUID without a unique metric/window key or upsert/merge path, and the gate does not prove the merge behavior.

Recommended next actions:

1. Add a schema/repository-level Class D metric bucket/window merge path.
2. Add a production-helper regression for duplicate-window additive/max-gauge merge behavior.
3. Extend `./scripts/test-gate.sh proposal-075` so the gate fails on plain insert-only telemetry rollups.
