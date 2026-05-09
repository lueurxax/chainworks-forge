# Implementation Audit R5: Proposal 075 Local Persistence Write Budget and Evidence Spooling

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit report | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R5.md` |
| Audit mode | `auto` |
| Audit timestamp | 2026-05-09T10:10:21+03:00 |
| Implementation target | branch `cw/implement-proposal-075-local-p/4aeb45a9` |
| Audited HEAD | `0f1112b8fc15f50e60a15b62e8aac7e96c154057` |
| Compare base | `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Worktree status before report | Clean on target branch |
| Proposal state | Active / approved implementation artifact |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High |

## Implementation Target

The audited branch implements P075 across the Rust control plane, GraphQL, MCP, Swift diagnostics capture, reference docs, and the proposal gate. The same-tree canonical proposal gate passed on audited HEAD `0f1112b8fc15f50e60a15b62e8aac7e96c154057`.

Changed surfaces since the compare base include:

- `control-plane/crates/db`: DbWriter, write classes, operation registry, bypass allowlist, evidence spool, migrations, storage health, tests.
- `control-plane/crates/engine`: failed-stage evidence spooling, executor/orchestrator write paths, transcript spooling.
- `control-plane/crates/graphql-server`: typed `storageHealth`.
- `control-plane/crates/mcp-server`: storage diagnostics and typed storage errors.
- `Chainworks Forge/Support` and tests: diagnostics bundle storage snapshots.
- `docs/evidence/p075`, `docs/reference`, `README.md`, and `scripts/test-gate.sh`: closeout evidence and gate contract.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: **Not reused**.

No prior proposal-review artifacts were found for this proposal via the adjacent review directory/sibling artifact discovery path or the bundled discovery helper. Existing P075 `IMPLEMENTATION_AUDIT_R*` files were intentionally ignored for reviewer selection per the audit skill.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P075 is primarily Rust persistence, async write ownership, DB module boundaries, and crate-level architecture. |
| `rust_reliability_reviewer` | The proposal commits to queueing, deadlines, backpressure, idempotency, shutdown drain, retry classification, and recovery. |
| `api_contract_reviewer` | The implementation changes GraphQL `storageHealth`, MCP storage tools, typed errors, auth capabilities, and diagnostics payloads. |
| `observability_rollout_reviewer` | The implementation changes test gates, telemetry baselines, kill switches, rollout evidence, and reference truth. |
| `chainworks_execution_truth_reviewer` | The write budget touches durable Run/Stage/Agent/work item/artifact/evidence truth and recovery semantics. |

Rejected close alternatives:

- `rust_security_reviewer`: storage diagnostics authorization is relevant, but the inspected auth boundary is covered by API contract and proposal-gate tests; no new unsafe/secret/public parser surface dominated the audit.
- `rust_performance_reviewer`: performance is relevant through contention metrics, but no benchmark target or throughput claim was central beyond the observability baseline requirement.
- `macos_ui_reviewer` / `apple_arch_reviewer`: Swift changes are diagnostics plumbing and bundle export support, not a proposal-mandated macOS UI flow.
- `product_reviewer`: proposal metrics are rollout/operational metrics, not product adoption or experiment decision metrics.

## Proposal Contract Summary

P075 commits to a Rust local-persistence write discipline for the Chainworks control plane:

- Route all non-test runtime writes through `DbWriter` or a source-controlled temporary bypass allowlist with owner and retirement criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`).
- Require each `WriteOperation` to declare class, lane, operation name, expected rows, batchability, barrier semantics, deadline, idempotency key, and replay policy (`:49`).
- Keep high-volume evidence bytes in local files and SQLite as compact metadata pointers only (`:50`, `:718`).
- Coalesce non-critical projection/status updates with bounded flushes (`:51`).
- Expose typed storage pressure through GraphQL `storageHealth`, MCP storage diagnostics, structured logs, and proposal gate evidence (`:52`, `:510-548`, `:723`).
- Preserve short, bounded SQLite transactions free of filesystem, provider, network, and ACP waits (`:53`).
- Capture and store baseline metrics before rollout phase comparison: write-lock p50/p95, busy retry rate, command latency p50/p95, WAL size, and direct write inventory under a canned workload (`:595`).
- Make the `proposal-075` gate fail closed on unapproved direct write bypasses and raw high-volume evidence persisted to SQLite (`:703-711`, `:726`).
- Retire temporary bypass allowlist entries by Phase 8 and publish durable reference truth (`:827-828`).

Explicit non-goals include P078 side-effect ledger/settlement/reconciliation semantics (`:57-59`, `:299`).

## Scope

Platform/product scope:

- Apple: macOS client support is incidental diagnostics capture/export only.
- Backend/service: Rust daemon, persistence, worker/write queue, GraphQL, MCP, data migrations, rollout/telemetry.
- Cross-stack scope: diagnostics payloads and operator readbacks across GraphQL/MCP/Swift bundle export.

Primary service flows audited:

1. Runtime write submission through `DbWriter`, admission, lane/deadline handling, and shutdown drain.
2. High-volume evidence producer writes file bytes first, stores one compact `EvidenceSpoolRef`, and survives orphan recovery.
3. Operator reads storage pressure through GraphQL `storageHealth` and MCP storage diagnostics with typed error behavior.
4. Maintenance reconciles evidence orphans through bounded, scoped MCP dry-run/non-dry-run paths.
5. The P075 gate proves write registry, allowlist, evidence-spooling, diagnostics, and closeout evidence on the audited tree.

## Proposal Fidelity Inventory

### Matches

- `WriteClass`, `WriteLane`, `WriteOperation`, `ReplayPolicy`, and `WriteResult` are implemented with validation for class/lane compatibility, barrier requirements, replay-policy compatibility, deadline caps, and idempotency-key bounds (`control-plane/crates/db/src/write_class.rs:1-330`).
- `DbWriter` implements bounded lanes, coalescing, deadline/result accounting, heartbeat, shutdown drain, and structured logs (`control-plane/crates/db/src/writer.rs`).
- Evidence spooling writes files with checksum/fsync/no-clobber semantics and metadata through `EvidenceSpoolRef` paths (`control-plane/crates/db/src/evidence_spool.rs`, `control-plane/crates/db/src/repos/evidence_spool_refs.rs`).
- Failed-stage evidence now writes a spooled file and compact pointer (`control-plane/crates/engine/src/evidence.rs:144-177`).
- GraphQL exposes typed `storageHealth` rather than an opaque JSON surface (`control-plane/crates/graphql-server/src/types/storage.rs:56-153`, `control-plane/crates/graphql-server/src/schema.rs:563-576`).
- MCP storage tools return typed storage error bodies for domain-level failures (`control-plane/crates/mcp-server/src/tools/storage.rs:24-41`).
- Phase 8 temporary rollout bypasses are retired from the allowlist; the current file has five permanent infrastructure entries and no `temporary_rollout` rows (`control-plane/crates/db/write-bypass-allowlist.toml:18-66`).
- `./scripts/test-gate.sh proposal-075` passed on the audited HEAD.

### Divergences

- The implementation and reference docs introduce an extra acceptable path, `begin_immediate_with_retry`, for non-test runtime writes. The proposal contract allows `DbWriter` or a source-controlled temporary bypass allowlist, not a third permanent bounded-transaction category (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`; `control-plane/crates/db/write-bypass-allowlist.toml:3-5`; `docs/reference/rust-control-plane.md:363`, `:368`, `:420`).
- Many runtime write owners still open direct `begin_immediate_with_retry` transactions outside `DbWriter` and outside the bypass allowlist. Examples include `command.StartRun` (`control-plane/crates/engine/src/command_handler.rs:1715-1717`), executor work-item transitions (`control-plane/crates/engine/src/executor.rs:768-781`), `runs.insert` (`control-plane/crates/db/src/repos/runs.rs:20-23`), and `work_items.enqueue` (`control-plane/crates/db/src/repos/work_items.rs:16-21`).
- The gate's runtime direct-write scan does not diff all direct write call sites against the allowlist as proposed. It scans selected runtime crates for `pool.begin().await` and `.execute(pool|&pool|&self.pool)`, but it does not reject `begin_immediate_with_retry` direct write owners and it does not inspect `control-plane/crates/db/src/repos` direct transaction owners as bypasses (`scripts/test-gate.sh:5596-5632`).
- The baseline evidence file records "non-null live sample" assertions rather than concrete stored p50/p95 values and says `command_latency_p50/p95` is represented by `transactionDurationP95Ms`, so the required command latency p50 and comparable numeric baseline are incomplete (`docs/evidence/p075/phase1-baseline.md:25-33`).
- The same baseline table still says "36 allowlisted bypass entries" while the Phase 8 inventory below it and the gate output say five bypass entries (`docs/evidence/p075/phase1-baseline.md:33`, `:70-80`).

