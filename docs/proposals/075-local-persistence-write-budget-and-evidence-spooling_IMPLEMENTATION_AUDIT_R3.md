# Proposal 075 Implementation Audit R3

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit report | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R3.md` |
| Audit timestamp | 2026-05-09T02:39:03+03:00 |
| Implementation target | Branch `cw/implement-proposal-075-local-p/4aeb45a9` |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Audited HEAD | `7fdf016ef421f0d690d340b9cd9c3c390600caf8` |
| Compare base | `origin/main` merge-base `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Working tree status | Clean before report creation |
| Proposal state | Active |
| Overall conformance | Not Implemented |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High for static/code/gate findings; Medium for live runtime behavior because no daemon canary run was exercised |

## Implementation Target / Compare Base

The audited tree is the explicit branch worktree supplied by the operator:

- Worktree: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9`
- Branch: `cw/implement-proposal-075-local-p/4aeb45a9`
- HEAD: `7fdf016ef421f0d690d340b9cd9c3c390600caf8`
- Compare base: merge-base with `origin/main` at `70b03d1af641bbcb76a745449cc18f9a8fddea4c`
- Working tree: clean before this report was written

The branch advanced since R2. This audit rechecked the implementation rather than reusing R2 conclusions.

## Prior Proposal-Review Reuse

| Field | Result |
|---|---|
| Prior proposal-review artifacts discovered | None |
| Existing implementation audit reports | `R1` and `R2` exist but were ignored for reviewer-selection per audit rules |
| Reviewer-selection reuse | Not reused |
| Rationale | The discovery helper returned no proposal-review artifacts for this proposal. Reviewers were selected from the active proposal contract and current implementation evidence. |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | DbWriter ownership, repository write gateway boundaries, runtime producer routing, and Rust persistence module shape |
| `rust_reliability_reviewer` | bounded queues, heartbeat/shutdown, startup orphan recovery, direct-write bypasses, and spool reconciliation |
| `api_contract_reviewer` | typed GraphQL `storageHealth`, MCP storage diagnostics, auth capability mapping, parameter and error semantics |
| `observability_rollout_reviewer` | baseline evidence, thresholds, storage health readbacks, diagnostics export, kill switches, rollout gates |
| `chainworks_execution_truth_reviewer` | run/stage/artifact evidence truth, compact spool references, recovery metadata, and durable evidence claims |

Rejected close alternatives:

- `rust_performance_reviewer`: performance is central to P075, but the hard cap was kept at five and performance gaps are captured through REQ-010 plus observability findings.
- `apple_arch_reviewer`: Swift changes are limited to diagnostics export plumbing and do not change app state ownership beyond existing lifecycle support.
- `macos_ui_reviewer`: no proposal-mandated macOS UI layout or interaction surface changed.
- `product_reviewer`: proposal metrics are operational rollout metrics, not product decision metrics.
- `rust_security_reviewer`: storage tool auth and path-safety evidence were inspected through API/architecture/reliability lenses; no new unsafe/FFI or public security surface warranted replacing a core reviewer.

## Proposal Contract Summary

P075 requires a local persistence write-budget system for the Rust control plane and macOS diagnostics surface:

- route non-test runtime writes through a bounded, priority-laned `DbWriter` or through a source-controlled temporary bypass allowlist with owners and retirement criteria;
- classify write classes A/B/C/D with deadlines, typed `WriteResult` variants, replay/idempotency contracts, shutdown behavior, queue bounds, coalescing, and mandatory flush behavior;
- spool high-volume evidence to files and persist compact `EvidenceSpoolRef` metadata instead of raw row-per-chunk evidence;
- recover orphaned evidence files on daemon startup and through a bounded MCP reconcile tool;
- expose typed, freshness-aware storage diagnostics through GraphQL `storageHealth`, parity MCP diagnostics, structured logs, and macOS diagnostics export;
- capture baseline write-pressure metrics and use thresholds, gates, kill switches, and reference docs for staged rollout and closeout;
- fail closed through the `proposal-075` gate on unapproved direct writes, unregistered DbWriter operations, raw high-volume SQLite evidence, schema drift, and missing closeout evidence.

Platform/product scope: macOS operator app plus Rust control-plane data, worker, API, MCP, rollout, and diagnostics scope. No iOS scope.

Primary service/operator flows audited:

1. Runtime persistence writes enter a bounded `DbWriter` lane with ordered admission, idempotency, deadlines, and shutdown behavior.
2. High-volume evidence is written file-first and represented in SQLite by compact, validated metadata.
3. Crash-orphaned evidence files are recovered during daemon startup and by scoped MCP maintenance.
4. Operators and diagnostics clients read storage pressure, spool state, WAL/degraded status, thresholds, freshness, and kill-switch state through GraphQL/MCP/export surfaces.
5. The canonical P075 gate proves the implemented slice and prevents drift before handoff.

## Implementation Evidence Pack

Touched/inspected implementation surfaces:

- Rust DB/writer: `control-plane/crates/db/src/writer.rs`, `control-plane/crates/db/src/evidence_spool.rs`, `control-plane/crates/db/src/repos/evidence_spool_refs.rs`, `control-plane/crates/db/src/repos/storage_health.rs`
- Rust engine producers: `control-plane/crates/engine/src/evidence.rs`, `control-plane/crates/engine/src/executor.rs`
- Daemon startup: `control-plane/crates/daemon/src/main.rs`, `control-plane/crates/daemon/src/storage_startup.rs`
- GraphQL: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/storage.rs`
- MCP/auth: `control-plane/crates/mcp-server/src/tools/storage.rs`, `control-plane/crates/auth/src/lib.rs`
- macOS diagnostics: `Chainworks Forge/Views/DaemonLifecycleSurface.swift`, `Chainworks Forge/Support/DaemonLifecycleClient.swift`, `Chainworks Forge/Support/DiagnosticsBundle.swift`
- Gates/docs/evidence: `scripts/test-gate.sh`, `docs/evidence/p075/phase1-baseline.md`, `docs/reference/current-system-baseline.md`, `docs/reference/rust-control-plane.md`, `docs/reference/test-gates.md`

