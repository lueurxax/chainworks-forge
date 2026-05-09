# Proposal 075 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit report | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R2.md` |
| Audit timestamp | 2026-05-08T22:20:07+03:00 |
| Implementation target | Branch `cw/implement-proposal-075-local-p/4aeb45a9` |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Audited HEAD | `bd906970dbcf4cdf9cf8f75ca77cae0a324615ad` |
| Compare base | `origin/main` merge-base `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Working tree status | Clean before report creation |
| Proposal state | Active |
| Overall conformance | Not Implemented |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for static/code/gate findings; Medium for runtime operational behavior not exercised live |

## Prior Proposal-Review Reuse

| Field | Result |
|---|---|
| Prior proposal-review artifacts discovered | None |
| Existing implementation audit reports | `R1` exists but was ignored for reviewer-selection per audit rules |
| Reviewer selection reuse | Not reused |
| Rationale | No prior proposal-review selection artifacts were found by the discovery helper. Current reviewers were routed from the proposal contract and implementation evidence. |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | DbWriter ownership, repository write gateway boundaries, and Rust crate/module persistence shape |
| `rust_reliability_reviewer` | bounded queues, idempotency, shutdown, recovery, direct-write bypasses, and orphan recovery |
| `api_contract_reviewer` | GraphQL `storageHealth` and MCP storage diagnostics contract parity |
| `observability_rollout_reviewer` | gate behavior, baselines, thresholds, diagnostics export, kill switches, rollout evidence |
| `chainworks_execution_truth_reviewer` | run/stage/artifact evidence truth, compact pointers, recovery metadata, and durable state claims |

Rejected close alternatives:

- `rust_performance_reviewer`: not selected to stay within the hard cap. Performance issues are tracked as proposal conformance and observability/rollout findings.
- `apple_arch_reviewer`: Swift changes are limited to optional diagnostics-bundle bytes and do not introduce new app state ownership beyond the existing exporter.
- `macos_ui_reviewer`: no proposal-mandated UI surface was added or changed.
- `product_reviewer`: product review remains opt-in and metrics are operational rather than product-decision metrics.
- `rust_security_reviewer`: security-sensitive spool path validation exists, but no new public auth boundary or unsafe/FFI surface was found beyond API/OPS coverage.

## Proposal Contract Summary

P075 requires a local persistence write-budget model for the Rust control plane and macOS diagnostics surface:

- route non-test runtime writes through a bounded priority-laned `DbWriter` or a source-controlled temporary bypass allowlist with owners and retirement criteria;
- classify write classes A/B/C/D with deadlines, `WriteResult` semantics, replay/idempotency contracts, shutdown behavior, and queue bounds;
- spool high-volume evidence to files, persist compact `EvidenceSpoolRef` metadata, and recover orphaned files after crashes;
- expose typed, freshness-aware storage diagnostics through GraphQL `storageHealth`, parity MCP tools, structured logs, and diagnostics export;
- capture pre-rollout baseline metrics and use thresholds/gates for rollout promotion;
- make the `proposal-075` gate fail closed on direct-write drift, evidence row-per-chunk regressions, schema/contract drift, and missing evidence.

Platform/product scope: macOS operator app plus Rust control-plane data, worker, API, MCP, rollout, and diagnostics scope. No iOS scope.

Primary service/operator flows audited:

1. Runtime writes enter the persistence layer through `DbWriter` lanes with bounded admission, ordering, idempotency, deadlines, and shutdown behavior.
2. High-volume evidence is written file-first, then represented in SQLite as compact metadata/pointers.
3. Crash-orphaned evidence files are reconciled into durable metadata without blocking startup-critical paths.
4. Operators and diagnostics clients read storage pressure, spool, WAL, thresholds, kill-switch, and freshness state through GraphQL/MCP/export surfaces.
5. The canonical proposal gate proves the implemented slice and prevents regression before handoff.

## Fidelity Inventory

Matches:

- `DbWriter` primitives, classes, lanes, deadlines, shutdown, coalescing, and tests exist in `control-plane/crates/db/src/writer.rs` and pass the P075 gate.
- Evidence spool primitives and metadata repository constraints exist in `control-plane/crates/db/src/evidence_spool.rs` and `control-plane/crates/db/src/repos/evidence_spool_refs.rs`.
- Failed-stage evidence now writes a spooled full packet and stores a compact stage pointer in `control-plane/crates/engine/src/evidence.rs`.
- `storageHealth` and MCP storage tools exist, and diagnostics bundle export can include caller-supplied storage snapshots.
- `./scripts/test-gate.sh proposal-075` passed on the audited HEAD.

Divergences:

- The implementation does not make `DbWriter` the normal runtime write gateway. Many repository/runtime paths still write directly and are accepted through 36 phase-8 bypass entries.
- The live high-volume ACP/tool/transcript/runtime-event producers are not fully spooled; one failed-stage evidence path and fake stream tests are covered.
- GraphQL returns `Json<serde_json::Value>` instead of the proposal's typed `StorageHealth` schema.
- Storage health values are largely synthesized/static: writer heartbeat/drain fields are not wired into readback, WAL probing is unavailable, and kill switches are always empty.
- Manual MCP orphan reconcile exists, but audit evidence did not find daemon startup wiring for the bounded orphan sweep.
- Baseline evidence still records `pending_live_canary` values instead of actual file-backed workload metrics.

Ambiguities / Evidence Gaps:

- No live daemon/file-backed canary run was available during this audit.
- No remote UI smoke was run; UI tests are remote-only by repository policy and no UI change required that path.
- The proposal branch describes a "closeout slice" in README while the proposal itself still includes Phase 8 bypass retirement and live baseline capture.

## Requirement Summary

| REQ | Title | Status |
|---|---|---|
| REQ-001 | DbWriter is the runtime write gateway or every bypass is source-controlled and temporary | Partially Implemented |
| REQ-002 | DbWriter lanes, deadlines, result semantics, coalescing, and shutdown behavior | Implemented |
| REQ-003 | DbWriter heartbeat and live writer health readback | Partially Implemented |
| REQ-004 | Operation registry, idempotency, replay policy, and gate validation | Implemented |
| REQ-005 | Evidence spool primitives and compact metadata constraints | Implemented |
| REQ-006 | Real high-volume evidence producers use file spool instead of SQLite row-per-chunk writes | Partially Implemented |
| REQ-007 | Startup orphan recovery and manual reconcile diagnostics | Partially Implemented |
| REQ-008 | Typed GraphQL `storageHealth` with units, freshness, thresholds, WAL, projections, and kill switches | Partially Implemented |
| REQ-009 | MCP storage diagnostics parity with GraphQL and requested inputs | Partially Implemented |
| REQ-010 | Telemetry rollup, write-pressure snapshots, WAL/lock metrics, and live baseline capture | Missing |
| REQ-011 | macOS diagnostics export includes storageHealth and evidence spool summary | Partially Implemented |
| REQ-012 | Proposal gate proves the full P075 contract fail-closed | Partially Implemented |
| REQ-013 | Phase 8 bypass retirement and durable reference truth | Missing |

## Detailed REQ Audit

### REQ-001 - DbWriter Gateway Or Temporary Bypass Allowlist

- Proposal source: lines 48, 412, 715, 821-828.
- Status: Partially Implemented.
- Evidence: code, config, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:1-25`, `73-220`
  - `scripts/test-gate.sh:5493-5609`
  - direct write scan found many direct calls in `control-plane/crates/db/src/repos/*.rs`, `control-plane/crates/engine/src/*.rs`, and MCP/report paths.