### Ambiguities / Evidence Gaps

- No prior proposal-review reviewer selection was available to reuse.
- The audit ran the canonical P075 gate, not the entire repository `full` gate.
- Swift diagnostics bundle support was inspected through code/tests-found, but no macOS UI runtime validation was run because P075 does not mandate a UI flow.
- Broader daemon soak evidence for queue depth, orphan count, and command latency below thresholds was not found; the current evidence is a gate-backed file canary plus static documents.

## Requirement Summary

| Requirement | Status |
|---|---|
| REQ-001 Route all non-test runtime writes through `DbWriter` or proposal allowlist | Partially Implemented |
| REQ-002 Define and validate full `WriteOperation` contract | Implemented |
| REQ-003 Implement bounded `DbWriter` lanes, deadlines, results, heartbeat, and shutdown drain | Implemented |
| REQ-004 Keep high-volume evidence in files with compact SQLite metadata and reader states | Implemented |
| REQ-005 Coalesce non-critical updates with bounded flushes | Implemented |
| REQ-006 Adopt high-volume producer spooling and inventory current producer truth | Implemented |
| REQ-007 Expose typed GraphQL `storageHealth` with units, freshness, thresholds, kill switches, WAL behavior | Implemented |
| REQ-008 Expose MCP storage diagnostics with GraphQL parity and typed errors | Implemented |
| REQ-009 Enforce operator-only storage diagnostics capability boundary | Implemented |
| REQ-010 Capture stored rollout baseline metrics and direct-write inventory | Partially Implemented |
| REQ-011 Include storage diagnostics in diagnostics bundle/reference truth | Implemented |
| REQ-012 Make `proposal-075` gate fail closed on unapproved direct writes and raw SQLite evidence | Partially Implemented |
| REQ-013 Retire temporary bypass entries by Phase 8 | Implemented |

## Detailed Requirement Audit

### REQ-001: Route All Non-Test Runtime Writes Through `DbWriter` Or Proposal Allowlist

- Source: Goals and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`); allowlist gate behavior (`:409-412`).
- Status: **Partially Implemented**.
- Evidence types: proposal, code, config, tests-run.
- Implementation mapping: `DbWriter` exists and some P075 metadata writes use `*_via_dbwriter` helpers, especially evidence-spool metadata (`control-plane/crates/db/src/repos/evidence_spool_refs.rs:868-981`).
- Gap: runtime write owners still use direct `begin_immediate_with_retry` transactions outside `DbWriter` and outside a proposal-recognized temporary bypass allowlist, for example `command.StartRun`, executor work-item transitions, `runs.insert`, and `work_items.enqueue`. The implementation describes this as an acceptable permanent bounded transaction boundary, but the proposal does not.

### REQ-002: Define And Validate The Full `WriteOperation` Contract

- Source: Goals (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:49`) and idempotency/replay contract (`:251-254`).
- Status: **Implemented**.
- Evidence types: code, config, tests-run.
- Implementation mapping: `WriteOperation` has class, lane, operation name, expected rows, batchability, barrier, deadline, idempotency key, replay policy, and observation ordering (`control-plane/crates/db/src/write_class.rs:148-185`). Runtime validation rejects class/lane mismatch, missing Class A barrier, replay-policy mismatch, too-long or control-character idempotency keys, and unannotated long Class A deadlines (`control-plane/crates/db/src/write_class.rs:187-293`). The registry and gate validate operation metadata (`control-plane/crates/db/write-operation-registry.toml`, `scripts/test-gate.sh:5576-5608`).

