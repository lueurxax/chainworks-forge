# Proposal 075 Implementation Audit R4

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit report | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R4.md` |
| Audit timestamp | 2026-05-09T08:54:22+03:00 |
| Implementation target | Branch `cw/implement-proposal-075-local-p/4aeb45a9` |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Audited HEAD | `793da3e0b11f9ae7c3c6522a81a502044751f7ce` |
| Compare base | `origin/main` merge-base `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Working tree status | Clean before report creation |
| Proposal state | Active |
| Overall conformance | Not Implemented |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High for static/code/gate findings; Medium for live operational metrics because no daemon canary workload was exercised |

## Implementation Target / Compare Base

The audited target is the explicit branch worktree supplied by the operator:

- Worktree: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9`
- Branch: `cw/implement-proposal-075-local-p/4aeb45a9`
- HEAD: `793da3e0b11f9ae7c3c6522a81a502044751f7ce`
- Compare base: merge-base with `origin/main` at `70b03d1af641bbcb76a745449cc18f9a8fddea4c`
- Working tree: clean before this report was written

The branch advanced since R3 with `86b90341 Harden P075 storage health runtime readback` and `793da3e0 Add live P075 write pressure readback`.

## Prior Proposal-Review Reuse

| Field | Result |
|---|---|
| Prior proposal-review artifacts discovered | None |
| Existing implementation audit reports | `R1`, `R2`, and `R3` exist but were ignored for reviewer-selection per audit rules |
| Reviewer-selection reuse | Not reused |
| Rationale | The discovery helper returned no proposal-review artifacts. Current reviewers were routed from the active proposal contract and current implementation evidence. |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | DbWriter ownership, shared writer injection, repository write boundaries, and Rust persistence shape |
| `rust_reliability_reviewer` | bounded queues, heartbeat/shutdown, orphan recovery, direct-write bypasses, and spool reconciliation |
| `api_contract_reviewer` | typed GraphQL `storageHealth`, MCP diagnostics, auth capability mapping, parameter and error semantics |
| `observability_rollout_reviewer` | baseline evidence, thresholds, live pressure readbacks, diagnostics export, kill switches, rollout gates |
| `chainworks_execution_truth_reviewer` | run/stage/artifact evidence truth, compact spool references, recovery metadata, and high-volume evidence claims |

Rejected close alternatives:

- `rust_performance_reviewer`: performance is central to P075, but the hard cap was kept at five and performance gaps are captured under observability/rollout.
- `apple_arch_reviewer`: Swift changes are diagnostics export plumbing, not a new app-state architecture surface.
- `macos_ui_reviewer`: no proposal-mandated UI screen, navigation, layout, or accessibility surface changed.
- `product_reviewer`: proposal metrics are operational rollout metrics, not product decision metrics.
- `rust_security_reviewer`: auth and path-safety evidence were inspected through API/reliability; no new unsafe/FFI or public security surface displaced a core reviewer.

## Proposal Contract Summary

P075 requires a local persistence write-budget system:

- route non-test runtime writes through bounded priority-laned `DbWriter` writes or a source-controlled temporary bypass allowlist with owners and retirement criteria;
- provide write classes A/B/C/D, deadlines, typed `WriteResult` variants, replay/idempotency, coalescing, shutdown, heartbeat, and queue bounds;
- keep high-volume evidence on local files and persist compact SQLite metadata pointers;
- recover orphaned evidence files on startup and through scoped MCP maintenance;
- expose typed, freshness-aware storage diagnostics through GraphQL `storageHealth`, MCP diagnostics, structured logs, and diagnostics export;
- capture baseline write-pressure metrics, tune thresholds, expose kill switches, and fail closed through the proposal gate;
- retire temporary bypass entries in Phase 8 and publish durable reference truth.

Platform/product scope: macOS operator diagnostics plus Rust control-plane data, worker, API, MCP, rollout, and diagnostics surfaces. No iOS scope.

Primary service/operator flows audited:

1. Runtime persistence writes enter bounded `DbWriter` lanes with ordered admission, idempotency, deadlines, and shutdown behavior.
2. High-volume evidence is written file-first and represented in SQLite by compact, validated metadata.
3. Crash-orphaned evidence files are recovered during daemon startup and by scoped MCP maintenance.
4. Operators and diagnostics clients read storage pressure, spool state, WAL/degraded status, thresholds, freshness, and kill-switch state through GraphQL/MCP/export surfaces.
5. The canonical P075 gate proves the implemented slice and prevents drift before handoff.

## Implementation Evidence Pack

Inspected surfaces:

- Rust DB/writer: `control-plane/crates/db/src/writer.rs`, `control-plane/crates/db/src/evidence_spool.rs`, `control-plane/crates/db/src/repos/evidence_spool_refs.rs`, `control-plane/crates/db/src/repos/storage_health.rs`
- Runtime producers and daemon wiring: `control-plane/crates/engine/src/evidence.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/daemon/src/main.rs`, `control-plane/crates/daemon/src/storage_startup.rs`
- API/MCP/auth: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/storage.rs`, `control-plane/crates/mcp-server/src/server.rs`, `control-plane/crates/mcp-server/src/tools/storage.rs`, `control-plane/crates/auth/src/lib.rs`
- macOS diagnostics: `Chainworks Forge/Views/DaemonLifecycleSurface.swift`, `Chainworks Forge/Support/DaemonLifecycleClient.swift`, `Chainworks Forge/Support/DiagnosticsBundle.swift`
- Gate/docs/evidence: `scripts/test-gate.sh`, `docs/evidence/p075/phase1-baseline.md`, `docs/reference/rust-control-plane.md`, `docs/reference/test-gates.md`, `README.md`

