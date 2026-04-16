# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/047-session-reuse-and-cancellation-settlement.md |
| Repository Root | . |
| Git SHA | db7d51aa91f71f898c4e621c01523708ca7d3c1b |
| Working Tree | dirty (181436 entries; dominated by existing build/output artifacts and in-flight control-plane changes) |
| Audited At | 2026-04-15T23:43:06+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Current `HEAD` implements most of P047’s concrete control-plane surface: canonical lineage storage, live ACP session ownership, execution-side provenance persistence, two-phase cancellation settlement, the single-run versus list-reader cancellation split, and the canonical `proposal-047` gate definition are all present and directly evidenced. P047 is still only partially implemented because two proposal-owned control points remain under-ported: the runtime does not yet produce the proposal’s stable `owner_execution_lineage_id` / recovery-branch-safe owner chain, and the live budget path still makes economics decisions from synthesized estimates because ACP execution does not return real cost telemetry.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Canonical owner-lineage and live economics contracts are still incomplete | High |
| Architecture | At Risk | `same_invocation_owner` is wired from a per-execution UUID instead of a stable execution-lineage producer | High |
| Product | At Risk | Cost-driven reuse invalidation cannot fire truthfully because runtime cost telemetry is absent | High |
| UI | Not Applicable | Proposal keeps direct session-inspector UI out of scope | High |
| UX | Not Applicable | No direct interactive UX contract beyond truthful northbound reader payloads | High |
| Readiness | Not Ready | Core conformance is still partial on canonical reuse-control paths, so the full proof gate was not rerun | High |

## Proposal Contract

### Scope
- Durable session lineage with immutable generations, append-only events, explicit owner/fingerprint identity, and transport-backed reuse.
- Generation-scoped context-budget evaluation driven by hard guardrails plus economics.
- Two-phase cancellation settlement with durable evidence and northbound reader exposure split by canonical single-run reads vs projection-backed list reads.

### Locked Decisions
- `InvocationOwnerKey` is the explicit tuple `{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}` and remains immutable on each generation.
- Binding fingerprints are built from the full resolved binding contract, including prompt, worktree mode, MCP inventory, permissions, and output contract fields.
- Live reuse is transport-backed through `AcpRuntimeManager`, not string-backed by a stored provider session ID alone.
- Cancellation remains two-phase: `Cancelling` during settlement, `Cancelled` only after session-close outcomes are recorded.
- Single-run northbound reads expose the full cancellation log; list reads expose summary only.

### Primary User Flows
- Reuse a live ACP session safely across loop turns when lineage, binding, and runtime-handle checks all agree.
- End or reset a generation, then resume or start a fresh generation with durable provenance explaining why.
- Cancel a running run and read truthful `cancelling -> cancelled` settlement evidence through canonical run readers.

### UI Commitments
- No direct UI surface is in scope for session inspection.
- The only northbound presentation commitments are payload-shape commitments for GraphQL/MCP readers:
  - single-run readers get `cancellation_settlement_log`
  - list readers get only `cancellation_settlement_summary`

### UX Commitments
- Reuse and cancellation truth must fail closed rather than silently guessing.
- Operator-facing readers should be able to explain what happened from durable provenance instead of reconstructing it heuristically.

### Acceptance Criteria
- Canonical lineage tables and execution provenance are populated correctly.
- Loop reuse, reset, resume, invalidation, and missing-live-handle cases settle into explicit dispositions.
- Budget decisions compact or invalidate reused generations according to persisted hard/economic signals.
- Cancellation settlement cleans up active executions/work items, records session-close outcomes, and exposes full-log vs summary readers through the correct northbound paths.

### Test / Evidence Requirements
- The repo defines the canonical `proposal-047|p047` proof gate in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
- The slice is intended to be validated through the control-plane Rust workspace, not iOS/macOS simulator gates.

### Explicit Exclusions
- Session checkpoint serialization format.
- Provider-specific budget tuning.
- Session inspector UI.

## Proposal Fidelity / Divergence