### REQ-003: Implement Bounded `DbWriter` Lanes, Deadlines, Results, Heartbeat, And Shutdown Drain

- Source: DbWriter contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:136-266`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: `DbWriter` implements six priority lanes, bounded channels, Class B coalescing, deadline/result accounting, heartbeat snapshots, lane starvation, and shutdown drain. Storage health consumes live heartbeat fields (`control-plane/crates/db/src/repos/storage_health.rs:131-178`).
- Verification: P075 gate ran db writer tests including lane drain order, saturation, coalescing, shutdown admission, heartbeat, live storage health readback, and file-backed lock/WAL canary.

### REQ-004: Keep High-Volume Evidence In Files With Compact SQLite Metadata And Reader States

- Source: Evidence spooling layout, statuses, orphan recovery, and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:297-325`, `:457-494`, `:718-719`).
- Status: **Implemented**.
- Evidence types: code, migration, tests-run.
- Implementation mapping: evidence spool file writer validates canonical layout, writes/checksums/fsyncs files, supports orphan sweep and recovered metadata (`control-plane/crates/db/src/evidence_spool.rs:128-550`). `EvidenceSpoolRefStatus` includes `available`, `legacy_absent`, `missing_file`, `checksum_mismatch`, `recovered_orphan`, and `pending_delete` (`control-plane/crates/db/src/repos/evidence_spool_refs.rs:88-124`). Migrations add `evidence_spool_refs` and constraints.

### REQ-005: Coalesce Non-Critical Updates With Bounded Flushes

- Source: Goals and Class B/coalescing contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:51`, `:190-230`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: `DbWriter` Class B coalescing buffer is keyed by idempotency key and flushes on bounded cadence/shutdown with saturation rejection and counters. Gate-covered tests include `coalesced_projection_invalidation_merges_and_flushes_500ms_64` and coalescing buffer saturation coverage.

### REQ-006: Adopt High-Volume Producer Spooling And Inventory Current Producer Truth

- Source: Phases 4 and 5 (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:824-825`) and high-volume evidence acceptance (`:718`).
- Status: **Implemented**.
- Evidence types: code, docs, tests-run.
- Implementation mapping: failed-stage diagnostic packets spool the full payload and store a compact v2 pointer; ACP transcript capture is inventoried behind `CHAINWORKS_PERSIST_ACP_TRANSCRIPTS=1`; tool trace/stdout/stderr/model-delta classes are reserved and documented as having no current primary SQLite byte producers (`docs/evidence/p075/producer-inventory.md:11-29`).
- Note: broader soak metrics are evaluated under REQ-010, not this code-adoption requirement.

### REQ-007: Expose Typed GraphQL `storageHealth`