Tests and validation:

- Executed same-tree canonical gate: `./scripts/test-gate.sh proposal-075`
- Result: passed on audited HEAD `793da3e0b11f9ae7c3c6522a81a502044751f7ce`
- Final gate output included: `P075 fail-closed registry check passed: 36 bypasses, 6 operations, 3 observed db/src operation literals, direct write call sites allowlisted` and `Proposal 075 gate passed`
- Warnings were limited to existing dead-code/lifetime-style warnings in Rust crates.

Runtime evidence not obtained:

- No live daemon canary workload was run.
- No remote UI smoke was run; UI tests are remote-only by repo policy and no UI behavior was in scope.

## Implementation Fingerprint

Stack tags:

- Rust persistence/writer, Rust engine evidence producers, daemon startup, GraphQL server, MCP server, auth capability mapping, Swift diagnostics export

Surface tags:

- SQLite write gateway, evidence spool, storage health readback, MCP diagnostics, diagnostics bundle export, proposal gate, rollout/reference docs

Risk tags:

- phase-8 bypass retirement, live baseline absence, incomplete high-volume producer conversion, incomplete writer timestamp/metric readback, MCP error-envelope edge cases, closeout gate looseness

## Fidelity Inventory

Matches:

- `DbWriter` classes, lanes, deadlines, shutdown behavior, coalescing, heartbeat primitive, operation registry, and operation tests exist.
- The daemon now constructs a shared `DbWriter` and injects its heartbeat into GraphQL and MCP.
- `BackgroundExecutor` now owns an injected/shared `DbWriter`; failed-stage evidence uses it when available.
- ACP transcript spooling writes through `self.db_writer` instead of constructing a local writer.
- Evidence spool primitives, path constraints, compact metadata, idempotent inserts, startup orphan sweep, and MCP reconcile exist.
- GraphQL `storageHealth` is typed and has live-writer tests.
- MCP storage tools honor core parameters and have tests for live writer heartbeat, stale annotation, invalid input, and helper error shape.
- macOS diagnostics export fetches storage snapshots and writes `storage_health.json` / `evidence_spool_summary.json` when available.
- The canonical P075 gate passes on the audited HEAD.

Divergences:

- Phase 8 bypass retirement is not complete: 36 allowlist entries remain, including 31 `temporary_rollout` entries.
- `storageHealth.writer.lastHeartbeatAt` and `lastDrainAt` remain `null`; the writer snapshot exposes ages, not the proposal's timestamp fields.
- Live write-lock wait p50/p95, transaction duration p95, busy retry rate, and WAL size are not rolled up into `storageHealth`.
- Baseline evidence still records `pending_live_canary` values for core pressure metrics.
- High-volume producer conversion is partial: failed-stage evidence and optional ACP transcript spooling are covered; the full transcript/tool trace/stdout/stderr/runtime-event producer inventory is not proven converted.
- MCP domain errors improved, but the exact typed error contract is incomplete: unauthorized is enforced before dispatch as method-not-found, `stale` is an annotation rather than an `error: true` envelope, and `maintenance_disabled` is only covered through helper shape rather than a reachable maintenance-disabled path.

Ambiguities / Evidence Gaps:

- No live daemon/canary run was available to validate actual storage pressure under workload.
- No runtime GraphQL/MCP server instance was exercised; evidence comes from code, unit/integration tests, and the canonical gate.
- Reference docs describe the implementation as a closeout slice while the active proposal still includes final bypass retirement and threshold-based canary promotion.

## Requirement Summary

| REQ | Title | Status |
|---|---|---|
| REQ-001 | Runtime writes use `DbWriter` or source-controlled temporary bypasses | Implemented |
| REQ-002 | DbWriter lanes, deadlines, results, coalescing, idempotency, and shutdown | Implemented |
| REQ-003 | DbWriter heartbeat and live storageHealth writer readback | Partially Implemented |
| REQ-004 | Write operation registry and fail-closed operation validation | Implemented |
| REQ-005 | Evidence spool primitives and compact metadata constraints | Implemented |
| REQ-006 | Real high-volume evidence producers use file spool | Partially Implemented |
| REQ-007 | Startup orphan recovery and scoped MCP reconcile | Implemented |
| REQ-008 | Typed GraphQL `storageHealth` contract | Implemented |
| REQ-009 | MCP storage diagnostics parity and typed errors | Partially Implemented |
| REQ-010 | Telemetry rollup, write-pressure snapshots, WAL/lock metrics, and live baseline | Partially Implemented |
| REQ-011 | macOS diagnostics export includes storageHealth and evidence spool summary | Implemented |
| REQ-012 | Proposal gate proves the full P075 closeout contract | Partially Implemented |
| REQ-013 | Phase 8 bypass retirement and durable reference truth | Missing |

## Detailed REQ Audit

### REQ-001 - Runtime Writes Use DbWriter Or Source-Controlled Temporary Bypasses

- Proposal source: lines 48, 136, 412, 715.
- Status: Implemented.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `control-plane/crates/daemon/src/main.rs:257-276`, `:341-361`
  - `control-plane/crates/engine/src/executor.rs:809-816`, `:2443-2468`
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-22`, `:87-411`
  - `scripts/test-gate.sh:5520-5660`
- Implementation mapping: daemon creates one shared `DbWriter`, injects it into `BackgroundExecutor`, GraphQL, and MCP; remaining direct write call sites are source-controlled in the allowlist with retirement metadata and gate coverage.
- Gap / note: this satisfies the rollout-stage "DbWriter or temporary bypass" requirement. Final retirement is tracked separately under REQ-013.

### REQ-002 - DbWriter Lanes, Deadlines, Results, Coalescing, Idempotency, And Shutdown

- Proposal source: lines 134-230, 681.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:466-725`, `:1063-1158`
  - `control-plane/crates/db/write-operation-registry.toml:22-57`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: bounded per-lane queues, priority drain, class B coalescing, deadlines, typed results, heartbeat primitive, graceful shutdown, and operation registration are implemented and gate-covered.
- Gap / note: no primitive-level gap found.

### REQ-003 - DbWriter Heartbeat And Live StorageHealth Writer Readback

