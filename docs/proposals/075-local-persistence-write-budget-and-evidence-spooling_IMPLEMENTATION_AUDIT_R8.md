# Proposal 075 Implementation Audit R8

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Proposal state | Approved proposal artifact |
| Audit mode | `auto` |
| Audit date | 2026-05-09 |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree, branch `main` |
| Audited HEAD | `8bb3896e8ec2b1609d04e923d24eab6c3fac2869` |
| Compare base | `origin/main` merge-base `ade1c7104a9a09f1a91854824d2e0c9817d0f24e` |
| Worktree status | Clean, `main...origin/main [ahead 3]` |
| Overall conformance | Partial |
| Overall readiness | Not Ready |
| Audit confidence | High for inspected Rust/storage paths and gates; Medium for runtime load behavior not exercised live |

## Implementation Target

The audit inspected `main` at `8bb3896e8ec2b1609d04e923d24eab6c3fac2869`. The branch contains the merged P075 implementation and the later shared-writer enforcement changes. No implementation files were edited during the audit.

## Prior Review Reuse

Reviewer-selection reuse: Not reused.

The skill helper found no prior proposal-review artifacts for this proposal:

```text
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

Prior `IMPLEMENTATION_AUDIT` reports were intentionally ignored for reviewer selection per the audit skill.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P075 changes Rust DB, engine, daemon, and persistence architecture. |
| `rust_reliability_reviewer` | DbWriter queues, deadlines, backpressure, shutdown, idempotency, and orphan recovery are reliability-critical. |
| `api_contract_reviewer` | P075 adds typed GraphQL `storageHealth` and MCP storage diagnostics. |
| `observability_rollout_reviewer` | P075 owns gates, thresholds, telemetry, diagnostics export, kill-switches, and rollout evidence. |
| `chainworks_execution_truth_reviewer` | Evidence spool refs, run/stage/agent persistence, projections, and recovery affect durable execution truth. |

Rejected close alternatives:

- `macos_ui_reviewer`: macOS remains a thin diagnostics client; no new mutation/control flow is in scope.
- `product_reviewer`: user value and rollout metrics are not the main unresolved risk.
- `security_reviewer`: security-sensitive path containment and auth boundaries are covered by targeted tests and code inspection, but the highest current blockers are reliability/observability semantics.

## Contract Summary

Platform and product scope:

- Apple scope: macOS thin client diagnostics/export only.
- Backend/service scope: Rust control-plane daemon, SQLite persistence, engine producers, GraphQL, MCP, diagnostics, migration, and rollout gates.

Primary service flows:

1. Runtime writes enter the daemon-owned shared `DbWriter`, are classified by operation, and execute through bounded lanes.
2. High-volume evidence writes file bytes first, fsyncs, then persists compact `EvidenceSpoolRef` metadata.
3. Startup/manual orphan reconciliation scans evidence files and backfills or reports orphan metadata.
4. Operators read storage pressure through typed GraphQL `storageHealth`, MCP storage tools, and diagnostics bundles.
5. Proposal gate validation rejects direct runtime write bypasses, raw high-volume SQLite evidence, registry drift, and expired bypasses.

## Fidelity Inventory

Matches:

- The daemon now constructs and registers one shared `DbWriter`, and file-backed registered transaction paths fail closed without it (`control-plane/crates/daemon/src/main.rs:261`, `control-plane/crates/db/src/writer.rs:256`, `control-plane/crates/db/src/writer.rs:280`).
- Direct runtime SQL write scans, operation registry checks, bypass allowlist checks, shared-writer injection checks, and raw-evidence scans are present in `./scripts/test-gate.sh proposal-075` (`scripts/test-gate.sh:5464`).
- Evidence spooling, metadata validation, orphan sweep, GraphQL/MCP storage diagnostics, and diagnostics bundle inclusion are implemented and covered by targeted tests.
- The same-tree canonical gate passed on audited HEAD `8bb3896e`.

Divergences:

- Production projection repository writes are classified as Class B, but the repository transaction helper bypasses the Class B coalescing buffer. The coalescing implementation only runs through `DbWriter::submit`, while `execute_repository_write!` opens a queued transaction directly.
- Production telemetry repository writes are classified as Class D, but there is no Class D rollup/drop policy in admission or storage health. `droppedTelemetryTotal` and lane `droppedTotal` are hard-coded to `0`.
- The gate proves Class B/D operation names are registered with the right class/replay labels, but does not prove production projection coalescing or telemetry rollup/drop behavior on the real repository entrypoints.

Ambiguities / Evidence Gaps:

- No live high-load daemon run was executed; validation is code inspection plus the canonical P075 gate.
- The implementation has synthetic Class B coalescing tests through `DbWriter::submit`, but no test proving real projection repository calls coalesce.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Route all non-test runtime writes through DbWriter or allowed bypasses | Implemented |
| REQ-002 | Declare and enforce class/lane/deadline/idempotency/replay metadata | Partially Implemented |
| REQ-003 | Serialize/prioritize Class A barrier writes with deadlines and metrics | Implemented |
| REQ-004 | Spool high-volume evidence to files with compact SQLite metadata | Implemented |
| REQ-005 | Provide startup orphan recovery and manual reconcile diagnostics | Implemented |
| REQ-006 | Coalesce projection/status updates with bounded flushes | Partially Implemented |
| REQ-007 | Roll up/drop Class D telemetry with memory caps, TTL, and counters | Partially Implemented |
| REQ-008 | Expose typed GraphQL/MCP storage diagnostics with freshness and units | Implemented |
| REQ-009 | Include storage diagnostics in diagnostics export | Implemented |
| REQ-010 | Document and implement forward migration/downgrade behavior | Implemented |
| REQ-011 | Preserve a safe P078 barrier/evidence foundation | Partially Implemented |
| REQ-012 | Gate direct runtime bypasses and raw high-volume SQLite evidence | Implemented |

## Detailed Requirement Audit

### REQ-001: Runtime Writes Through DbWriter Or Allowlist

Source: proposal goals and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:715`).