- Source: GraphQL contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:510-542`, `:693`, `:723`).
- Status: **Implemented**.
- Evidence types: schema, code, tests-run.
- Implementation mapping: typed GraphQL storage types include `StorageHealth`, `DbWriterHealth`, `WalHealth`, `EvidenceSpoolSummary`, kill switches, thresholds, units, freshness fields, and fail-closed stale/degraded behavior (`control-plane/crates/graphql-server/src/types/storage.rs:56-153`; `control-plane/crates/db/src/repos/storage_health.rs:131-178`).
- Verification: P075 gate ran typed and live `storageHealth` tests.

### REQ-008: Expose MCP Storage Diagnostics With Typed Errors

- Source: MCP contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:544-590`, `:694`).
- Status: **Implemented**.
- Evidence types: API contract, code, tests-run.
- Implementation mapping: MCP storage tools include `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, and `storage.reconcile_evidence_orphans` (`control-plane/crates/mcp-server/src/tools/storage.rs:44-118`). Domain-level failures return typed error bodies with `error`, `errorCode`, `message`, and `tool` (`control-plane/crates/mcp-server/src/tools/storage.rs:24-41`).
- Verification: P075 gate ran MCP parameter semantics, stale, invalid input, maintenance-disabled, unavailable, and unauthorized typed-error tests.

### REQ-009: Enforce Operator-Only Storage Diagnostics Capability Boundary

- Source: MCP capability ids and storage diagnostic sensitivity (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:546-548`, `:582-590`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: storage tool dispatch returns typed `unauthorized` for callers lacking storage capability, and tests cover both unauthorized and maintenance-disabled paths.
- Verification: P075 gate ran `proposal_075_storage_tool_dispatch_returns_typed_unauthorized` and auth capability boundary tests.

### REQ-010: Capture Stored Rollout Baseline Metrics And Direct-Write Inventory

- Source: Telemetry baseline requirement (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:595`).
- Status: **Partially Implemented**.
- Evidence types: docs, tests-run.
- Implementation mapping: `docs/evidence/p075/phase1-baseline.md` exists and references a file-backed live storage canary plus gate enforcement (`docs/evidence/p075/phase1-baseline.md:20-33`). The P075 gate rejects the old `pending_live_canary` marker and checks required baseline markers.
- Gap: the stored baseline does not contain concrete captured p50/p95 values for write-lock wait and command latency, does not capture `command_latency_p50`, and says command p50/p95 are represented by `transactionDurationP95Ms` (`docs/evidence/p075/phase1-baseline.md:27-31`). That is not enough to compare each rollout phase against a stored baseline. The direct-write inventory row also contains a stale "36 allowlisted bypass entries" value while the Phase 8 inventory and gate say five (`docs/evidence/p075/phase1-baseline.md:33`, `:70-80`).

### REQ-011: Include Storage Diagnostics In Diagnostics Bundle And Reference Truth

- Source: Diagnostics export and docs/reference closeout (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:675`, `:696`, `:827-828`).
- Status: **Implemented**.
- Evidence types: code, docs, tests-found.
- Implementation mapping: Swift diagnostics bundle inputs and bundle writer include optional `storage_health.json` and `evidence_spool_summary.json` (`Chainworks Forge/Support/DiagnosticsBundle.swift:42-46`, `:90-94`, `:252-258`). Tests cover bundle inclusion when supplied (`Chainworks ForgeTests/DiagnosticsBundleTests.swift:264-285`). Reference docs and README include P075 storage truth.

### REQ-012: Make `proposal-075` Gate Fail Closed On Unapproved Direct Writes And Raw SQLite Evidence

- Source: Gate behavior and acceptance criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:411-412`, `:703-711`, `:726`).
- Status: **Partially Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: the gate validates allowlist shape, rejects `temporary_rollout`, validates operation registry, runs DB/engine/GraphQL/MCP/auth tests, checks raw failed-stage evidence patterns, requires baseline and producer inventory documents, and passed on the audited HEAD.
- Gap: the gate does not implement the proposal's "diffs direct write call sites against the allowlist" behavior for all direct runtime write owners. It permits direct `begin_immediate_with_retry` as a non-allowlisted path and omits DB repository direct transaction owners from its runtime direct-write scan (`scripts/test-gate.sh:5596-5632`).

### REQ-013: Retire Temporary Bypass Entries By Phase 8

- Source: Phase 8 (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:827-828`) and acceptance (`:715`).
- Status: **Implemented**.
- Evidence types: config, tests-run.
- Implementation mapping: allowlist rows are limited to migrations, tests, startup repair, and evidence-spool orphan repair; no row has `scope = "temporary_rollout"` (`control-plane/crates/db/write-bypass-allowlist.toml:18-66`). Gate output confirmed "5 bypasses ... 0 temporary rollout bypasses."

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Runtime direct writes are still outside the proposal's `DbWriter`/temporary-allowlist contract. | High |
| Rust architecture | Not Ready | A third write ownership category has become repository truth without proposal coverage. | High |
| Rust reliability | Ready with major risk | Direct transaction owners bypass DbWriter lane admission, deadline/result classification, and shutdown-drain semantics. | High |
| API contract | Ready with minor risk | GraphQL/MCP typed surfaces and errors are covered by gate tests. | Medium |
| Observability/rollout | Not Ready | Stored baseline evidence is not numeric/comparable enough for phase comparison. | High |
| Chainworks execution truth | Not Ready | Run/work item truth still has direct write owners not governed by the P075 write gateway contract. | High |