- Proposal source: lines 138, 520, 784.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:398-535`
  - `control-plane/crates/db/src/repos/storage_health.rs:86-157`, `:452-492`
  - `control-plane/crates/graphql-server/src/schema.rs:562-577`, `:4029-4072`
  - `control-plane/crates/mcp-server/src/tools/storage.rs:134-169`, `:390-401`
- Implementation mapping: live writer heartbeat is now injected into GraphQL/MCP paths and `storage_health_with_writer` reports `writer.alive`, `totalQueued`, lane depths, oldest queued age, rejected total, and `isStale=false` when the writer is alive.
- Gap / note: proposal fields `lastHeartbeatAt` and `lastDrainAt` are still null because the writer snapshot exposes elapsed ages rather than timestamps. Wait p50/p95 and transaction duration p95 are also null.

### REQ-004 - Write Operation Registry And Fail-Closed Operation Validation

- Proposal source: lines 404-412, 677-720.
- Status: Implemented.
- Evidence types: config, code, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-operation-registry.toml:22-57`
  - `control-plane/crates/engine/src/executor.rs:6362-6371`
  - `scripts/test-gate.sh:5520-5660`
  - gate output: 6 operations, 3 observed db/src operation literals
- Implementation mapping: observed `WriteOperation.operation_name` values are registered and the gate fails closed on registry/direct-write drift.
- Gap / note: no registry/gate validation gap found for currently routed operations.

### REQ-005 - Evidence Spool Primitives And Compact Metadata Constraints

- Proposal source: lines 297-321, 718.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/evidence_spool.rs`
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs`
  - `control-plane/crates/db/migrations/046_p075_evidence_spool_refs.sql`
  - `control-plane/crates/db/migrations/048_p075_evidence_path_constraints.sql`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: file-before-metadata ordering, checksum, fsync/atomic commit, path containment, summary validation, idempotent metadata insertion, and orphan-safe behavior are implemented.
- Gap / note: none for the shared primitives.

### REQ-006 - Real High-Volume Evidence Producers Use File Spool

- Proposal source: lines 73, 718, 823-825.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/engine/src/evidence.rs:16-25`, `:176-190`
  - `control-plane/crates/engine/src/executor.rs:6334-6371`
  - `docs/reference/rust-control-plane.md:397-412`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: failed-stage evidence spools the full packet; ACP transcript persistence has a spool path and now uses the shared executor writer.
- Gap / note: the proposal identifies transcripts, tool traces, stdout/stderr snippets, and raw runtime events as primary spooling candidates. This audit found partial adoption, not a complete producer-by-producer conversion or soak evidence.

### REQ-007 - Startup Orphan Recovery And Scoped MCP Reconcile

- Proposal source: lines 321, 574, 720, 823.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/daemon/src/main.rs:293-315`
  - `control-plane/crates/daemon/src/storage_startup.rs:26-70`, `:82-166`
  - `control-plane/crates/mcp-server/src/tools/storage.rs:244-303`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: daemon startup performs a best-effort bounded sweep; MCP reconcile resolves artifact root server-side, requires `runId` for non-dry-run calls, and delegates to `sweep_evidence_orphans`.
- Gap / note: no startup/manual recovery gap found.

### REQ-008 - Typed GraphQL storageHealth Contract

- Proposal source: lines 510-542, 672, 693, 723.
- Status: Implemented.
- Evidence types: schema, code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/graphql-server/src/types/storage.rs:6-155`, `:279-296`
  - `control-plane/crates/graphql-server/src/schema.rs:562-577`, `:3968-4072`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: GraphQL exposes typed `StorageHealth` with writer, WAL, projections, evidence spool, kill switches, thresholds, freshness, and degraded/unavailable handling. Tests assert the SDL is not JSON and live-writer readback succeeds when injected.
- Gap / note: schema conformance is implemented. Missing live metric values are tracked under REQ-003 and REQ-010.

### REQ-009 - MCP Storage Diagnostics Parity And Typed Errors

- Proposal source: lines 546-574, 693-694.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:11-38`, `:134-303`, `:390-496`
  - `control-plane/crates/mcp-server/src/server.rs:138-149`, `:731-739`, `:950-959`
  - `control-plane/crates/auth/src/lib.rs:672-676`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: MCP exposes the storage tools, receives the daemon writer heartbeat, honors core parameters, and returns typed domain responses for selected invalid/stale/unavailable helper cases.
- Gap / note: the exact proposal error contract is not complete. Unauthorized is enforced as capability filtering before dispatch rather than a typed `unauthorized` tool response; stale health is an `errorCode` annotation without `error: true`; maintenance-disabled is covered through helper shape, not a reachable maintenance-disabled execution path.

