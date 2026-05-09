# Implementation Audit R6: Proposal 075 Local Persistence Write Budget and Evidence Spooling

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Audit report | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R6.md` |
| Audit mode | `auto` |
| Audit timestamp | 2026-05-09T11:51:27+03:00 |
| Implementation target | branch `cw/implement-proposal-075-local-p/4aeb45a9` |
| Audited HEAD | `ed3c891a92b1b6f6127e28b0cf3d5e5599d951eb` |
| Compare base | `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Worktree status before report | Clean on target branch |
| Proposal state | Active / approved implementation artifact |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High |

## Implementation Target

This audit covers the current P075 implementation branch at `ed3c891a92b1b6f6127e28b0cf3d5e5599d951eb`. The latest implementation commit is `ed3c891a Route P075 runtime transactions through DbWriter`.

The branch has advanced since R5 by changing many runtime transaction call sites, expanding `write-operation-registry.toml` from 6 to 69 operations, tightening the P075 gate scan, updating the allowlist wording, and replacing the baseline evidence table with concrete numeric values.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: **Not reused**.

The bundled prior-review discovery helper found no proposal-review artifacts for P075. Existing P075 `IMPLEMENTATION_AUDIT_R*` files were ignored for reviewer selection per the audit workflow.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P075 is primarily Rust persistence, write gateway ownership, DB repository boundaries, and async transaction architecture. |
| `rust_reliability_reviewer` | The proposal commits to deadlines, backpressure, idempotency, shutdown behavior, and recovery-safe writes. |
| `api_contract_reviewer` | GraphQL `storageHealth`, MCP storage tools, typed errors, and capability boundaries are in scope. |
| `observability_rollout_reviewer` | The proposal includes rollout gates, numeric baselines, diagnostics, kill switches, and reference truth. |
| `chainworks_execution_truth_reviewer` | The write budget governs durable Run/Stage/Agent/work item/artifact/evidence truth. |

Rejected close alternatives:

- `rust_security_reviewer`: storage diagnostics authorization is relevant but covered by API contract and explicit auth tests; no new unsafe/secret parser surface dominated this pass.
- `rust_performance_reviewer`: contention and latency are covered through rollout/observability; there is no benchmark target in the proposal.
- `macos_ui_reviewer` / `apple_arch_reviewer`: Swift changes are diagnostics plumbing, not a proposal-mandated macOS UI journey.
- `product_reviewer`: proposal metrics are operational rollout gates, not product adoption or experiment metrics.

## Proposal Contract Summary

P075 requires:

- All non-test runtime write transactions route through `DbWriter` or a source-controlled temporary bypass allowlist with owner and retirement criteria (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`).
- Every `WriteOperation` declares class, lane, operation name, expected row count, batchability, barrier semantics, deadline, idempotency key, and replay policy (`:49`, `:251-255`).
- Non-idempotent Class A barrier mutations using `caller_guarded` provide a duplicate-application test (`:251-252`).
- High-volume raw evidence is spooled to files with compact SQLite metadata (`:50`, `:718-719`).
- Coalesced updates flush on bounded cadence and cannot remain unflushed indefinitely (`:51`, `:706`).
- `storageHealth` and MCP diagnostics expose typed units, freshness, thresholds, kill-switch state, and degraded/unavailable cases (`:510-548`, `:723`).
- Phase baseline metrics are captured and stored for comparison (`:595`).
- The `proposal-075` gate fails on unapproved direct runtime write bypasses or raw high-volume evidence in SQLite (`:411-412`, `:726`).
- Temporary rollout bypass entries are retired by Phase 8 (`:827-828`).

## Scope

Platform/product scope:

- Apple: macOS support code is incidental diagnostics capture/export only.
- Backend/service: Rust daemon, persistence, worker/write queue, GraphQL, MCP, data migration, rollout/telemetry.
- Cross-stack: operator diagnostics and reference truth across Swift, GraphQL, MCP, and local artifacts.

Primary service flows audited:

1. Runtime write entry through the proposed `DbWriter` gateway.
2. Registered operation metadata and duplicate/replay behavior.
3. High-volume failed-stage/transcript evidence spooling and metadata readback.
4. Operator storage diagnostics over GraphQL and MCP.
5. P075 gate enforcement and rollout baseline evidence.

## Proposal Fidelity Inventory

### Matches

- The allowlist no longer blesses `begin_immediate_with_retry` as a permanent runtime write category. It now says non-test runtime writes enter through `DbWriter` and only permanent infrastructure/test/startup repair bypasses remain (`control-plane/crates/db/write-bypass-allowlist.toml:1-67`).
- The baseline evidence now stores concrete values for write-lock p50/p95, busy retry rate, command latency p50/p95, WAL size, and direct-write inventory (`docs/evidence/p075/phase1-baseline.md:25-33`, `:88-113`).
- GraphQL and MCP storage diagnostics remain typed and gate-covered.
- Evidence spooling and producer inventory remain present and gate-covered.
- The canonical P075 gate passed on this audited HEAD.

### Divergences

- The new `begin_registered_immediate_transaction` / `begin_repository_transaction` path validates a `WriteOperation` but then opens a P061 pool transaction directly; it does not submit the work through the bounded `DbWriter` queues (`control-plane/crates/db/src/writer.rs:204-228`).
- `DbWriter::begin_immediate_transaction` also bypasses `submit`; after a shutdown check it delegates to `begin_registered_immediate_transaction` (`control-plane/crates/db/src/writer.rs:880-895`).
- `DbWriter::submit_transaction`, the method that actually submits a full transaction body through `DbWriter::submit`, appears unused in production source.
- Runtime examples such as `runs.insert`, `work_items.enqueue`, executor invoke-claim helpers, and command transactions still open immediate transactions directly rather than entering the live queued writer (`control-plane/crates/db/src/repos/runs.rs:20-30`, `control-plane/crates/db/src/repos/work_items.rs:17-26`, `control-plane/crates/engine/src/executor.rs:768-795`).
- Several DB repository write methods still execute directly on `SqlitePool` or open raw `pool.begin_with("BEGIN IMMEDIATE")` transactions and are called from runtime code (`control-plane/crates/db/src/repos/ideas.rs:10-33`, `control-plane/crates/mcp-server/src/tools/ideas.rs:69-80`, `control-plane/crates/db/src/repos/stages.rs:120-132`, `control-plane/crates/engine/src/orchestrator.rs:837-842`, `control-plane/crates/db/src/repos/artifact_contracts.rs:44-52`, `control-plane/crates/engine/src/orchestrator.rs:1444`).
- Many registry rows use `replay_policy = "caller_guarded"` with generic idempotency text and `duplicate_application_test_path = "scripts/test-gate.sh::proposal-075_operation_registry_enforcement"`, which proves registry presence, not operation-specific duplicate safety (`control-plane/crates/db/write-operation-registry.toml:63-138`).
- The P075 gate still reports "runtime direct SQL scan clean" even though it does not catch raw `pool.begin_with("BEGIN IMMEDIATE")` and does not scan DB repository `.execute(pool)` write helpers (`scripts/test-gate.sh:5618-5648`).

### Ambiguities / Evidence Gaps

- No prior proposal-review reviewer selection was available.
- The audit ran `./scripts/test-gate.sh proposal-075`, not `./scripts/test-gate.sh full`.
- No live daemon soak workload was run; the baseline numeric sample is a gate-backed file canary.
- The implementation introduces "DbWriter-owned registered transaction helpers" in reference docs, but that category is not present in the proposal acceptance text.

## Requirement Summary

| Requirement | Status |
|---|---|
| REQ-001 Route all non-test runtime writes through `DbWriter` or proposal allowlist | Partially Implemented |
| REQ-002 Define per-operation class/lane/idempotency/replay contract | Partially Implemented |
| REQ-003 Implement bounded `DbWriter` lanes, deadlines, results, heartbeat, and shutdown drain | Implemented |
| REQ-004 Keep high-volume evidence in files with compact SQLite metadata and reader states | Implemented |
| REQ-005 Coalesce non-critical updates with bounded flushes | Implemented |
| REQ-006 Adopt current high-volume producer spooling and inventory | Implemented |
| REQ-007 Expose typed GraphQL `storageHealth` | Implemented |
| REQ-008 Expose MCP storage diagnostics with typed errors | Implemented |
| REQ-009 Enforce operator-only storage diagnostics capability boundary | Implemented |
| REQ-010 Capture stored rollout baseline metrics and direct-write inventory | Implemented |
| REQ-011 Include storage diagnostics in diagnostics bundle/reference truth | Implemented |
| REQ-012 Make `proposal-075` gate fail closed on unapproved direct writes/raw SQLite evidence | Partially Implemented |
| REQ-013 Retire temporary bypass entries by Phase 8 | Implemented |

## Detailed Requirement Audit

### REQ-001: Route All Non-Test Runtime Writes Through `DbWriter` Or Proposal Allowlist

- Source: proposal goals and acceptance (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`).
- Status: **Partially Implemented**.
- Evidence types: proposal, code, config, tests-run.
- Implementation mapping: evidence metadata producers use real `DbWriter::submit` helpers; many transaction call sites now carry registered operation names.
- Gap: the common runtime path validates operation metadata and opens a direct transaction outside the queued writer (`control-plane/crates/db/src/writer.rs:204-228`, `:880-895`). Single-statement DB repository writes also remain direct and runtime-callable.