### Matches
- The repo now carries the canonical `session_lineages` / `session_generations` / `session_events` migration plus the execution-provenance columns promised by P047.
- `AcpRuntimeManager` owns live session handles keyed by generation ID and the executor reuses them only after explicit policy evaluation.
- Cancellation settlement is execution-first, two-phase, and exposed as canonical full-log reads versus projection-backed summary reads.
- Recovery/report readers can consume reuse/reset provenance directly from `agent_executions`.
- The canonical `proposal-047` gate is now defined in both the gate docs and the runner.

### Divergences
- The runtime still seeds `owner_execution_lineage_id` from a fresh `AgentExecutionId` per invoke and `SessionPolicyInput` has no explicit recovery-branch input, so the proposal’s stable owner-lineage / retry-drift contract is only partially implemented.
- The budget evaluator exists, but live economics are still synthesized from prompt-size and cache-share estimates because ACP prompt execution returns no real `cost_cents`.

### Ambiguities / Evidence Gaps
- Because objective conformance is already partial, this audit did not rerun the full `./scripts/test-gate.sh proposal-047` gate on the same tree.
- The proposal is a control-plane slice, so Apple-platform UI and UX review dimensions are mostly out of scope and intentionally thin here.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical lineage migration, immutable generations, and append-only events exist in the Rust control plane
- Proposal Source: `§1a`, `§2a`, AC1-6, AC11-12 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:16-32`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:74-141`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:440-451`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1`
  - `control-plane/crates/domain/src/session.rs:4`
  - `control-plane/crates/db/src/repos/sessions.rs:7`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact` (covered by the current db integration suite)
- Gap / Note: The legacy-table rename, canonical tables, and append-only event persistence are present as proposed.

### REQ-002 Invocation-owner identity and binding identity follow the proposal’s explicit owner chain and fail-closed reuse contract
- Proposal Source: `§2b-§2c`, AC5, AC8 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:142-166`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:195-210`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:444-447`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:146-153`
  - `control-plane/crates/engine/src/session/fingerprint.rs:4`
  - `control-plane/crates/engine/src/executor.rs:343`
  - `control-plane/crates/engine/src/session/policy.rs:13`
  - `control-plane/crates/domain/src/agent.rs:40`
  - `control-plane/crates/engine/src/session/policy.rs:640`
- Gap / Note: The explicit tuple string and full binding fingerprint are now persisted rather than hashed, but the runtime still manufactures `owner_execution_lineage_id` from a fresh `AgentExecutionId` (`executor.rs:343-361`) and `SessionPolicyInput` carries no distinct recovery-branch input. That means the stable owner-lineage / retry-drift bridge promised by `§2b-§2c` is still not fully wired even though the tuple syntax exists.

### REQ-003 Live ACP session reuse, reset, resume, and missing-live-handle fail-closed behavior are implemented through the runtime manager
- Proposal Source: `§1a`, `§2c-§2d`, AC1-10 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:16-32`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:168-249`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:440-449`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:16`
  - `control-plane/crates/engine/src/session/policy.rs:36`
  - `control-plane/crates/engine/src/executor.rs:388`
  - `control-plane/crates/engine/src/command_handler.rs:404`
  - `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact` (passed)
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
  - `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact` (passed)
- Gap / Note: Current `HEAD` now covers the family-scope live reuse path, checkpoint-backed resume generation creation, reset handling, and missing-live-handle fail-closed invalidation.

### REQ-004 Execution-side session provenance is persisted on `agent_executions` and reader code can answer “what happened” without lineage joins
- Proposal Source: `§2d-ii`, AC13-14 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:453-457`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/agent.rs:40`
  - `control-plane/crates/db/src/repos/agent_executions.rs:8`
  - `control-plane/crates/engine/src/executor.rs:455`
  - `control-plane/crates/engine/src/recovery.rs:182`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:64`
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact` (passed)
  - `cargo test -p mcp-server tools::reports::tests::reports_get_decodes_validation_failure_payload -- --exact` (passed)
- Gap / Note: Execution provenance is now written before ACP execution and consumed directly by recovery/report readers, which matches the proposal’s execution-first ownership model.

### REQ-005 Context-budget evaluation is generation-scoped and economics-driven in the live runtime path
- Proposal Source: `§1b`, `§2e`, AC15-20 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:277-330`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:458-465`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/session.rs:49`
  - `control-plane/crates/db/migrations/007_session_budget_signals.sql:1`
  - `control-plane/crates/db/migrations/008_session_runtime_usage.sql:1`
  - `control-plane/crates/engine/src/session/budget.rs:1`
  - `control-plane/crates/engine/src/session/policy.rs:262`
  - `control-plane/crates/engine/src/executor.rs:515`
  - `control-plane/crates/acp/src/session.rs:44`
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
- Gap / Note: Persisted generation counters and usage snapshots now feed the evaluator, but the live economics half of the contract is still under-ported. `budget_signals_from_generation()` synthesizes `normalized_savings_versus_fresh` from prompt-size estimates (`policy.rs:266-300`), `ExecutionResult.cost_cents` remains `None` in ACP session prompting (`acp/src/session.rs:44-55`), and the executor therefore persists zero incremental cost on every turn (`executor.rs:560-575`). The hard-guardrail path exists; the proposal’s runtime economics path does not yet have real telemetry.