Tests and validation:

- Same-tree canonical gate executed: `./scripts/test-gate.sh proposal-075`
- Result: passed on audited HEAD `7fdf016ef421f0d690d340b9cd9c3c390600caf8`
- Gate output included: `P075 fail-closed registry check passed: 36 bypasses, 6 operations, 3 observed db/src operation literals, direct write call sites allowlisted` and `Proposal 075 gate passed`

Runtime evidence not obtained:

- No live daemon canary run, file-backed workload, or remote UI smoke was executed during this audit.
- UI tests remain remote-only by repository policy and no UI behavior was in scope.

## Implementation Fingerprint

Stack tags:

- Rust control-plane persistence, engine evidence producer, daemon startup, GraphQL server, MCP server, auth capability mapping
- macOS Swift diagnostics export client

Surface tags:

- SQLite write gateway, evidence spool, storage health readback, MCP diagnostics, diagnostics bundle export, proposal gate, rollout/reference docs

Risk tags:

- direct write bypass retirement, live telemetry/baseline absence, shared writer heartbeat visibility, high-volume producer adoption, diagnostics parity/error semantics, operational closeout evidence

## Fidelity Inventory

Matches:

- `DbWriter` primitives, classes, lanes, deadlines, shutdown behavior, coalescing, heartbeat primitive, registry, and operation tests exist and the P075 gate passes.
- Evidence spool primitives and compact `evidence_spool_refs` metadata exist with path validation, checksums, idempotent inserts, and orphan recovery primitives.
- Failed-stage evidence writes a full spooled packet and stores a compact pointer through `DbWriter`.
- ACP transcript spooling now exists behind `CHAINWORKS_PERSIST_ACP_TRANSCRIPTS=1` and registers `p075_acp_transcript_spool_ref_insert`.
- Daemon startup now calls the bounded P075 evidence-orphan sweep and has a startup recovery test.
- GraphQL `storageHealth` is now a typed `GqlStorageHealth` object rather than an opaque JSON field, with fail-closed mapping tests.
- MCP storage diagnostics now honor key parameters such as `includeThresholds`, `windowSeconds`, `includeLanes`, `runId`, `includeOrphans`, `dryRun`, and `maxFiles`; non-dry-run reconcile requires `runId`.
- macOS diagnostics export now attempts to fetch storage diagnostic snapshots through `DaemonLifecycleClient` and writes `storage_health.json` / `evidence_spool_summary.json` when available.
- The proposal gate expanded to cover daemon startup sweep wiring, typed GraphQL, auth capability mapping, MCP parameter semantics, and fail-closed registry checks.