- Implementation mapping: the allowlist is source-controlled and the gate checks direct write call sites under `db/src/repos`.
- Gap: most runtime repository owners remain direct writes under phase-8 bypasses. That is a rollout inventory, not completed gateway adoption.

### REQ-002 - DbWriter Semantics

- Proposal source: lines 134-230, 681.
- Status: Implemented.
- Evidence: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:249-278`, `463-724`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:68-240`, `666-944`
  - `./scripts/test-gate.sh proposal-075` passed.
- Implementation mapping: bounded channels, lane order, class B coalescing, deadlines, shutdown rejection/drain, and `WriteResult` variants are implemented and covered.
- Gap: none for the isolated DbWriter primitive contract.

### REQ-003 - Heartbeat And Live Writer Health

- Proposal source: lines 138, 520, 784.
- Status: Partially Implemented.
- Evidence: code, tests-found.
- Evidence references:
  - `control-plane/crates/db/src/writer.rs:386-457`, `507-523`, `1034-1137`
  - `control-plane/crates/db/src/repos/storage_health.rs:125-138`
- Implementation mapping: `DbWriterHeartbeat` exists and tests assert `is_alive()`.
- Gap: `storage_health()` hard-codes `writer.alive = true` and returns `lastHeartbeatAt`, `lastDrainAt`, and wait metrics as null/zero. It is not reading a live daemon writer instance.