Status: Implemented.

Evidence:

- Code: shared writer registry and fail-closed file-backed transaction path (`control-plane/crates/db/src/writer.rs:256`, `control-plane/crates/db/src/writer.rs:280`).
- Code: daemon shared writer registration/injection (`control-plane/crates/daemon/src/main.rs:261`).
- Tests-run: `./scripts/test-gate.sh proposal-075` passed; the gate includes runtime direct write scans and shared-writer checks (`scripts/test-gate.sh:5650`, `scripts/test-gate.sh:5678`).

Gap / note: In-memory tests may still create local writers, which is explicitly outside runtime file-backed behavior.

### REQ-002: WriteOperation Metadata And Replay Discipline

Source: proposal goals and idempotency contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:49`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:249`).

Status: Partially Implemented.

Evidence:

- Code: repository operations classify projection writes as Class B and storage telemetry writes as Class D (`control-plane/crates/db/src/writer.rs:201`).
- Code: registry records Class B projection operations and Class D telemetry operations (`control-plane/crates/db/write-operation-registry.toml:651`, `control-plane/crates/db/write-operation-registry.toml:750`, `control-plane/crates/db/write-operation-registry.toml:848`).
- Gap: Class B and Class D idempotency keys in the actual helper are just `operation_name`, not `(run_id, surface, projection_kind)` or metric bucket/time window (`control-plane/crates/db/src/writer.rs:212`, `control-plane/crates/db/src/writer.rs:231`).

### REQ-003: Barrier Write Serialization And Priority

Source: proposal write class, transaction, and gate requirements (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:79`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:186`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:705`).

Status: Implemented.

Evidence:

- Code: priority lanes, shutdown filtering, deadline accounting, and busy classification in `DbWriter`.
- Tests-found/run: writer priority/deadline/shutdown tests run by `./scripts/test-gate.sh proposal-075` (`scripts/test-gate.sh:5474`).

### REQ-004: Evidence Spooling

Source: evidence spooling contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:295`) and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:718`).

Status: Implemented.

Evidence:

- Code/tests: evidence spool file ordering, no-clobber behavior, path ownership, symlink defense, and one metadata pointer per logical object are covered under the P075 gate (`scripts/test-gate.sh:5483`, `scripts/test-gate.sh:5489`).
- Tests-run: `proposal_075_dbwriter.rs` includes high-volume fake stream and file fsync ordering tests.

### REQ-005: Orphan Recovery And Manual Reconcile

Source: orphan recovery contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:318`) and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:720`).

Status: Implemented.

Evidence:

- Tests-run: daemon startup orphan sweep and bounded sweep tests passed in the P075 gate (`scripts/test-gate.sh:5492`).
- Code/tests: MCP `storage.reconcile_evidence_orphans` tool and typed error behavior are covered (`control-plane/crates/mcp-server/src/tools/storage.rs:245`).

### REQ-006: Projection Coalescing

Source: proposal Class B and projection invalidation policy (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:92`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:203`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:275`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:425`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:721`).