Divergences:

- `DbWriter` is not the normal injected runtime write gateway. Runtime producers create ad hoc writers, and most repository/runtime writes remain direct writes accepted through phase-8 bypass entries.
- `storage_health()` does not consume a live daemon `DbWriterHeartbeat`; it hard-codes a fail-closed stale/degraded writer state and leaves live wait/transaction metrics absent.
- Baseline evidence still records `pending_live_canary` metrics for write-lock wait, busy retry rate, command latency, and WAL size.
- The high-volume producer conversion is incomplete: failed-stage evidence and optional ACP transcript spooling are covered, but the full transcript/tool trace/stdout/stderr/receipt/model-delta/runtime-event producer set is not proven converted.
- MCP storage tools are much closer to the proposal contract, but the explicit typed error model (`unavailable`, `stale`, `unauthorized`, `invalid_input`, `maintenance_disabled`) is not proven or gate-enforced.
- Phase 8 bypass retirement is not complete; docs still frame the state as an implemented closeout slice with remaining direct writes visible through the allowlist.

Ambiguities / Evidence Gaps:

- No live canary workload was available to prove actual p50/p95 write-lock wait, command latency, WAL size, busy retry rate, or storageHealth threshold behavior.
- No runtime GraphQL/MCP daemon instance was exercised; contract confidence comes from code/tests/gate evidence.
- The branch documents current behavior as a closeout slice, while the active proposal still requires final bypass retirement and live baseline/threshold promotion evidence.

## Requirement Summary

| REQ | Title | Status |
|---|---|---|
| REQ-001 | Runtime writes use `DbWriter` or source-controlled temporary bypasses | Partially Implemented |
| REQ-002 | DbWriter lanes, deadlines, results, coalescing, idempotency, and shutdown | Implemented |
| REQ-003 | DbWriter heartbeat and live storageHealth writer readback | Partially Implemented |
| REQ-004 | Write operation registry and fail-closed operation validation | Implemented |
| REQ-005 | Evidence spool primitives and compact metadata constraints | Implemented |
| REQ-006 | Real high-volume evidence producers use file spool | Partially Implemented |
| REQ-007 | Startup orphan recovery and scoped MCP reconcile | Implemented |
| REQ-008 | Typed GraphQL `storageHealth` contract | Implemented |
| REQ-009 | MCP storage diagnostics parity and typed errors | Partially Implemented |
| REQ-010 | Telemetry rollup, write-pressure snapshots, WAL/lock metrics, and live baseline | Missing |
| REQ-011 | macOS diagnostics export includes storageHealth and evidence spool summary | Implemented |
| REQ-012 | Proposal gate proves the full P075 closeout contract | Partially Implemented |
| REQ-013 | Phase 8 bypass retirement and durable reference truth | Missing |

## Detailed REQ Audit