### REQ-010 - Telemetry Rollup, Write-Pressure Snapshots, WAL/Lock Metrics, And Live Baseline

- Proposal source: lines 52, 595, 622, 730-747, 848-849.
- Status: Partially Implemented.
- Evidence types: code, docs, telemetry, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/repos/storage_health.rs:41-79`, `:264-389`
  - `docs/evidence/p075/phase1-baseline.md:7`, `:25-30`, `:52-54`
  - `control-plane/crates/db/write-bypass-allowlist.toml:403-411`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: storage write-pressure snapshots and live heartbeat-derived write-pressure readback exist. StorageHealth exposes telemetry rollup policy constants and thresholds.
- Gap / note: the live baseline is still pending: write-lock wait p50/p95, busy retry rate, command latency p50/p95, and WAL size are all `pending_live_canary`. WAL is still unavailable, and p50/p95 wait/transaction metrics are not populated from live telemetry.

### REQ-011 - macOS Diagnostics Export Includes StorageHealth And Evidence Spool Summary

- Proposal source: lines 675, 696, 791.
- Status: Implemented.
- Evidence types: code, tests-found.
- Evidence references:
  - `Chainworks Forge/Views/DaemonLifecycleSurface.swift:38-45`
  - `Chainworks Forge/Support/DaemonLifecycleClient.swift:462-484`, `:518-554`, `:590`
  - `Chainworks Forge/Support/DiagnosticsBundle.swift:42-46`, `:255-259`
  - `Chainworks ForgeTests/DaemonLifecycleClientTests.swift:186-218`
  - `Chainworks ForgeTests/DiagnosticsBundleTests.swift:270-288`
- Implementation mapping: export fetches storage snapshots when available; client decodes `storageHealth` and nested `evidenceSpool`; bundle emits storage JSON files; tests cover decode and bundle inclusion.
- Gap / note: no code-level export gap found. Runtime availability depends on a live daemon endpoint.

### REQ-012 - Proposal Gate Proves The Full P075 Closeout Contract

- Proposal source: lines 677-723, 860-869.
- Status: Partially Implemented.
- Evidence types: tests-run, script.
- Evidence references:
  - `scripts/test-gate.sh:5464-5660`
  - `./scripts/test-gate.sh proposal-075` passed on `793da3e0b11f9ae7c3c6522a81a502044751f7ce`
- Implementation mapping: the gate covers DbWriter/spool tests, db full regression, startup sweep, typed GraphQL, live writer readback, auth capability boundary, MCP parameter/error tests, and fail-closed registry/allowlist checks.
- Gap / note: the gate still passes with 36 phase-8 bypasses, `pending_live_canary` baseline values, null heartbeat timestamp fields, incomplete producer conversion, and missing WAL/lock p50/p95 rollups. It proves a strong slice, not full closeout.

### REQ-013 - Phase 8 Bypass Retirement And Durable Reference Truth

- Proposal source: line 828.
- Status: Missing.
- Evidence types: config, docs, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-22`, `:87-411`
  - `docs/evidence/p075/phase1-baseline.md:71-75`
  - `README.md:112`
  - `docs/reference/rust-control-plane.md:363-395`
  - gate output: 36 bypasses
- Implementation mapping: current docs and gate make remaining bypasses visible.
- Gap / note: Phase 8 requires retiring temporary bypass allowlist entries and publishing durable reference truth. The current branch retains 36 phase-8 entries, including 31 `temporary_rollout` entries, and reference docs still describe an implemented closeout slice.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Not Implemented | Not Ready | Phase 8 is Missing and closeout evidence remains partial | High |
| Rust architecture | Partial | Not Ready | Shared writer injection is improved, but direct runtime owners remain bypassed | High |
| Rust reliability | Partial | Not Ready | Live health readback lacks proposal timestamp and p50/p95 fields | Medium |
| API contract | Partial | Not Ready | MCP error contract is not exact for unauthorized/stale/maintenance-disabled cases | Medium |
| Observability/rollout | Partial | Not Ready | Baseline metrics and WAL/lock rollups are pending | High |
| Execution truth | Partial | Not Ready | High-volume producer conversion is not proven complete | High |