Status: Partially Implemented.

Evidence:

- Implemented piece: `DbWriter::submit` routes Class B work through `submit_class_b` (`control-plane/crates/db/src/writer.rs:993`, `control-plane/crates/db/src/writer.rs:1267`), and tests cover synthetic Class B coalescing through `writer.submit` (`control-plane/crates/db/tests/proposal_075_dbwriter.rs:681`).
- Gap: production projection functions call `execute_repository_write!` (`control-plane/crates/db/src/repos/projections.rs:327`, `control-plane/crates/db/src/repos/projections.rs:402`, `control-plane/crates/db/src/repos/projections.rs:430`, `control-plane/crates/db/src/repos/projections.rs:466`), which opens `begin_repository_transaction` (`control-plane/crates/db/src/writer.rs:306`) and then `begin_immediate_transaction` (`control-plane/crates/db/src/writer.rs:1074`). That path does not call `submit_class_b`.

### REQ-007: Class D Telemetry Rollup/Drop

Source: proposal Class D behavior, overflow, idempotency, and rollout budget (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:119`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:209`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:253`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:597`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:722`).

Status: Partially Implemented.

Evidence:

- Implemented piece: Class D type/lane/registry rows exist (`control-plane/crates/db/src/writer.rs:217`, `control-plane/crates/db/write-operation-registry.toml:750`, `control-plane/crates/db/write-operation-registry.toml:848`).
- Gap: production telemetry writes use `execute_repository_write!` (`control-plane/crates/db/src/repos/storage_health.rs:51`, `control-plane/crates/db/src/repos/scheduler.rs:401`) and therefore direct queued transactions rather than any telemetry merge/drop policy.
- Gap: storage health exposes constant zero drop counters (`control-plane/crates/db/src/repos/storage_health.rs:151`, `control-plane/crates/db/src/repos/storage_health.rs:433`, `control-plane/crates/db/src/repos/storage_health.rs:457`).

### REQ-008: Typed GraphQL/MCP Storage Diagnostics

Source: GraphQL and MCP contracts (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:508`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:544`) and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:723`).

Status: Implemented.

Evidence:

- Code/tests: GraphQL typed `storageHealth` tests (`control-plane/crates/graphql-server/src/schema.rs:3969`, `control-plane/crates/graphql-server/src/schema.rs:4030`).
- Code/tests: MCP storage tool dispatch, parameter semantics, typed errors, and GraphQL parity tests (`control-plane/crates/mcp-server/src/tools/storage.rs:359`, `control-plane/crates/mcp-server/src/server.rs:2260`).

Gap / note: The typed surface exists, but its telemetry drop fields currently report placeholder zero values as covered by REQ-007.

### REQ-009: Diagnostics Export

Source: diagnostics requirement (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:670`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:696`).

Status: Implemented.

Evidence:

- Code: `DaemonDiagnosticsExportCommand` pulls storage diagnostics snapshots before exporting (`Chainworks Forge/Views/DaemonLifecycleSurface.swift:39`).
- Code: `DiagnosticsBundleBuilder` writes `storage_health.json` and `evidence_spool_summary.json` when supplied (`Chainworks Forge/Support/DiagnosticsBundle.swift:252`).
- Tests-found: `DiagnosticsBundleTests` asserts the exported files exist.

### REQ-010: Migration And Downgrade/Re-upgrade Behavior

Source: data/schema migration section (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:448`).

Status: Implemented.

Evidence:

- Code: migrations for `evidence_spool_refs`, `storage_write_pressure_snapshots`, and strengthened path constraints (`control-plane/crates/db/migrations/046_p075_evidence_spool_refs.sql`, `control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql`, `control-plane/crates/db/migrations/048_p075_evidence_path_constraints.sql`).
- Reference docs document implemented state (`docs/reference/rust-control-plane.md:394`).

### REQ-011: P078 Barrier/Evidence Foundation

Source: P078 foundation goal and non-goal (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:54`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:59`).

Status: Partially Implemented.

Evidence:

- Implemented piece: barrier and evidence-spool foundation are present.
- Gap: incomplete production Class B/D semantics mean the shared write-budget discipline is not fully reliable for downstream durable layers.

### REQ-012: Proposal Gate

Source: canonical gate requirements (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:698`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:703`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:726`).