### REQ-001 - Runtime Writes Use DbWriter Or Source-Controlled Temporary Bypasses

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:136`, `:412`, `:715`, `:821-828`.
- Status: Partially Implemented.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-22`, `:87-411`
  - `control-plane/crates/engine/src/evidence.rs:178-180`
  - `control-plane/crates/engine/src/executor.rs:6342`
  - `control-plane/crates/db/src/writer.rs:468-541`
  - `scripts/test-gate.sh:5464-5637`
- Implementation mapping: the source-controlled bypass allowlist exists; gate output proves all currently observed direct write sites are allowlisted; failed-stage evidence and ACP transcript metadata can submit Class C writes through `DbWriter`.
- Gap / note: the active runtime architecture still relies on 36 phase-8 bypasses and ad hoc producer-local `DbWriter::new(...)` instances. That is not the proposal's engine-facing constructor model where writers receive a shared `DbWriter` and storageHealth observes that live gateway.

### REQ-002 - DbWriter Lanes, Deadlines, Results, Coalescing, Idempotency, And Shutdown

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:134-230`, `:681`.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:466-725`, `:1063-1158`, `:1674-1685`
  - `control-plane/crates/db/write-operation-registry.toml:22-57`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: bounded per-lane queues, priority drain, class B coalescing, deadlines, typed results, heartbeat primitive, graceful shutdown drain, and operation registration are implemented and gate-covered.
- Gap / note: no isolated primitive gap found. Runtime adoption gaps are tracked in REQ-001, REQ-003, and REQ-013.

### REQ-003 - DbWriter Heartbeat And Live StorageHealth Writer Readback

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:138`, `:520`, `:784`.
- Status: Partially Implemented.
- Evidence types: code, tests-found.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:386-457`, `:507-523`, `:1137`, `:1674-1685`
  - `control-plane/crates/db/src/repos/storage_health.rs:81-185`
  - `control-plane/crates/db/src/repos/storage_health.rs:116-143`
- Implementation mapping: `DbWriterHeartbeat` exists and the writer records heartbeat/drain state internally.
- Gap / note: `storage_health()` does not receive or read the live daemon writer. It explicitly reports `writer_alive = false`, `isStale = true`, null `lastHeartbeatAt` / `lastDrainAt`, null write-lock wait p50/p95, null transaction duration p95, and zero queue depths. That is fail-closed, but it is not the proposal's live heartbeat/readback contract.

### REQ-004 - Write Operation Registry And Fail-Closed Operation Validation

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:404-412`, `:677-720`.
- Status: Implemented.
- Evidence types: config, code, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-operation-registry.toml:22-57`
  - `control-plane/crates/engine/src/executor.rs:6336`
  - `scripts/test-gate.sh:5464-5637`
  - `./scripts/test-gate.sh proposal-075` passed with 6 operations and 3 observed db/src operation literals
- Implementation mapping: the observed `WriteOperation.operation_name` values are registered and the gate fails closed on registry/direct-write drift.
- Gap / note: registry coverage only applies to writes routed through `WriteOperation`; remaining direct writes are governed by the bypass allowlist until Phase 8 retirement.