## Routed Specialist Findings

### OPS-001 - Live Baseline And Pressure Metrics Are Still Not Closeout-Ready

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-003, REQ-010, REQ-012; proposal lines 52, 595, 622, 730-747, 848-849.
- Evidence types: docs, code, telemetry, tests-run.
- Evidence references:
  - `docs/evidence/p075/phase1-baseline.md:7`, `:25-30`
  - `control-plane/crates/db/src/repos/storage_health.rs:130-183`
  - `./scripts/test-gate.sh proposal-075` passed
- Why it matters: P075 promotion requires objective pressure metrics and threshold readbacks. The branch now exposes live lane/queue pressure from the writer heartbeat, but the concrete baseline and lock/WAL latency metrics are still absent.
- Recommended action: run the file-backed canary workload, persist concrete baseline values, roll up wait/transaction/WAL/busy-retry metrics into storageHealth, and gate closeout on non-pending baseline values.
- Acceptance criteria: baseline values are concrete; storageHealth reports live p50/p95 wait/transaction/WAL metrics; the gate fails on `pending_live_canary`.

### API-001 - storageHealth Still Omits Mandatory Heartbeat Timestamp Fields

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-003, REQ-008, REQ-010; proposal lines 138, 520.
- Evidence types: code, schema, tests-found.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:398-535`
  - `control-plane/crates/db/src/repos/storage_health.rs:135-143`
  - `control-plane/crates/graphql-server/src/schema.rs:4029-4072`
- Why it matters: Consumers can see `writer.alive`, queues, and freshness, but the proposal explicitly names `lastHeartbeatAt` and `lastDrainAt` as storageHealth fields. Returning null for these fields weakens diagnosis of stale writer state and misses a concrete contract detail.
- Recommended action: record wall-clock heartbeat/drain timestamps or derive auditable timestamp fields alongside monotonic ages, then expose them in GraphQL and MCP.
- Acceptance criteria: live writer readback has non-null `lastHeartbeatAt`; `lastDrainAt` becomes non-null after a write drains; tests assert both fields.

### TRUTH-001 - High-Volume Evidence Producer Conversion Remains Partial

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-006, REQ-012; proposal lines 73, 718, 823-825.
- Evidence types: code, docs, tests-run.
- Evidence references:
  - `control-plane/crates/engine/src/evidence.rs:176-190`
  - `control-plane/crates/engine/src/executor.rs:6334-6371`
  - `docs/reference/rust-control-plane.md:397-412`
  - `./scripts/test-gate.sh proposal-075` passed
- Why it matters: P075 is intended to stop SQLite from acting as a high-volume runtime evidence stream. The current branch proves important producer paths but does not prove the complete inventory is converted or soaked.
- Recommended action: inventory each transcript/tool-trace/stdout/stderr/runtime-event producer, route each through the spool plus compact metadata, and add gate coverage for raw payload regressions per producer class.
- Acceptance criteria: every primary producer has a test or gate fixture proving one compact metadata row per logical object; no primary high-volume producer writes raw payloads directly to SQLite.

### API-002 - MCP Typed Error Contract Is Improved But Not Exact

- Reviewer: `api_contract_reviewer`
- Severity: Minor
- Confidence: Medium
- Related proposal items / REQs: REQ-009, REQ-012; proposal line 548.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:11-38`, `:157-169`, `:256-276`, `:461-496`
  - `control-plane/crates/mcp-server/src/server.rs:352-364`, `:950-959`
- Why it matters: The proposal requires typed unavailable, stale, unauthorized, invalid_input, and maintenance_disabled errors. Current coverage proves selected shapes, but not a uniform callable error contract for every listed condition.
- Recommended action: decide whether storage diagnostic auth failures should intentionally remain capability-hidden or return typed `unauthorized`; make stale/maintenance-disabled behavior consistent; test each listed error through the dispatched MCP path.
- Acceptance criteria: tests assert each required error kind through the real storage tool dispatch path, or the reference docs explicitly narrow the proposal contract with operator-approved rationale.