### REQ-004 - Registry, Idempotency, Replay, Gate Validation

- Proposal source: lines 404-412, 677-720.
- Status: Implemented.
- Evidence: config, code, tests-run.
- Evidence references:
  - `control-plane/crates/db/write-operation-registry.toml`
  - `control-plane/crates/db/src/write_class.rs`
  - `scripts/test-gate.sh:5504-5584`
  - `./scripts/test-gate.sh proposal-075` passed with `5 operations, 3 observed db/src operation literals`.
- Implementation mapping: operation names used by `DbWriter` helpers are registered, checked, and covered by the gate.
- Gap: registry coverage only applies to writes already routed through `WriteOperation`; remaining direct writes are governed by the bypass allowlist.

### REQ-005 - Evidence Spool Primitives And Metadata

- Proposal source: lines 297-321, 718.
- Status: Implemented.
- Evidence: code, migration, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/evidence_spool.rs`
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs:860-991`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:430-587`
  - `./scripts/test-gate.sh proposal-075` passed.
- Implementation mapping: file-before-metadata ordering, checksum, fsync, atomic rename, compact metadata, idempotent insert, path ownership, and orphan-safe failure behavior are implemented.
- Gap: none for the shared primitives.

### REQ-006 - Real High-Volume Producer Adoption

- Proposal source: lines 73, 718, 823-825.
- Status: Partially Implemented.
- Evidence: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/engine/src/evidence.rs:143-205`, `426-470`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:589-664`
  - `scripts/test-gate.sh:5489-5490`, `5611-5622`
- Implementation mapping: failed-stage evidence now spools the full packet and stores a compact pointer. A fake high-volume stream test proves one metadata row per logical object.
- Gap: the proposal requires canarying and converting real ACP/tool/transcript/runtime-event producers. Audit evidence found only the failed-stage evidence producer adopted, not the full producer set.

### REQ-007 - Orphan Recovery And Manual Reconcile

- Proposal source: lines 321, 574, 720, 823.
- Status: Partially Implemented.
- Evidence: code, tests-found, tests-run.
- Evidence references:
  - `control-plane/crates/db/src/evidence_spool.rs:260`
  - `control-plane/crates/mcp-server/src/tools/storage.rs:57-80`, `99-150`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:516-587`
  - search found no daemon startup call to `sweep_evidence_orphans` outside MCP/tests.
- Implementation mapping: the sweep function and MCP reconcile tool exist, and the orphan sweep test passes.
- Gap: the bounded startup sweep is not wired into daemon startup evidence found by this audit. MCP dry-run also returns a stub report rather than an actual inventory.

### REQ-008 - GraphQL StorageHealth Contract

- Proposal source: lines 510-542, 672, 693, 723.
- Status: Partially Implemented.
- Evidence: schema, code, tests-found.
- Evidence references:
  - `control-plane/crates/graphql-server/src/schema.rs:529-535`
  - `control-plane/crates/db/src/repos/storage_health.rs:81-175`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:946-996`
- Implementation mapping: GraphQL has a `storage_health` query returning JSON with many proposal-shaped fields.
- Gap: the proposal specified a typed GraphQL `StorageHealth` object. The implementation returns `Json<serde_json::Value>`. Several fields are static/unavailable rather than live readbacks.

### REQ-009 - MCP Storage Diagnostics Parity

- Proposal source: lines 546-574, 693-694.
- Status: Partially Implemented.
- Evidence: code, tests-found.
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/storage.rs:6-154`
  - `control-plane/crates/mcp-server/src/tools/mod.rs:84-120`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs:998-1023`
- Implementation mapping: MCP exposes `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, and `storage.reconcile_evidence_orphans`.
- Gap: request parameters such as `windowSeconds`, `includeLanes`, `runid`, and `includeOrphans` are declared but not applied in execution. Parity is tested at a narrow units level, not contract equivalence.