### REQ-005 - Evidence Spool Primitives And Compact Metadata Constraints

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:297-321`, `:718`.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/evidence_spool.rs`
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs`
  - `docs/reference/rust-control-plane.md:395-410`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: file-before-metadata ordering, checksum, fsync/atomic commit, path containment, summary validation, idempotent metadata insertion, and orphan-safe behavior are implemented and documented.
- Gap / note: none for the shared primitives.

### REQ-006 - Real High-Volume Evidence Producers Use File Spool

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:73`, `:718`, `:823-825`.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/engine/src/evidence.rs:176-187`
  - `control-plane/crates/engine/src/executor.rs:6288-6342`
  - `docs/reference/rust-control-plane.md:397-412`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: failed-stage evidence and optional ACP transcript persistence use the evidence spool path and compact metadata.
- Gap / note: the proposal identifies transcripts, tool traces, stdout/stderr snippets, and raw runtime events as primary spooling candidates. This audit found only partial producer adoption, with ACP transcript spooling gated by an environment variable and no proof that the full producer set has been converted.

### REQ-007 - Startup Orphan Recovery And Scoped MCP Reconcile

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:321`, `:574`, `:720`, `:823`.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/daemon/src/main.rs:289-309`
  - `control-plane/crates/daemon/src/storage_startup.rs:26-70`, `:82-166`
  - `control-plane/crates/mcp-server/src/tools/storage.rs:175-207`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: daemon startup performs a best-effort bounded sweep across active run artifact roots; MCP reconcile resolves artifact root server-side, requires `runId` for non-dry-run calls, and delegates to `sweep_evidence_orphans`.
- Gap / note: no implementation gap found for startup/manual recovery in this audit.

### REQ-008 - Typed GraphQL storageHealth Contract

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:510-542`, `:672`, `:693`, `:723`.
- Status: Implemented.
- Evidence types: schema, code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/graphql-server/src/types/storage.rs:6-155`, `:279-296`
  - `control-plane/crates/graphql-server/src/schema.rs:534-538`
  - `control-plane/crates/graphql-server/src/schema.rs:3944-3985`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: GraphQL exposes typed `GqlStorageHealth` with writer, WAL, projections, evidence spool, kill switches, thresholds, freshness, and degraded/unavailable handling. Tests assert the SDL is not `storageHealth: JSON` and the typed query succeeds.
- Gap / note: typed schema conformance is implemented. Live metric quality is separately incomplete under REQ-003 and REQ-010.

### REQ-009 - MCP Storage Diagnostics Parity And Typed Errors

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:546-574`, `:693-694`.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:11-80`, `:90-207`, `:283-345`
  - `control-plane/crates/auth/src/lib.rs:672-676`
  - `scripts/test-gate.sh:5499-5504`
  - `./scripts/test-gate.sh proposal-075` passed
- Implementation mapping: MCP exposes storage health, write pressure, evidence spool summary, and reconcile tools. The current implementation honors core parameters and the gate covers auth boundary plus parameter semantics.
- Gap / note: the explicit proposal error model (`unavailable`, `stale`, `unauthorized`, `invalid_input`, `maintenance_disabled`) is not proven as a typed MCP contract. Some errors are ordinary Rust errors, and the gate does not validate GraphQL/MCP agreement for error envelopes.

### REQ-010 - Telemetry Rollup, Write-Pressure Snapshots, WAL/Lock Metrics, And Live Baseline

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:595`, `:622`, `:730-747`, `:848-849`.
- Status: Missing.
- Evidence types: code, docs, telemetry.
- Evidence references:
  - `docs/evidence/p075/phase1-baseline.md:7`, `:25-30`, `:71-72`
  - `control-plane/crates/db/src/repos/storage_health.rs:41`, `:116-151`
  - `control-plane/crates/db/write-bypass-allowlist.toml:403-411`
- Implementation mapping: the storage write-pressure snapshot repository exists, and storageHealth has fields for write pressure/WAL/thresholds.
- Gap / note: the live baseline is still pending: write-lock wait p50/p95, busy retry rate, command latency p50/p95, and WAL size are all `pending_live_canary`. `storageHealth` reports WAL unavailable and does not roll up live writer wait/transaction metrics. The write-pressure snapshot insertion path is itself still a phase-8 direct-write bypass.