### REQ-006 Cancellation settlement follows the promised two-phase contract and cleans up active execution/work-item truth before final cancel
- Proposal Source: `§1c`, `§2f`, AC21-23, AC26-27 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:53-68`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:332-397`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:466-474`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/src/work_item.rs:48`
  - `control-plane/crates/engine/src/cancellation.rs:22`
  - `control-plane/crates/engine/src/command_handler.rs:377`
  - `control-plane/crates/db/src/repos/runs.rs:157`
  - `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact` (passed)
- Gap / Note: The current implementation cancels running executions and work items in Phase 1, keeps the run in `Cancelling`, then records close outcomes and finalizes `Cancelled` in Phase 2.

### REQ-007 Single-run northbound readers expose the full cancellation log while list readers expose only the projection summary
- Proposal Source: `§3` reader wiring, AC24-25 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:471-472`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:26`
  - `control-plane/crates/graphql-server/src/schema.rs:52`
  - `control-plane/crates/graphql-server/src/types/run.rs:5`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:124`
  - `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact` (passed)
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact` (passed)
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact` (passed)
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact` (passed)
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact` (passed)
- Gap / Note: `QueryRoot.run(id)` is canonical and log-bearing, while list queries and `runs.list` remain projection-backed and summary-only, exactly as P047 specifies.

### REQ-008 The canonical `proposal-047|p047` proof gate is defined for the control-plane workspace
- Proposal Source: `§5 Test Gate` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `docs/reference/test-gates.md:541`
  - `scripts/test-gate.sh:1490`
  - `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md`
- Gap / Note: The gate definition exists in both the docs and the runner. It was not rerun in this audit pass because overall conformance already failed closed on REQ-002 and REQ-005.

## Architecture Review

**Summary:** At Risk

### ARCH-001 The runtime still lacks a canonical producer for `owner_execution_lineage_id` and retry-branch-safe same-owner reuse
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§2b-§2c`, REQ-002
- Evidence Type: code
- Evidence:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:146-153`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:205-210`
  - `control-plane/crates/engine/src/executor.rs:343`
  - `control-plane/crates/engine/src/session/policy.rs:13`
  - `control-plane/crates/domain/src/agent.rs:40`
- Why It Matters: The proposal does not merely want a comparable string; it wants a stable execution-lineage owner chain that can distinguish legitimate same-owner reuse from retry/resume drift. Today the runtime feeds a fresh `AgentExecutionId` into the owner tuple and never carries a separate recovery-branch signal into policy evaluation, so the architecture still under-ports the fail-closed ownership contract it claims to implement.
- Recommended Action: Add a real owner-execution-lineage / recovery-branch producer to the runtime path, persist it on `agent_executions`, and build `InvocationOwnerKey` from that stable lineage input rather than a per-attempt execution UUID.

## Product Review

**Summary:** At Risk

### PROD-001 Live reuse economics still depend on synthesized estimates rather than provider telemetry
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§1b`, `§2e`, REQ-005
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:266`
  - `control-plane/crates/acp/src/session.rs:44`
  - `control-plane/crates/engine/src/executor.rs:560`
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
- Why It Matters: The product promise here is not only “reuse sessions,” but “reuse them only while continuity is still economically justified.” With `cost_cents` always absent and savings inferred from prompt-size heuristics, the daemon can miss the proposal’s cost-driven invalidation path or make compaction decisions from proxies rather than actual provider economics.
- Recommended Action: Extend ACP execution results and adapter plumbing to return real cost/cache telemetry, persist those values on `session_generations`, and drive the economic budget signals from those canonical runtime fields.