### REQ-002: Define Per-Operation Class/Lane/Idempotency/Replay Contract

- Source: proposal WriteOperation and replay contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:49`, `:251-255`).
- Status: **Partially Implemented**.
- Evidence types: proposal, code, config, tests-run.
- Implementation mapping: `WriteOperation` type validation exists, and `write-operation-registry.toml` now has 69 operation rows.
- Gap: many `caller_guarded` rows use generic idempotency wording and a static gate path as their duplicate-application proof rather than an operation-specific duplicate test. That does not satisfy the proposal's explicit duplicate-application-test requirement for non-idempotent barrier writes.

### REQ-003: Implement Bounded `DbWriter` Lanes, Deadlines, Results, Heartbeat, And Shutdown Drain

- Source: DbWriter contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:136-266`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: `DbWriter` implements bounded MPSC lanes, coalescing, heartbeat snapshots, deadline/result accounting, and shutdown drain. Gate-covered writer tests passed.
- Note: this component exists, but REQ-001 covers the incomplete runtime adoption.

### REQ-004: Keep High-Volume Evidence In Files With Compact SQLite Metadata

- Source: evidence spooling contract and acceptance (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:297-325`, `:718-719`).
- Status: **Implemented**.
- Evidence types: code, migration, tests-run.
- Implementation mapping: evidence spool writer, `evidence_spool_refs`, status enum coverage, checksum/path validation, orphan sweep, and failed-stage evidence producer are present and gate-covered.

### REQ-005: Coalesce Non-Critical Updates With Bounded Flushes

- Source: goals and gate proof (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:51`, `:706`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: Class B coalescing buffer, cadence, saturation behavior, and shutdown flush are implemented and covered by P075 db writer tests.

### REQ-006: Adopt Current High-Volume Producer Spooling And Inventory

- Source: rollout phases and high-volume evidence acceptance (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:824-825`, `:718`).
- Status: **Implemented**.
- Evidence types: code, docs, tests-run.
- Implementation mapping: failed-stage diagnostic packets and ACP transcript metadata are inventoried; classes without current primary SQLite byte producers are documented in `docs/evidence/p075/producer-inventory.md`.

### REQ-007: Expose Typed GraphQL `storageHealth`

- Source: GraphQL contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:510-542`, `:723`).
- Status: **Implemented**.
- Evidence types: schema, code, tests-run.
- Implementation mapping: typed `StorageHealth`, `DbWriterHealth`, WAL, evidence spool, thresholds, and kill switches are exposed and gate-tested.

### REQ-008: Expose MCP Storage Diagnostics With Typed Errors

- Source: MCP contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:544-590`).
- Status: **Implemented**.
- Evidence types: API contract, code, tests-run.
- Implementation mapping: `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, and `storage.reconcile_evidence_orphans` exist with typed error bodies and gate tests.

### REQ-009: Enforce Operator-Only Storage Diagnostics Capability Boundary

- Source: MCP capability and diagnostic sensitivity contract (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:546-548`, `:582-590`).
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: auth and MCP dispatch tests cover observer denial and typed unauthorized responses.

### REQ-010: Capture Stored Rollout Baseline Metrics And Direct-Write Inventory

- Source: baseline requirement (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:595`).
- Status: **Implemented**.
- Evidence types: docs, tests-run.
- Implementation mapping: `docs/evidence/p075/phase1-baseline.md` now records concrete numeric values, capture timestamp, command, workload description, and direct-write inventory (`docs/evidence/p075/phase1-baseline.md:25-33`, `:88-113`).

### REQ-011: Include Storage Diagnostics In Diagnostics Bundle And Reference Truth

- Source: diagnostics export and closeout docs (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:675`, `:696`, `:827-828`).
- Status: **Implemented**.
- Evidence types: code, docs, tests-found.
- Implementation mapping: Swift diagnostics bundle support and docs/reference updates are present.

### REQ-012: Make `proposal-075` Gate Fail Closed On Unapproved Direct Writes

- Source: allowlist gate behavior and acceptance (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:411-412`, `:726`).
- Status: **Partially Implemented**.
- Evidence types: code, tests-run.
- Implementation mapping: the gate validates allowlist shape, operation registry shape, temporary bypass retirement, selected direct transaction tokens, raw-evidence patterns, baseline markers, and producer inventory. It passed on this HEAD.
- Gap: the direct-write scan treats registered helper use as sufficient and misses remaining direct write surfaces: raw `pool.begin_with("BEGIN IMMEDIATE")`, DB repository `.execute(pool)` write helpers, and generic caller-guarded duplicate-test placeholders.

### REQ-013: Retire Temporary Bypass Entries By Phase 8

- Source: Phase 8 and acceptance (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:827-828`, `:715`).
- Status: **Implemented**.
- Evidence types: config, tests-run.
- Implementation mapping: allowlist has five permanent infrastructure entries and zero `temporary_rollout` entries; the P075 gate passed this check.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Runtime write entry is metadata-validated but not actually queued through `DbWriter`. | High |
| Rust architecture | Not Ready | "DbWriter-owned registered transaction helpers" are a new contract category outside proposal acceptance. | High |
| Rust reliability | Not Ready | Direct transactions bypass lane admission, backpressure, queue ordering, shutdown drain, and WriteResult accounting. | High |
| API contract | Ready | GraphQL/MCP storage surfaces and typed errors are gate-covered. | Medium |
| Observability/rollout | Not Ready | Gate can pass while core write-routing enforcement remains incomplete. | High |
| Chainworks execution truth | Not Ready | Run/stage/work item/artifact truth can still mutate through non-queued write paths. | High |

## Routed Specialist Findings

### ARCH-001 / REL-001 / READY-001: Registered Transaction Helpers Do Not Route Runtime Writes Through The Bounded DbWriter

- Reviewer: `rust_arch_reviewer`, `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-003, REQ-012
- Evidence types: proposal, code, config, tests-run
- Evidence references:
  - Proposal requires all non-test runtime writes to route through `DbWriter` or the source-controlled bypass allowlist (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:48`, `:715`).
  - `begin_registered_immediate_transaction` validates an operation and calls `begin_immediate_with_retry` directly (`control-plane/crates/db/src/writer.rs:204-228`).
  - `DbWriter::begin_immediate_transaction` performs only a shutdown admission check before delegating to the same direct helper (`control-plane/crates/db/src/writer.rs:880-895`).
  - `DbWriter::submit_transaction`, the actual queued transaction bridge, is unused in production source.
  - Runtime examples include `runs.insert` and `work_items.enqueue` using the helper (`control-plane/crates/db/src/repos/runs.rs:20-30`, `control-plane/crates/db/src/repos/work_items.rs:17-26`), and executor helpers creating local writers only to call `begin_immediate_transaction` (`control-plane/crates/engine/src/executor.rs:768-795`).
- Why it matters: a validation helper in the writer module is not equivalent to the proposal's bounded single-writer gateway. These transactions bypass queue capacity, lane priority, backpressure, queued deadline accounting, shared heartbeat pressure, Class B/D ordering, and the live writer's shutdown drain.
- Recommended action: route multi-row runtime writes through a shared `DbWriter::submit` / `submit_transaction` path, or explicitly amend the proposal to add and review a separate registered direct-transaction category.
- Acceptance criteria:
  - Production runtime transactions require a shared `DbWriter` instance and submit their transaction body through the writer queue.
  - Creating a fresh local writer solely to call `begin_immediate_transaction` is removed from runtime code.
  - A static or integration test fails if a runtime path opens a registered immediate transaction without traversing `DbWriter::submit`.

### OPS-001 / READY-002: The P075 Gate Can Pass While Direct Runtime Writes Remain

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-012
- Evidence types: code, tests-run
- Evidence references:
  - Gate direct-write regex catches `pool.begin().await`, `begin_immediate_with_retry(`, and some direct `.execute(pool)` calls, but DB repository scanning switches to a transaction-only regex and omits `.execute(pool)` (`scripts/test-gate.sh:5618-5648`).
  - The regex does not catch `pool.begin_with("BEGIN IMMEDIATE")`, which remains in runtime-callable artifact contract methods (`control-plane/crates/db/src/repos/artifact_contracts.rs:44-52`, `:257-264`, `:542-552`).
  - Direct repository writes remain runtime-callable, for example `ideas::insert` used by MCP create-idea (`control-plane/crates/db/src/repos/ideas.rs:10-33`, `control-plane/crates/mcp-server/src/tools/ideas.rs:69-80`) and `stages::update_status` used by the orchestrator (`control-plane/crates/db/src/repos/stages.rs:120-132`, `control-plane/crates/engine/src/orchestrator.rs:837-842`).
  - The gate passed with `runtime direct SQL scan clean`.
- Why it matters: the proposal specifically says the gate must fail on unapproved direct runtime writes. A green P075 gate is currently not strong evidence of that contract.
- Recommended action: extend the gate to reject raw `pool.begin_with("BEGIN IMMEDIATE")`, direct DB repository `.execute(pool)` writes unless allowlisted/test-only, and registered helper use that does not submit through the live writer queue.
- Acceptance criteria:
  - Adding a direct `pool.begin_with("BEGIN IMMEDIATE")` runtime helper fails the gate.
  - Adding a direct repository `.execute(pool)` write helper used by runtime fails unless it is test-only or allowlisted.
  - The gate distinguishes operation-name registration from actual `DbWriter` queue submission.

### REL-002: Caller-Guarded Registry Rows Do Not Provide Operation-Specific Duplicate Tests

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-002
- Evidence types: proposal, config
- Evidence references:
  - Proposal says non-naturally-idempotent Class A barrier mutations using `caller_guarded` must provide a duplicate-application test (`docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md:251-252`).
  - Many registry rows use `replay_policy = "caller_guarded"` with generic idempotency text and `duplicate_application_test_path = "scripts/test-gate.sh::proposal-075_operation_registry_enforcement"` (`control-plane/crates/db/write-operation-registry.toml:63-138` and following rows).
- Why it matters: a static registry-presence check does not prove that replaying `runs.insert`, work-item transitions, artifact claim mutations, or conflict settlement cannot double-apply domain state.
- Recommended action: replace generic duplicate-test placeholders with operation-specific tests or change operations to natural-key/idempotent SQL forms where appropriate.
- Acceptance criteria:
  - Each `caller_guarded` operation links to a concrete test for duplicate submission/replay of that operation.
  - The registry gate rejects generic placeholder duplicate-test paths.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Canonical P075 gate on audited HEAD | Passed | `./scripts/test-gate.sh proposal-075` ended with `==> Proposal 075 gate passed`. |
| Same-tree gate/fingerprint | Passed | Gate ran in target worktree at HEAD `ed3c891a92b1b6f6127e28b0cf3d5e5599d951eb`. |
| Core service flow integration | Partial | DB/engine/GraphQL/MCP tests passed, but runtime write gateway adoption is incomplete. |
| Full regression suite | Not run | Audit ran the canonical proposal gate, not `./scripts/test-gate.sh full`. |
| UI empty/loading/error/accessibility/localization | Out of scope | No P075 UI flow is mandated. |
| Privacy/permissions/entitlements | Low residual risk | Operator-only storage diagnostics are auth-tested. |
| Critical tests executed | Passed | DB writer/spool/ref tests, engine failed-stage evidence, daemon startup sweep, GraphQL storage tests, auth capability test, MCP storage tests, and P075 static checks. |
| Release/handoff readiness | Not Ready | Blocked by ARCH-001/REL-001/READY-001, OPS-001/READY-002, and REL-002. |

## Verification Log

| Command / Inspection | Result |
|---|---|
| `git status --short --branch` in target worktree before report | Clean on `cw/implement-proposal-075-local-p/4aeb45a9`. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` | Chose this R6 report path. |
| Prior-review discovery helper | No prior proposal-review artifacts discovered. |
| `git log --oneline 0f1112b8..HEAD` | New commit: `ed3c891a Route P075 runtime transactions through DbWriter`. |
| `rg --count-matches "begin_registered_immediate_transaction\\(" .../src` | Found registered direct-transaction helper usage across DB repositories, especially work items and runs. |
| `rg --count-matches "pool\\.begin_with\\(\"BEGIN IMMEDIATE\"" .../src` | Found five raw BEGIN IMMEDIATE call sites in `artifact_contracts.rs`. |
| `rg --count-matches "\\.execute\\((?:pool|&pool|&self\\.pool)\\)" .../src` | Found remaining direct pool executes in repository/runtime sources; focused reads confirmed runtime-callable write helpers. |
| `rg -n "submit_transaction\\(" control-plane/crates/{db,engine,daemon,graphql-server,mcp-server}/src` | No production use found. |
| `./scripts/test-gate.sh proposal-075` | Passed. Final static output: `P075 fail-closed registry check passed: 5 bypasses, 69 operations, 66 observed db/src operation literals, 0 temporary rollout bypasses, runtime direct SQL scan clean`; then `==> Proposal 075 gate passed`. |

## Final Verdict

Overall conformance is **Partial**. The branch fixes the R5 baseline evidence and removes the explicit permanent `begin_immediate_with_retry` wording from the allowlist, and the P075 gate passes. However, the current "routed" transaction model still opens direct pool transactions after metadata validation instead of submitting runtime writes through the bounded live `DbWriter` queue. Direct repository write helpers also remain runtime-callable, and the gate can still report clean while those paths exist.

Overall implementation readiness is **Not Ready**. The remaining blockers are proposal-critical because they affect the central single-writer/write-budget guarantee, the idempotency/replay contract, and the gate's ability to prove the implementation.

Recommended next actions:

1. Replace registered direct transaction helpers with actual shared `DbWriter::submit` / `submit_transaction` usage for runtime mutations.
2. Convert or explicitly govern single-statement repository writes that execute directly on `SqlitePool`.
3. Tighten the P075 gate to catch raw `BEGIN IMMEDIATE`, direct repository `.execute(pool)` writes, and non-queued registered helper paths.
4. Replace generic `caller_guarded` duplicate-test placeholders with operation-specific duplicate/replay tests.
5. Rerun `./scripts/test-gate.sh proposal-075` on the updated HEAD.