### REQ-011 - macOS Diagnostics Export Includes StorageHealth And Evidence Spool Summary

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:675`, `:696`, `:791`.
- Status: Implemented.
- Evidence types: code, tests-found.
- Evidence references:
  - `Chainworks Forge/Views/DaemonLifecycleSurface.swift:38-45`
  - `Chainworks Forge/Support/DaemonLifecycleClient.swift:462-484`, `:518-554`, `:590`
  - `Chainworks Forge/Support/DiagnosticsBundle.swift:42-46`, `:255-259`
  - `Chainworks ForgeTests/DaemonLifecycleClientTests.swift:186-218`
  - `Chainworks ForgeTests/DiagnosticsBundleTests.swift:270-288`
- Implementation mapping: the export command fetches storage snapshots from the operator GraphQL endpoint when available; the client decodes `storageHealth` and nested `evidenceSpool`; the bundle writer emits `storage_health.json` and `evidence_spool_summary.json`; tests cover decode and bundle inclusion.
- Gap / note: no code-level export gap found. Runtime availability still depends on a live daemon endpoint.

### REQ-012 - Proposal Gate Proves The Full P075 Closeout Contract

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:677-723`, `:860-869`.
- Status: Partially Implemented.
- Evidence types: tests-run, script.
- Evidence references:
  - `scripts/test-gate.sh:5464-5637`
  - `./scripts/test-gate.sh proposal-075` passed on `7fdf016ef421f0d690d340b9cd9c3c390600caf8`
- Implementation mapping: the gate covers DbWriter/spool tests, operation registry, allowlist integrity, raw-evidence static checks, daemon startup sweep wiring, typed GraphQL, auth capability boundary, and MCP parameter semantics.
- Gap / note: the gate still permits 36 phase-8 bypasses and does not require live baseline metrics, live `DbWriter` heartbeat readback, full producer conversion, WAL probing, typed MCP error envelopes, or Phase 8 retirement. It is a strong slice gate, not proof of final proposal closeout.

### REQ-013 - Phase 8 Bypass Retirement And Durable Reference Truth

- Proposal source: `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:828`.
- Status: Missing.
- Evidence types: config, docs, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-22`, `:87-411`
  - `docs/evidence/p075/phase1-baseline.md:71-75`
  - `docs/reference/rust-control-plane.md:363-395`
  - `./scripts/test-gate.sh proposal-075` passed while reporting 36 bypasses
- Implementation mapping: docs and gate make the remaining bypass inventory visible.
- Gap / note: Phase 8 explicitly requires retiring temporary bypass allowlist entries and publishing durable reference truth. The current branch retains 36 phase-8 entries, including 31 `temporary_rollout` entries, and reference docs describe an implemented closeout slice rather than a fully retired write-bypass state.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Not Implemented | Not Ready | REQ-010 and REQ-013 are Missing | High |
| Rust architecture | Partial | Not Ready | Runtime writer is not a shared injected gateway and direct writes remain | High |
| Rust reliability | Partial | Not Ready | Live writer heartbeat/backpressure/shutdown behavior is not observed through the operational path | Medium |
| API contract | Partial | Not Ready | MCP typed error semantics and GraphQL/MCP error parity are not proven | Medium |
| Observability/rollout | Missing for live baseline | Not Ready | Baseline, thresholds, WAL, and live metrics are still pending | High |
| Execution truth | Partial | Not Ready | High-volume evidence producer adoption is incomplete | High |

## Routed Specialist Findings

### ARCH-001 - Runtime DbWriter Gateway Is Still A Slice, Not The Shared Persistence Boundary

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-001, REQ-003, REQ-013; proposal lines 48, 136, 715, 828.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-22`, `:87-411`
  - `control-plane/crates/engine/src/evidence.rs:178-180`
  - `control-plane/crates/engine/src/executor.rs:6342`
  - `control-plane/crates/db/src/repos/storage_health.rs:116-143`
  - gate output: 36 bypasses, direct write call sites allowlisted
- Why it matters: P075's intended operating model is a visible write budget with shared admission, queue pressure, heartbeat, and shutdown behavior. Producer-local `DbWriter::new(...)` instances and broad phase-8 direct-write bypasses prove important primitives, but they do not establish a single runtime write boundary or a live writer state that storageHealth can observe.
- Recommended action: inject a daemon-owned `DbWriter` into engine-facing write paths, route remaining runtime repository owners through registered operations or narrowly justified non-runtime bypasses, and have storageHealth read the same live writer heartbeat/queue state.
- Acceptance criteria: no `temporary_rollout` runtime bypasses remain for in-scope write owners; runtime producers do not construct ad hoc writers for normal operation; storageHealth's writer section reflects the shared live writer.