## UI Review

**Summary:** Not Applicable

This proposal does not define a direct UI surface. Its only reader-facing commitments are GraphQL/MCP payload shapes, which are audited under conformance and readiness rather than visual/UI fit.

## UX Review

**Summary:** Not Applicable

This slice does not define an end-user interaction flow beyond truthful operator-facing reader data. The relevant UX-adjacent contract is the fail-closed settlement/reuse truth, which is covered in the conformance and product sections.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 P047 is still not ready to call parity-complete because the remaining gaps sit on canonical control-path decisions
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-005, `§5 Test Gate`
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:343`
  - `control-plane/crates/engine/src/session/policy.rs:262`
  - `control-plane/crates/acp/src/session.rs:44`
  - `docs/reference/test-gates.md:541-568`
  - Focused tests listed in the verification log all passed
- Why It Matters: The current tree is no longer blocked on peripheral plumbing. The unresolved work is the proposal’s owner-truth and economics-truth path, which are exactly the places that decide whether reuse is safe and worthwhile. Until those are closed, the slice is not ready to present as parity-complete or regression-proven.
- Recommended Action: Close REQ-002 and REQ-005 first, then rerun `./scripts/test-gate.sh proposal-047` on the same tree before claiming full conformance or readiness.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Relevant Rust crates compiled as part of the focused cargo test runs below; no iOS/macOS simulator gate is required for this slice |
| Core user flow runtime-validated | Partial | Family-scope live reuse, budget checkpoint/resume creation, cancellation finalize, and northbound reader split were validated; same-invocation-owner lineage/retry-branch behavior remains only partially wired |
| Empty/loading/error states covered | Not Applicable | Control-plane proposal with no direct UI flow |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No user-visible strings were audited as a primary slice |
| Critical tests executed | Pass | Focused engine/db/graphql/mcp tests for reuse, budget persistence, cancellation settle, and reader payload shape all passed |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not rerun because the audit already failed closed on objective conformance |
| Privacy/permissions/entitlements reviewed | Not Applicable | No Apple-platform entitlement surface in this proposal |

## Verification Log

- `git rev-parse --show-toplevel`
- `git rev-parse HEAD`
- `git status --short`
- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/047-session-reuse-and-cancellation-settlement.md'`
- `rg -n "SessionReuseDisposition|session_lineage|session_generation|AcpRuntimeManager|ActiveAcpSessionHandle|cancellation_settlement|WorkItemStatus::Cancelled|enum WorkItemStatus|ReusedAfterResume|FreshAfterInvalidation|cancellation_settled_at|session_reuse_disposition|session_generation_id|invocation_owner_key|BudgetDecision|BudgetSignals|QueryRoot.run|runs.get|cancellation_settlement_summary" control-plane/crates`
- `rg -n "proposal-047|p047|session lineage|cancellation settlement|ContextBudgetGuard|FreshAfterInvalidation|ReusedAfterResume|cancellation_settlement" control-plane scripts docs/proposals`
- `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact` -> passed
- `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` -> passed
- `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact` -> passed
- `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact` -> passed
- `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact` -> passed
- `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact` -> passed
- `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact` -> passed
- `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact` -> passed
- `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact` -> passed
- `cargo test -p mcp-server tools::reports::tests::reports_get_decodes_validation_failure_payload -- --exact` -> passed
- `rg -n "current_recovery_branch_id|recovery_branch_id|owner_execution_lineage_id" control-plane/crates/engine control-plane/crates/domain control-plane/crates/db`

## Recommended Next Actions

1. Introduce a real `owner_execution_lineage_id` / retry-branch producer and thread it through the executor, policy input, and retry/resume paths before reusing the session under `same_invocation_owner`.
2. Extend ACP execution results so live prompts return canonical cost/cache telemetry, then persist those signals on `session_generations` and drive the budget economics from them instead of prompt-derived proxies.
3. After those two gaps are closed, rerun `./scripts/test-gate.sh proposal-047` on the same tree and promote the slice only if the full gate passes.