### REQ-010 - Telemetry, Write Pressure, WAL/Lock Metrics, Baseline

- Proposal source: lines 595, 622, 730-747, 848-849.
- Status: Missing.
- Evidence: code, docs, telemetry.
- Evidence references:
  - `docs/evidence/p075/phase1-baseline.md:23-31`
  - `control-plane/crates/db/src/repos/storage_health.rs:81-175`, `243-307`
  - `control-plane/crates/db/src/pool.rs:67-100`
- Implementation mapping: a storage write-pressure snapshot table and repository exist. `begin_immediate_with_retry` logs wait/retry values.
- Gap: the required baseline values are `pending_live_canary`; WAL size is not probed; writer wait p50/p95, transaction duration, busy retry rate, and command latency are not rolled up from live telemetry into `storageHealth`.

### REQ-011 - Diagnostics Export Includes Storage Snapshots

- Proposal source: lines 675, 696, 791.
- Status: Partially Implemented.
- Evidence: code, tests-found.
- Evidence references:
  - `Chainworks Forge/Support/DiagnosticsBundle.swift:90-119`, `252-280`
  - `Chainworks Forge/Views/DaemonLifecycleSurface.swift:25-40`
  - `Chainworks ForgeTests/DiagnosticsBundleTests.swift:264-290`
- Implementation mapping: `DiagnosticsBundleBuilder` writes `storage_health.json` and `evidence_spool_summary.json` when supplied by the caller.
- Gap: `DaemonDiagnosticsExportCommand.run()` still calls `.defaults(status:)`, which supplies nil storage snapshot data. The actual operator export path does not fetch or attach storage health by default.

### REQ-012 - Proposal Gate Proves Full Contract

- Proposal source: lines 677-723.
- Status: Partially Implemented.
- Evidence: tests-run, script.
- Evidence references:
  - `scripts/test-gate.sh:5464-5632`
  - `./scripts/test-gate.sh proposal-075` passed on `bd906970dbcf4cdf9cf8f75ca77cae0a324615ad`.
- Implementation mapping: the gate runs DbWriter/spool/registry tests and static direct-write/raw-evidence checks.
- Gap: the gate does not prove typed GraphQL SDL, real MCP input semantics, live baseline metrics, daemon startup orphan sweep wiring, diagnostics export end-to-end attachment, or full producer conversion. It also accepts all remaining temporary entries because they are set to phase 8 and the gate only rejects entries with `expires_after_phase < current_phase`.

### REQ-013 - Phase 8 Retirement And Durable Reference Truth

- Proposal source: line 828.
- Status: Missing.
- Evidence: config, docs.
- Evidence references:
  - `control-plane/crates/db/write-bypass-allowlist.toml:19-25`, `73-220`
  - `README.md:112`
  - `docs/reference/rust-control-plane.md:361-410`
- Implementation mapping: reference docs describe the current slice and acknowledge remaining direct write owners.
- Gap: Phase 8 explicitly requires retiring temporary bypass entries and publishing final durable reference truth. Temporary rollout entries remain, and there is no dedicated `docs/reference/local-persistence-write-budget.md` equivalent that presents the completed final contract without open rollout gaps.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Track 1 conformance | Not Implemented | Missing live baseline and bypass retirement; broad partials remain | High |
| Rust architecture | Not Ready | Direct writes remain normal runtime path for many owners | High |
| Rust reliability | Not Ready | startup orphan sweep is not wired; full producer conversion not proven | High |
| API contract | Not Ready | GraphQL is untyped JSON and MCP parameters are not enforced | High |
| Observability/rollout | Not Ready | gate passes despite missing live metrics and incomplete contract proof | High |
| Execution truth | Not Ready | failed-stage evidence pointer is improved, but broad evidence truth is not fully converted | Medium-High |