### OPS-001 - Live Storage Pressure Baseline And Telemetry Rollup Are Still Absent

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-003, REQ-010, REQ-012; proposal lines 52, 595, 622, 730-747, 848-849.
- Evidence types: docs, code, telemetry.
- Evidence references:
  - `docs/evidence/p075/phase1-baseline.md:7`, `:25-30`
  - `control-plane/crates/db/src/repos/storage_health.rs:116-151`
  - `control-plane/crates/db/src/repos/storage_health.rs:178-185`
- Why it matters: P075 promotion decisions depend on objective pressure metrics and thresholds. A fail-closed stale readback is safer than false health, but it cannot support rollout promotion, threshold tuning, or operator diagnosis.
- Recommended action: run the file-backed canary workload, persist the baseline values, roll up live wait/transaction/WAL/busy-retry metrics into storageHealth, and gate closeout on non-pending baseline values.
- Acceptance criteria: baseline file contains concrete values; storageHealth reports non-null live writer metrics when the writer is alive; stale/degraded states are reserved for actual missing or old data; the proposal gate fails on `pending_live_canary`.

### TRUTH-001 - High-Volume Evidence Producer Conversion Is Not Complete

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-006, REQ-012; proposal lines 73, 718, 823-825.
- Evidence types: code, docs, tests-run.
- Evidence references:
  - `control-plane/crates/engine/src/evidence.rs:176-187`
  - `control-plane/crates/engine/src/executor.rs:6288-6342`
  - `docs/reference/rust-control-plane.md:397-412`
  - `./scripts/test-gate.sh proposal-075` passed
- Why it matters: The proposal is about reducing local SQLite write pressure from high-volume evidence. Converting one failed-stage packet path plus optional ACP transcript persistence is meaningful, but not enough to prove that transcripts, tool traces, stdout/stderr snippets, receipts, model deltas, and runtime events are no longer raw high-volume DB pressure.
- Recommended action: inventory each real high-volume producer, route each through `write_spool_file` plus compact `EvidenceSpoolRef`, and add gate coverage that fails when a producer regresses to raw SQLite payload persistence.
- Acceptance criteria: producer-by-producer evidence shows one compact metadata row per logical object; all primary high-volume producer kinds have tests or gate fixtures; direct raw payload writes are rejected by the P075 gate.

### API-001 - MCP Storage Error Contract Is Not Fully Proven