## Routed Specialist Findings

### ARCH-001 / READY-001: Permanent `begin_immediate_with_retry` Runtime Writes Bypass The Proposal's Write Ownership Contract

- Reviewer: `rust_arch_reviewer`, `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-012
- Evidence types: proposal, code, config, docs, tests-run
- Evidence references:
  - Proposal says all non-test runtime writes route through `DbWriter` or source-controlled temporary bypass allowlist (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`).
  - The allowlist header now says runtime writes may use `DbWriter` or `begin_immediate_with_retry` (`control-plane/crates/db/write-bypass-allowlist.toml:3-5`).
  - Reference truth also documents `begin_immediate_with_retry` as an accepted production path (`docs/reference/rust-control-plane.md:363`, `:368`, `:420`).
  - Direct examples remain in `command.StartRun` (`control-plane/crates/engine/src/command_handler.rs:1715-1717`), executor work-item transitions (`control-plane/crates/engine/src/executor.rs:768-781`), `runs.insert` (`control-plane/crates/db/src/repos/runs.rs:20-23`), and `work_items.enqueue` (`control-plane/crates/db/src/repos/work_items.rs:16-21`).
- Why it matters: P075's central safety property is that runtime write ownership is auditable through `DbWriter` metadata or an explicit, retiring allowlist. Direct bounded transactions may be short and P061-safe, but they bypass P075 lane admission, operation metadata, replay policy, shutdown admission, WriteResult classification, and source-controlled retirement accounting.
- Recommended action: Either route the remaining runtime write owners through `DbWriter` or add explicit proposal-approved allowlist entries with owner/reason/scope/allowed context/retirement criteria. If the intended architecture is "DbWriter or bounded transaction boundary," amend the proposal/reference contract and rerun review against that changed contract.
- Acceptance criteria:
  - Non-test runtime direct write owners are either submitted as registered `WriteOperation`s through `DbWriter` or appear in the source-controlled bypass allowlist under a proposal-approved scope.
  - The allowlist/reference docs no longer imply an unreviewed permanent third category.
  - The P075 gate fails on an added unallowlisted `begin_immediate_with_retry` runtime owner.