## Routed Specialist Findings

### ARCH-001 - Direct Write Gateway Adoption Is Still Inventory Mode

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-001, REQ-013
- Evidence: code, config, tests-run.
- Evidence references: `control-plane/crates/db/write-bypass-allowlist.toml:73-220`, direct write scan across `control-plane/crates/db/src/repos`, `scripts/test-gate.sh:5591-5609`.
- Why it matters: P075's core architecture is a bounded write gateway. Keeping most runtime owners as phase-8 allowlisted direct writes leaves the contention and ordering model split between old and new paths.
- Recommended action: route runtime repository owners through `DbWriter` by class/lane, or explicitly re-scope the proposal as a partial closeout slice.
- Acceptance criteria: temporary rollout bypass count monotonically decreases to zero or only permanent non-runtime categories remain; direct runtime writes outside DbWriter fail the gate.

### REL-001 - Startup Evidence Orphan Recovery Is Not Wired

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-007
- Evidence: code, tests-found.
- Evidence references: `control-plane/crates/db/src/evidence_spool.rs:260`, `control-plane/crates/mcp-server/src/tools/storage.rs:99-150`, search found no startup invocation outside MCP/tests.
- Why it matters: file-before-metadata ordering intentionally creates crash windows. Without startup reconciliation, orphan recovery depends on a manual MCP path and does not satisfy the promised startup safety net.
- Recommended action: add bounded daemon startup sweep wiring with backpressure limits, structured results, and non-blocking startup behavior.
- Acceptance criteria: daemon startup invokes the bounded sweep for configured artifact roots, records counts/errors, and tests prove crash-orphan files are reconciled without blocking startup-critical paths.

### API-001 - StorageHealth Is Not The Typed GraphQL Contract

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-008, REQ-009
- Evidence: schema, code.
- Evidence references: `control-plane/crates/graphql-server/src/schema.rs:529-535`, `control-plane/crates/mcp-server/src/tools/storage.rs:85-154`.
- Why it matters: clients cannot depend on typed fields, enum values, nullability, or backward-compatible schema evolution when the GraphQL surface is a raw JSON blob. MCP parity is also narrow because declared parameters are not used.
- Recommended action: implement the proposal's typed `StorageHealth` GraphQL object and apply MCP request parameters or remove unsupported inputs from the contract.
- Acceptance criteria: GraphQL SDL exposes typed `StorageHealth` fields; MCP tools return schema-equivalent units/enums/freshness and honor their documented inputs.

### OPS-001 - Observability Is Mostly Shape, Not Live Operational Evidence

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-003, REQ-008, REQ-010, REQ-012
- Evidence: code, docs, telemetry.
- Evidence references: `docs/evidence/p075/phase1-baseline.md:23-31`, `control-plane/crates/db/src/repos/storage_health.rs:125-173`, `control-plane/crates/db/src/pool.rs:67-100`.
- Why it matters: P075 uses storageHealth, baselines, and thresholds as rollout controls. Static/null writer fields and pending baseline values cannot support canary promotion or operational triage.
- Recommended action: wire live writer heartbeat/drain/queue metrics, WAL probing, write-lock wait rollups, and a file-backed baseline capture into storageHealth and evidence docs.
- Acceptance criteria: baseline contains actual p50/p95/latency/WAL values from a representative file-backed run; storageHealth reports live values with freshness and units; the P075 gate checks the evidence freshness.

### TRUTH-001 - Evidence Truth Is Improved For Failed Stages Only

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-006
- Evidence: code, tests-found.
- Evidence references: `control-plane/crates/engine/src/evidence.rs:143-205`, `control-plane/crates/engine/src/evidence.rs:426-470`.
- Why it matters: the failed-stage path now has a durable full packet and compact pointer, but the proposal covers high-volume runtime evidence generally. A single producer does not establish the repository's evidence truth model.
- Recommended action: convert the named ACP/tool/transcript/runtime-event producers one kind at a time with compatibility pointers and recovery/readback tests.
- Acceptance criteria: each producer kind writes files through the spool, persists one compact metadata row per logical object, and exposes readback/recovery evidence without row-per-chunk SQLite storage.