- Reviewer: `api_contract_reviewer`
- Severity: Minor
- Confidence: Medium
- Related proposal items / REQs: REQ-009, REQ-012; proposal line 548.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:90-207`
  - `control-plane/crates/auth/src/lib.rs:672-676`
  - `scripts/test-gate.sh:5499-5504`
- Why it matters: P075 explicitly requires typed MCP errors for unavailable, stale, unauthorized, invalid_input, and maintenance_disabled states, with GraphQL/MCP agreement on units and enums. Parameter behavior is now covered, but callers still do not have a proven stable error envelope for diagnostics and maintenance failures.
- Recommended action: define the storage diagnostics error envelope, map invalid input and unavailable/stale/maintenance-disabled conditions into typed responses, and add GraphQL/MCP parity tests for both success and error cases.
- Acceptance criteria: tests assert each required MCP error kind; unauthorized access remains capability-gated; GraphQL/MCP units/enums and stale/unavailable meanings match.

### READY-001 - Passing Proposal Gate Is Not Yet A Final Closeout Gate

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-010, REQ-012, REQ-013; proposal lines 700-723, 821-828.
- Evidence types: tests-run, script, docs.
- Evidence references:
  - `scripts/test-gate.sh:5464-5637`
  - `docs/evidence/p075/phase1-baseline.md:71-75`
  - gate output: `Proposal 075 gate passed`
- Why it matters: The canonical gate passing is good evidence for the implemented slice. It is not yet evidence that the full proposal is ready to close because it accepts phase-8 bypasses, pending live baseline metrics, incomplete producer conversion, and static/fail-closed storageHealth values.
- Recommended action: split or tighten the gate so the closeout mode requires final P075 criteria: no temporary runtime bypasses, live baseline values, live writer readback, full producer conversion, WAL/threshold evidence, and typed MCP error coverage.
- Acceptance criteria: `./scripts/test-gate.sh proposal-075` fails on the current missing closeout items and passes only when the full active proposal contract is satisfied.

## Readiness Checklist

| Item | Status | Evidence / Note |
|---|---|---|
| Canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-075` passed on audited HEAD |
| Full repository regression | Not run | Not required for a negative readiness verdict; canonical proposal gate was the relevant same-tree gate |
| Core Rust writer primitive validation | Passed | Included in proposal gate |
| Startup orphan recovery | Passed by code/gate | Daemon wiring and unit test present |
| GraphQL typed contract | Passed by code/gate | Typed SDL/query covered |
| MCP storage parameter behavior | Passed by code/gate | Parameter semantics covered |
| MCP typed error behavior | Partial | Not proven |
| Live daemon/canary storage pressure | Missing | No runtime workload; baseline values remain pending |
| High-volume producer conversion | Partial | Failed-stage and optional ACP transcript paths found; full producer set not proven |
| Phase 8 bypass retirement | Missing | 36 phase-8 bypasses remain |
| macOS diagnostics export | Implemented by code/tests | Snapshot fetch/decode/bundle paths present |
| UI empty/loading/error/accessibility/localization | Out of scope | No user-visible UI change audited |
| Privacy/permissions/entitlements | No new Apple entitlement risk found | Storage diagnostics remain local/operator-oriented |

## Verification Log

| Command / Check | Result |
|---|---|
| `git status --short --branch` | Clean branch before report creation: `## cw/implement-proposal-075-local-p/4aeb45a9...origin/cw/implement-proposal-075-local-p/4aeb45a9` |
| `git rev-parse HEAD` | `7fdf016ef421f0d690d340b9cd9c3c390600caf8` |
| `git merge-base origin/main HEAD` | `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../075-local-persistence-write-budget-and-evidence-spooling.md` | Returned this R3 report path |
| Prior review discovery helper | Returned no proposal-review artifacts |
| `./scripts/test-gate.sh proposal-075` | Passed on audited HEAD |
| Static/code inspection | Confirmed typed GraphQL, MCP parameter handling, daemon startup sweep wiring, diagnostics export fetch, partial producer adoption, fail-closed storageHealth, pending baseline values, and remaining phase-8 bypasses |

## Final Verdict

Overall conformance: Not Implemented.

Overall implementation readiness: Not Ready.

The branch is materially improved since R2: typed GraphQL `storageHealth`, MCP parameter semantics, daemon startup orphan sweep, diagnostics export snapshot fetch, auth mapping, and ACP transcript spooling all advanced. The canonical P075 gate passes on the audited HEAD.

The proposal still cannot be closed out because live storage pressure/baseline telemetry is missing, storageHealth does not read a live shared writer heartbeat, the high-volume producer conversion is incomplete, the MCP typed error contract is not proven, and Phase 8 bypass retirement is not done.

Recommended next actions:

1. Route runtime write owners through a shared injected `DbWriter` and retire phase-8 `temporary_rollout` bypasses.
2. Wire storageHealth to live writer/WAL/write-pressure metrics and capture the concrete canary baseline.
3. Finish producer-by-producer evidence spool adoption and gate raw high-volume payload regressions.
4. Add typed MCP storage error parity tests and tighten the P075 closeout gate to fail on the current missing closeout criteria.