Status: Implemented for direct-write/raw-evidence bypass enforcement; incomplete as behavioral proof.

Evidence:

- Tests-run: `./scripts/test-gate.sh proposal-075` passed on audited HEAD.
- Code: gate scans registry, direct write sites, shared writer injection, nested evidence DbWriter reentry, per-call local writer creation, raw evidence patterns, baseline evidence, and producer inventory (`scripts/test-gate.sh:5524`).

Gap / note: Behavioral proof gaps for production Class B/D paths are captured in READY-001.

## Reviewer/Lens Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Class B/D semantics incomplete on production repository paths | High |
| Rust architecture | Needs changes | Split between `submit` semantics and queued repository transaction semantics | High |
| Rust reliability | Not Ready | Coalescing and telemetry shedding do not protect the actual high-churn paths | High |
| API contract | Mostly ready | Typed fields exist, but drop counters are not backed by real counters | Medium |
| Observability/rollout | Not Ready | Passing gate misses production Class B/D behavior | High |
| Execution truth | Needs changes | Projection update pressure is classified but not actually coalesced | Medium |

## Routed Specialist Findings

### REL-001: Production Class B Projection Writes Bypass Coalescing

Reviewer: `rust_reliability_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-002, REQ-006, REQ-011

Evidence type: code, tests-found, tests-run

Evidence references:

- Proposal requires Class B replacement by coalescing key and bounded flushes (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:203`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:275`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:425`).
- `DbWriter::submit` is the only path that routes Class B through `submit_class_b` (`control-plane/crates/db/src/writer.rs:993`).
- The production macro uses `begin_repository_transaction` and a queued immediate transaction (`control-plane/crates/db/src/writer.rs:318`, `control-plane/crates/db/src/writer.rs:1074`).
- Real projection repos use the macro (`control-plane/crates/db/src/repos/projections.rs:327`, `control-plane/crates/db/src/repos/projections.rs:402`, `control-plane/crates/db/src/repos/projections.rs:430`).
- Existing coalescing tests use synthetic `writer.submit`, not real projection repository functions (`control-plane/crates/db/tests/proposal_075_dbwriter.rs:681`).

Why it matters:

P075's write-pressure control depends on repeated non-critical projection work being merged or replaced before it reaches SQLite. The current production path classifies projection writes as Class B but still enqueues each repository transaction as work. Under churn, projection rebuilds can still consume the SQLite writer one-by-one and the merged/flush counters do not describe production behavior.

Recommended action:

Route Class B repository transaction work through a coalescing-aware path, either by migrating the macro to `DbWriter::submit_transaction` for coalescible operations or adding Class B handling to `begin_immediate_transaction`/queued transactions. Derive idempotency keys from the real projection key, for example `(run_id, surface, projection_kind)`.

Acceptance criteria:

- Concurrent real calls such as `projections::rebuild_run_summary(pool, run_id)` coalesce by key.
- Repeated projection invalidations for the same run/read model produce one committed materialization and coalesced results for superseded writes.
- Distinct run/read-model keys remain independent.
- P075 gate includes this production repository-path test.

### OPS-001: Class D Telemetry Is Not Rolled Up Or Dropped With Counters

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-002, REQ-007, REQ-008

Evidence type: code, tests-found, tests-run

Evidence references:

- Proposal requires Class D to aggregate, drop under pressure, increment `dropped_total`, and use metric bucket/time-window keys (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:119`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:209`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:253`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:597`).
- Class D operations are classified but use operation-name idempotency keys (`control-plane/crates/db/src/writer.rs:217`, `control-plane/crates/db/src/writer.rs:231`).
- Production telemetry inserts use `execute_repository_write!` (`control-plane/crates/db/src/repos/storage_health.rs:51`, `control-plane/crates/db/src/repos/scheduler.rs:401`).
- Storage health hard-codes drop counters to zero (`control-plane/crates/db/src/repos/storage_health.rs:151`, `control-plane/crates/db/src/repos/storage_health.rs:433`, `control-plane/crates/db/src/repos/storage_health.rs:457`).