### OPS-001 / READY-002: Baseline Evidence Is Not A Stored Numeric Rollout Baseline

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-010
- Evidence types: proposal, docs, tests-run
- Evidence references:
  - Proposal requires current write-lock p50/p95, busy retry rate, command latency p50/p95, WAL size, and direct write inventory to be captured and stored for rollout-phase comparison (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:595`).
  - Baseline table stores "non-null live sample, gate-enforced" for write-lock metrics and WAL, records expected uncontended retry rate, and maps command latency p50/p95 to only `transactionDurationP95Ms` (`docs/evidence/p075/phase1-baseline.md:25-33`).
  - Direct-write inventory line says "36 allowlisted bypass entries" while the same document's Phase 8 inventory says five bypass entries (`docs/evidence/p075/phase1-baseline.md:33`, `:70-80`).
- Why it matters: The gate proves live fields are non-null, but it does not leave a comparable baseline artifact. Without concrete captured values for p50/p95 and command latency, later rollout phases cannot objectively compare drift against the Phase 1 baseline the proposal requires.
- Recommended action: Store the actual numeric baseline snapshot from a file-backed canned workload, including write-lock wait p50/p95, busy retry rate, command latency p50/p95 or clearly named replacement metrics, WAL size, and direct-write inventory. Update the gate to require concrete numeric values and consistent inventory counts.
- Acceptance criteria:
  - Baseline artifact contains concrete values, units, capture timestamp, workload description, commit, and command used.
  - Command latency p50 and p95 are captured or the proposal is amended to replace them with `transactionDurationP95Ms`.
  - Direct-write inventory count agrees with the allowlist and gate output.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Canonical P075 gate on audited HEAD | Passed | `./scripts/test-gate.sh proposal-075` ended with `==> Proposal 075 gate passed`. |
| Same-tree gate/fingerprint | Passed | Gate was run in target worktree at HEAD `0f1112b8fc15f50e60a15b62e8aac7e96c154057`. |
| Core service flow integration | Partial | DB/engine/GraphQL/MCP gate tests passed; direct write-routing flow remains contract-incomplete. |
| Full regression suite | Not run | Audit ran canonical proposal gate, not `./scripts/test-gate.sh full`. |
| UI empty/loading/error/accessibility/localization | Out of scope | P075 has no proposal-mandated UI behavior; Swift diagnostics bundle path inspected only as support surface. |
| Privacy/permissions/entitlements | Low residual risk | Storage diagnostics are operator-only and typed unauthorized path is tested; no entitlement changes inspected. |
| Critical tests executed | Passed | DB writer/spool/ref tests, engine failed-stage evidence test, daemon startup sweep test, GraphQL storage tests, auth capability test, MCP storage tests, and P075 registry/static checks. |
| Release/handoff readiness | Not Ready | Blocked by ARCH-001/READY-001 and OPS-001/READY-002. |

## Verification Log

| Command / Inspection | Result |
|---|---|
| `git status --short --branch` in target worktree before report | Clean on `cw/implement-proposal-075-local-p/4aeb45a9`. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` | Chose this R5 report path. |
| Prior-review discovery helper | No prior proposal-review artifacts discovered. |
| `git diff --name-status 70b03d1...HEAD` | Confirmed Rust DB/engine/GraphQL/MCP, Swift diagnostics, docs, evidence, and gate surfaces changed. |
| `./scripts/test-gate.sh proposal-075` | Passed. Final static output: `P075 fail-closed registry check passed: 5 bypasses, 6 operations, 3 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`; then `==> Proposal 075 gate passed`. |
| Direct write scan and focused file reads | Found runtime examples of direct `begin_immediate_with_retry` ownership outside `DbWriter`/proposal allowlist. |
| Baseline evidence read | Found gate-backed non-null samples but not stored numeric p50/p95 baseline values; found stale direct-write count. |

## Final Verdict

Overall conformance is **Partial**. The branch implements most P075 infrastructure and the same-tree P075 gate passes, including DbWriter primitives, evidence spooling, typed GraphQL/MCP storage diagnostics, typed errors, operator capability enforcement, producer inventory, and retirement of temporary rollout bypass rows.

Overall implementation readiness is **Not Ready** because the implementation has redefined the central write-routing contract to allow permanent direct `begin_immediate_with_retry` runtime writes, while the proposal requires `DbWriter` or a source-controlled temporary bypass allowlist. The rollout baseline evidence is also not yet a stored numeric baseline suitable for phase comparison.

Recommended next actions:

1. Resolve ARCH-001 by routing remaining runtime write owners through `DbWriter` or explicitly allowlisting proposal-approved temporary bypasses with retirement criteria.
2. Tighten the P075 gate so unapproved `begin_immediate_with_retry` runtime owners fail, including DB repository owners when they are runtime write boundaries.
3. Replace the P075 baseline with concrete numeric captured values and fix the stale bypass inventory count.
4. Rerun `./scripts/test-gate.sh proposal-075` on the updated HEAD.