### READY-001 - The Passing Gate Still Allows Known Closeout Gaps

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-010, REQ-012, REQ-013; proposal lines 700-723, 821-828.
- Evidence types: tests-run, script, docs.
- Evidence references:
  - `scripts/test-gate.sh:5464-5660`
  - `docs/evidence/p075/phase1-baseline.md:71-75`
  - gate output: `Proposal 075 gate passed`
- Why it matters: The gate is valuable and now much broader than R3, but it still accepts the state that the active proposal describes as not yet complete: phase-8 bypasses, pending baseline metrics, incomplete producer conversion, and absent live lock/WAL metrics.
- Recommended action: add a closeout mode or tighten `proposal-075` so final pass requires bypass retirement, concrete baseline values, full producer conversion evidence, and live metric readback.
- Acceptance criteria: the gate fails on the current closeout gaps and passes only when the active proposal's final acceptance criteria are met.

## Readiness Checklist

| Item | Status | Evidence / Note |
|---|---|---|
| Canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-075` passed on audited HEAD |
| Full repository regression | Not run | Not needed for a negative verdict; canonical P075 gate was executed |
| Core Rust writer primitive validation | Passed | Included in proposal gate |
| Shared writer injection | Implemented | Daemon injects shared writer into executor, GraphQL, MCP |
| Startup orphan recovery | Passed by code/gate | Daemon startup wiring and unit test present |
| GraphQL typed contract | Passed by code/gate | Typed SDL/query and live writer tests present |
| MCP storage parameter/error behavior | Partial | Improved tests, but exact error contract gaps remain |
| Live daemon/canary storage pressure | Missing | No runtime workload; baseline values remain pending |
| High-volume producer conversion | Partial | Failed-stage and ACP transcript paths found; full producer set not proven |
| Phase 8 bypass retirement | Missing | 36 phase-8 bypasses remain |
| macOS diagnostics export | Implemented by code/tests | Snapshot fetch/decode/bundle paths present |
| UI empty/loading/error/accessibility/localization | Out of scope | No user-visible UI change audited |
| Privacy/permissions/entitlements | No new Apple entitlement risk found | Storage diagnostics remain local/operator-oriented |

## Verification Log

| Command / Check | Result |
|---|---|
| `git status --short --branch` | Clean branch before report creation: `## cw/implement-proposal-075-local-p/4aeb45a9...origin/cw/implement-proposal-075-local-p/4aeb45a9` |
| `git rev-parse HEAD` | `793da3e0b11f9ae7c3c6522a81a502044751f7ce` |
| `git merge-base origin/main HEAD` | `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../075-local-persistence-write-budget-and-evidence-spooling.md` | Returned this R4 report path |
| Prior review discovery helper | Returned no proposal-review artifacts |
| `./scripts/test-gate.sh proposal-075` | Passed on audited HEAD |
| Static/code inspection | Confirmed shared writer injection, live heartbeat readback, MCP error improvements, remaining pending baseline values, remaining phase-8 bypasses, and partial producer conversion |

## Final Verdict

Overall conformance: Not Implemented.

Overall implementation readiness: Not Ready.

The branch is materially stronger than R3. It now has shared daemon-owned `DbWriter` injection, live writer heartbeat readback in GraphQL/MCP, improved MCP typed-error coverage, and a passing canonical P075 gate on the audited HEAD.

The active proposal still cannot be closed because Phase 8 bypass retirement is missing, live baseline metrics remain `pending_live_canary`, storageHealth still omits the proposal's heartbeat/drain timestamp fields and live lock/WAL p50/p95 metrics, high-volume producer conversion is partial, and the gate still passes with those closeout gaps present.

Recommended next actions:

1. Retire phase-8 `temporary_rollout` bypasses or move non-runtime exemptions into permanent non-runtime categories with narrow rationale.
2. Capture the file-backed canary baseline and wire concrete lock/WAL/transaction metrics into storageHealth.
3. Populate `lastHeartbeatAt` and `lastDrainAt` from live writer state.
4. Finish producer-by-producer high-volume spool adoption and tighten the closeout gate to require it.
5. Complete or explicitly narrow the MCP typed error contract for unauthorized/stale/maintenance-disabled cases.