Why it matters:

The operator sees a typed telemetry surface, but the drop fields are not objective counters and telemetry writes are still durable inserts queued in the lowest lane. That does not satisfy the proposal's overload behavior: telemetry should be bounded, mergeable, and droppable before it creates the same pressure it is supposed to measure.

Recommended action:

Implement a Class D telemetry accumulator/drop policy with the documented 1 MiB/10000-sample budget, metric-window idempotency, `DroppedTelemetry` results or equivalent internal accounting, and real `droppedTelemetryTotal`/lane `droppedTotal` readback.

Acceptance criteria:

- Saturating telemetry samples do not block Class A/C work.
- Over-budget Class D input increments real dropped counters.
- Duplicate telemetry windows merge by additive counters or max gauges.
- GraphQL and MCP report the same non-placeholder drop totals.
- P075 gate includes a forced-overload Class D test.

### READY-001: The Canonical Gate Passes Without Proving Production Class B/D Semantics

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related requirements: REQ-006, REQ-007, REQ-012

Evidence type: code, tests-found, tests-run

Evidence references:

- Proposal says the gate must prove coalesced writes are batched/superseded and telemetry is rolled up/cannot starve Class A (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:705`, `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:709`).
- Gate asserts Class B/D registry class/replay labels (`scripts/test-gate.sh:5613`) but does not exercise real projection or telemetry repository calls under pressure.
- Synthetic coalescing tests call `writer.submit` directly (`control-plane/crates/db/tests/proposal_075_dbwriter.rs:712`).
- `./scripts/test-gate.sh proposal-075` passed on audited HEAD despite REL-001 and OPS-001.

Why it matters:

This creates a false closeout signal: the same-tree gate is green, but two proposal-critical behaviors remain unproven or incomplete on production entrypoints.

Recommended action:

Extend `proposal-075` gate coverage beyond classification to production behavior:

- real projection repository coalescing under repeated invalidations;
- Class D over-cap/drop behavior and actual readback counters;
- storageHealth/MCP parity for non-zero dropped telemetry.

Acceptance criteria:

- The gate fails before the behavioral fixes and passes after them.
- Gate docs describe both registry conformance and production behavior coverage.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical P075 gate | Passed | `./scripts/test-gate.sh proposal-075` passed on HEAD `8bb3896e` |
| Full regression / canonical same-tree evidence | Passed for P075 | P075 gate includes db full regression and targeted engine/daemon/graphql/auth/mcp tests |
| Runtime direct write bypass scan | Passed | Gate reports runtime direct SQL scan clean |
| Shared DbWriter injection/fail-closed path | Passed | Gate and code inspection |
| Evidence spooling and orphan recovery | Passed | Gate and tests |
| GraphQL/MCP typed storage diagnostics | Passed | Gate and tests |
| Diagnostics export | Passed by code/tests-found | Swift diagnostics builder/client tests found |
| Production Class B coalescing | Blocked | REL-001 |
| Production Class D rollup/drop counters | Blocked | OPS-001 |

## Verification Log

| Command / Inspection | Result |
|---|---|
| `git status --short --branch` | `## main...origin/main [ahead 3]`, no file changes before report write |
| `git rev-parse HEAD` | `8bb3896e8ec2b1609d04e923d24eab6c3fac2869` |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` | No prior review artifacts found |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` | Selected this R8 report path |
| `./scripts/test-gate.sh proposal-075` | Passed; final line: `Proposal 075 gate passed` |
| Code inspection: writer shared registry | Shared registration and file-backed fail-closed path present |
| Code inspection: projection and telemetry repository paths | Class B/D production semantics incomplete as described in REL-001 and OPS-001 |

## Final Verdict

P075 on `main` is materially stronger than the prior implementation branch state: the shared `DbWriter` runtime boundary, file-backed fail-closed behavior, registry/bypass gate, evidence spooling, orphan recovery, typed storage diagnostics, and diagnostics export are present and the canonical P075 gate passes on the audited HEAD.

The implementation is not ready for closeout because production Class B projection writes and Class D telemetry writes do not yet enforce the coalescing/rollup/drop semantics that P075 explicitly requires. Overall conformance remains Partial, and implementation readiness is Not Ready until REL-001, OPS-001, and READY-001 are closed.