### READY-001 - Passing Gate Is Necessary But Not Sufficient For Closeout

- Reviewer: readiness synthesis
- Severity: Major
- Confidence: High
- Related REQs: REQ-001 through REQ-013
- Evidence: tests-run, code, docs.
- Evidence references: `./scripts/test-gate.sh proposal-075` passed; gaps listed in REQ-006 through REQ-013.
- Why it matters: the same-tree gate result is positive, but the gate does not yet cover several proposal-critical behaviors. Reporting this branch as ready would convert a partial implementation slice into repository truth.
- Recommended action: keep the branch in implementation/audit loop until missing live baseline, typed contracts, startup sweep wiring, producer conversion, diagnostics export integration, and bypass retirement are addressed or explicitly de-scoped.
- Acceptance criteria: all in-scope REQs are Implemented or deliberately de-scoped in a superseding proposal; canonical gate covers the de-scoped/final contract and passes on the audited HEAD.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-075` passed on `bd906970dbcf4cdf9cf8f75ca77cae0a324615ad`. |
| Full regression suite | Not run | The proposal-specific canonical gate was run because P075 has a dedicated gate. |
| Core DbWriter integration validation | Partial | Primitive and tests pass; broad runtime owner adoption remains allowlisted. |
| Evidence spool runtime validation | Partial | failed-stage evidence path covered; full producer set not converted. |
| Startup/recovery validation | Partial | sweep function/test exists; startup wiring not found. |
| API contract validation | Partial | JSON shape exists; typed GraphQL SDL and full MCP semantics missing. |
| Observability/baseline validation | Missing | baseline values are `pending_live_canary`; live rollups unavailable. |
| macOS diagnostics export | Partial | builder supports supplied bytes; command path does not supply them. |
| Accessibility/localization/privacy/entitlements | Not applicable | No UI/entitlement surface added by the audited implementation. |
| Security/privacy risk | Partial coverage | spool path validation and permissions are tested, but no dedicated security review was selected. |

## Verification Log

Commands and results:

- `git status --short --branch` -> clean branch `cw/implement-proposal-075-local-p/4aeb45a9`.
- `git rev-parse HEAD` -> `bd906970dbcf4cdf9cf8f75ca77cae0a324615ad`.
- `git merge-base origin/main HEAD` -> `70b03d1af641bbcb76a745449cc18f9a8fddea4c`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` -> this R2 report path.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` -> no prior proposal-review artifacts.
- targeted code inspection across `writer.rs`, `evidence.rs`, `storage_health.rs`, GraphQL schema, MCP storage tools, diagnostics bundle, allowlist, baseline docs, reference docs, and gate script.
- `./scripts/test-gate.sh proposal-075` -> passed. The final gate summary reported: `P075 fail-closed registry check passed: 36 bypasses, 5 operations, 3 observed db/src operation literals, direct write call sites allowlisted` and `Proposal 075 gate passed`.

## Final Verdict

P075 is not ready for closeout. The current branch is materially improved over a scaffold: DbWriter primitives are real, evidence spool primitives are strong, failed-stage evidence now uses the spool, storage diagnostic surfaces exist, and the canonical proposal gate passes on the audited HEAD.

The implementation still does not satisfy the full proposal. The blockers are full runtime write routing/bypass retirement, live storage telemetry and baseline capture, startup orphan sweep wiring, typed GraphQL/MCP parity, end-to-end diagnostics export attachment, and full real producer conversion for high-volume evidence.

Recommended next actions:

1. Decide whether this branch is a partial P075 slice or the full P075 implementation. If partial, record that explicitly in proposal/reference truth.
2. Wire and test daemon startup orphan sweep.
3. Convert real ACP/tool/transcript/runtime-event producers beyond failed-stage evidence.
4. Replace JSON GraphQL `storageHealth` with the typed schema and enforce MCP parameter semantics.
5. Wire live writer/WAL/lock metrics, capture a file-backed baseline, and make the gate validate baseline freshness.
6. Retire temporary runtime bypasses or change the proposal scope before closeout.
