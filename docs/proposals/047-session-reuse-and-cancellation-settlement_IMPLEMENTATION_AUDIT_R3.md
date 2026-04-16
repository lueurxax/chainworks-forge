# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | docs/proposals/047-session-reuse-and-cancellation-settlement.md |
| Repository Root | . |
| Git SHA | db7d51aa91f71f898c4e621c01523708ca7d3c1b |
| Working Tree | dirty (174689 paths reported during audit metadata capture) |
| Audited At | 2026-04-15T23:03:51+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Current `HEAD` implements most of P047. The repository now has the canonical lineage migration, durable lineage/generation/event persistence, transport-backed live-session ownership in `AcpRuntimeManager`, fail-closed reuse on binding/live-handle mismatch, execution-side provenance on `agent_executions`, two-phase cancellation settlement, and the canonical single-run versus list-reader cancellation split across GraphQL and MCP.

P047 is still only partially implemented for two proposal-owned reasons. First, `invocation_owner_key` is persisted as a SHA-256 digest of the owner components instead of the explicit persisted tuple that `§2b` and the stable session-lineage reference define. Second, the live budget path still under-ports the proposal's economics contract: the evaluator exists and persisted generation counters now flow into it, but the runtime still synthesizes cache share / savings / prompt-fraction signals from prompt estimates, while ACP prompt execution returns no real provider cost or cache metadata. That leaves one locked storage decision and one primary control-flow decision only partially aligned with the proposal.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | `invocation_owner_key` storage and live budget economics still diverge from the proposal contract | High |
| Architecture | At Risk | Owner identity is reduced to an opaque digest, weakening the persisted provenance contract | High |
| Product | At Risk | Long-lived session reuse still makes economics decisions from synthesized estimates instead of real runtime telemetry | High |
| UI | Acceptable | Proposal scope is northbound payload/read-surface behavior rather than a standalone UI | Medium |
| UX | Acceptable | No proposal-owned interactive UX surface is introduced beyond fail-closed reader behavior | Medium |
| Readiness | Not Ready | The remaining gaps sit on locked session-ownership and budget-decision paths, not on peripheral polish | High |

## Proposal Contract

### Scope

- Durable session lineage with immutable generations, explicit invocation owner keys, binding fingerprints, and append-only events. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:16-32`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:74-275`
- Generation-scoped context-budget evaluation driven by hard guardrails plus economics rather than prompt-size heuristics. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:277-330`
- Two-phase cancellation settlement with durable settlement evidence and distinct canonical-vs-projection northbound reader behavior. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:53-68`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:332-432`

### Locked Decisions

- Existing installs must rename the legacy `session_lineages` table to `session_lineages_legacy`, create canonical lineage tables, and avoid synthetic backfill. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`
- `InvocationOwnerKey` is the persisted tuple `{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}` and is immutable on the generation. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-153`
- `BindingFingerprint` must hash the full binding contract, including prompt text, IO inventory, MCP inventory, skill snapshot, permission profile, and output-contract details. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:155-166`
- Reuse policy must implement the explicit disposition taxonomy and fail closed on unverifiable or mismatched reuse conditions. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:170-249`
- Execution provenance belongs on `agent_executions` so report/recovery readers can answer "what happened" without lineage joins. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`
- Cancellation truth is canonical on single-run reads and summary-only on list/projection reads. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`

### Primary User Flows

1. Reuse a live ACP session safely across loop iterations when lineage, owner, binding, and live-handle checks all pass.
2. Force a fresh or resumed generation when bindings drift, operators reset the session, the live handle is missing, or the last generation ended with checkpoint-backed compaction.
3. Read session provenance from execution-first report and recovery surfaces without reconstructing lineage history.
4. Cancel an in-flight run in two phases and expose full settlement truth on single-run reads while keeping list readers summary-only.
5. Compact or invalidate a reused session when the generation budget crosses hard or economic thresholds.

### UI Commitments

- No standalone session-inspector UI is in scope. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:509-513`
- The concrete northbound surface is payload shape: canonical single-run reads expose `cancellation_settlement_log`, while list readers expose only `cancellation_settlement_summary`. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:471-472`

### UX Commitments

- Reuse must fail closed when lineage truth or runtime live-session truth is unverifiable. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:195-249`
- Reset, resume, budget, and cancellation outcomes must be explicit and durable rather than inferred. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:173-330`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:334-397`

### Acceptance Criteria

- Session-lineage acceptance covers live reuse, binding mismatch, reset, immutable owner/fingerprint fields, resume provenance, and fail-closed missing-live-handle behavior. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:439-449`
- Legacy migration acceptance requires legacy-table rename plus no synthetic backfill. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:451-453`
- Execution-side provenance acceptance requires reader answers from `agent_executions` alone. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:455-457`
- Context-budget acceptance requires hard guardrails plus economics-driven compaction/invalidation using persisted generation signals. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:459-465`
- Cancellation acceptance requires two-phase settlement, supporting work-item cleanup, no residual active executions, and the single-run vs list-reader split. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`

### Test / Evidence Requirements

- The repo must define a canonical `proposal-047|p047` gate in `scripts/test-gate.sh` and `docs/reference/test-gates.md`. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`
- Because this audit failed closed on objective conformance before a successful roll-up, the full `proposal-047` gate was not rerun in this R3 audit pass.

### Explicit Exclusions

- Session checkpoint serialization format. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:511`
- Provider-specific budget tuning. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:512`
- UI for a session inspector. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:513`

## Proposal Fidelity / Divergence

### Matches

- Legacy lineage migration now renames the projection-era table, creates canonical lineage tables, and widens execution/run storage for provenance and cancellation truth.
- Live reuse is transport-backed through `AcpRuntimeManager`, with fail-closed fallback when the DB says a generation is active but no live handle exists.
- Execution-side session provenance is persisted on `agent_executions` and consumed by report/recovery surfaces without lineage joins.
- Cancellation settlement now follows a true `cancelling -> cancelled` two-phase path and northbound readers split canonical full log from projection summary.

### Divergences

- `invocation_owner_key` is persisted as a hash of the owner components instead of the explicit tuple promised by `§2b`.
- Economic budget signals are still synthesized from prompt-size estimates and generation counters because ACP execution does not return real provider cache or cost telemetry.

### Ambiguities / Evidence Gaps

- Platform scope remains `Ambiguous` because this is a daemon/control-plane slice rather than a platform-specific UI proposal.
- The full canonical `proposal-047` regression gate exists, but it was not rerun in R3 because the audit already failed closed on objective conformance.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Legacy migration replaces the projection-era lineage schema with canonical lineage tables and no synthetic backfill

- Proposal Source: `§2a` and AC11-12 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:451-453`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1-59`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact` (passed)
- Gap / Note: Canonical readers/writers now target the new lineage tables; no synthetic generation backfill path was found.

### REQ-002 Invocation owner key persists the proposal-owned tuple, binding fingerprint covers the full binding contract, and both remain immutable on the generation

- Proposal Source: `§2b` and AC4-5 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-166`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:443-445`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:4-20`
  - `control-plane/crates/engine/src/session/fingerprint.rs:22-81`
  - `control-plane/crates/engine/src/executor.rs:345-385`
  - `control-plane/crates/db/src/repos/sessions.rs:30-62`
  - `docs/reference/session-lineage-reuse-and-operator-reset.md:44-60`
- Gap / Note: Binding fingerprint coverage and generation immutability are implemented, but `invocation_owner_key` is stored as a SHA-256 digest of the owner components rather than the explicit persisted tuple required by the proposal and stable reference. The implementation preserves equality semantics, but not the promised stored owner shape or inspectability.

### REQ-003 Reuse policy, live-handle ownership, reset/resume, and fail-closed transport behavior match the durable session-owner contract

- Proposal Source: `§2c`, `§2d`, and AC1-10 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:170-249`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:439-449`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/session.rs:25-76`
  - `control-plane/crates/engine/src/session/policy.rs:36-260`
  - `control-plane/crates/acp/src/manager.rs:17-183`
  - `control-plane/crates/engine/src/executor.rs:388-427`
  - `control-plane/crates/engine/tests/integration.rs:1185`
  - `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact` (passed)
- Gap / Note: Current `HEAD` covers the previously-missing live-session reuse, checkpoint-backed resume, reset, and missing-live-handle fail-closed paths.

### REQ-004 Execution-side provenance is persisted on `agent_executions` and report/recovery readers can answer "what happened" without lineage joins

- Proposal Source: `§2d-ii` and AC13-14 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:455-457`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:48-55`
  - `control-plane/crates/domain/src/agent.rs:40-57`
  - `control-plane/crates/db/src/repos/agent_executions.rs:8-35`
  - `control-plane/crates/engine/src/executor.rs:455-492`
  - `control-plane/crates/engine/src/recovery.rs:182-206`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:64-90`
  - `control-plane/crates/db/tests/integration.rs:160`
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact` (passed)
- Gap / Note: Current recovery/report readers can decorate output directly from `agent_executions`, which matches the execution-first ownership promised by the proposal.

### REQ-005 Context-budget evaluation is generation-scoped and economics-driven in live reuse decisions

- Proposal Source: `§1b`, `§2e`, and AC15-20 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:277-330`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:459-465`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/session.rs:55-75`
  - `control-plane/crates/db/migrations/007_session_budget_signals.sql:1-5`
  - `control-plane/crates/engine/src/session/budget.rs:1-128`
  - `control-plane/crates/engine/src/session/policy.rs:262-299`
  - `control-plane/crates/db/src/repos/sessions.rs:198-228`
  - `control-plane/crates/acp/src/lib.rs:57-74`
  - `control-plane/crates/acp/src/session.rs:44-55`
  - `control-plane/crates/engine/src/executor.rs:515-578`
  - `control-plane/crates/engine/tests/integration.rs:1385`
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
- Gap / Note: Persisted generation counters now feed the evaluator, but the economics half of the contract is still under-ported. `cached_token_share`, `normalized_savings_versus_fresh`, and `effective_prompt_size_fraction` are synthesized from prompt-size estimates inside `budget_signals_from_generation()`, while ACP prompt execution returns `cost_cents: None` and no provider cache metadata. The evaluator exists, but live economics decisions are still heuristic rather than provider-metadata-backed.

### REQ-006 Cancellation settlement follows the promised two-phase contract and cleans up active executions/work items before final cancel

- Proposal Source: `§2f` and AC21-23, AC26-27 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:334-397`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/src/work_item.rs:46-81`
  - `control-plane/crates/db/src/repos/work_items.rs:130-146`
  - `control-plane/crates/domain/src/run.rs:28-40`
  - `control-plane/crates/db/src/repos/runs.rs:121-190`
  - `control-plane/crates/engine/src/cancellation.rs:22-168`
  - `control-plane/crates/engine/tests/integration.rs:1604`
  - `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact` (passed)
- Gap / Note: The current implementation now settles cancellation through canonical `Cancelling` and `Cancelled` run states, cancels running work items, persists execution-keyed settlement entries, and records close outcomes in Phase 2. The proposal explicitly keeps stages on `Failed`, so the absence of `StageStatus::Cancelled` is not a conformance gap.

### REQ-007 Single-run readers expose the full cancellation log while list readers expose only the projection summary

- Proposal Source: `§3` northbound reader wiring and AC24-25 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:471-472`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:12-33`
  - `control-plane/crates/db/src/repos/projections.rs:59-99`
  - `control-plane/crates/db/src/repos/projections.rs:267-305`
  - `control-plane/crates/graphql-server/src/schema.rs:59-76`
  - `control-plane/crates/graphql-server/src/types/run.rs:5-76`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:124-145`
  - `control-plane/crates/db/tests/integration.rs:1042`
  - `control-plane/crates/graphql-server/src/schema.rs:699`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:249`
  - `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact` (passed)
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact` (passed)
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact` (passed)
- Gap / Note: `QueryRoot.run(id)` reads canonical `Run`, while `runs` list queries and `runs.list` stay projection-backed and summary-only, which matches the proposal's explicit split.

### REQ-008 The repository defines the canonical `proposal-047|p047` proof gate

- Proposal Source: `§5 Test Gate` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `docs/reference/test-gates.md:541-568`
  - `scripts/test-gate.sh:1198`
  - `scripts/test-gate.sh:1490-1497`
  - `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md`
- Gap / Note: The gate exists in the repo inventory. It was not rerun in this R3 audit because overall conformance was already partial and the skill only requires same-tree full regression before a successful roll-up.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Session owner identity is persisted as an opaque digest instead of the proposal's explicit tuple

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Locked Decisions, REQ-002
- Evidence Type: code, design-reference
- Evidence:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-153`
  - `docs/reference/session-lineage-reuse-and-operator-reset.md:44-60`
  - `control-plane/crates/engine/src/session/fingerprint.rs:12-20`
  - `control-plane/crates/db/src/repos/sessions.rs:30-62`
- Why It Matters: The proposal does not merely require equality checks; it explicitly defines the stored owner identity shape. Hashing the tuple preserves comparison behavior, but it drops the promised persisted owner components that report/recovery readers and operator inspection can reason about directly.
- Recommended Action: Persist the proposal-defined tuple string on `session_generations` and `agent_executions`, or explicitly amend the proposal/reference to make the hashed representation canonical instead of silently narrowing the owner contract in code.

## Product Review

**Summary:** At Risk

### PROD-001 Live reuse economics are still decided from synthesized estimates rather than provider runtime telemetry

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Primary User Flows, REQ-005
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:262-299`
  - `control-plane/crates/acp/src/lib.rs:57-74`
  - `control-plane/crates/acp/src/session.rs:44-55`
  - `control-plane/crates/engine/tests/integration.rs:1385`
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
- Why It Matters: The user-facing job here is not just "reuse a session"; it is "reuse it only while continuity is still economically and operationally worth it." Today the daemon can pass unit-level budget logic while still making live continue/compact/invalidate decisions from prompt-size proxies rather than real provider cache and cost telemetry.
- Recommended Action: Extend ACP execution results with real prompt/cost/cache signals, persist them on the active generation, and drive `cached_token_share`, `normalized_savings_versus_fresh`, and related budget inputs from those canonical runtime fields.

## UI Review

**Summary:** Acceptable

No additional blocking UI findings. P047 does not introduce a proposal-owned standalone screen; its concrete UI surface is northbound payload shape, and the current GraphQL/MCP canonical-vs-projection split matches that contract.

## UX Review

**Summary:** Acceptable

No additional blocking UX findings. The proposal's UX commitments are primarily fail-closed runtime behavior and explicit reader truth, both of which are already captured in the conformance audit above.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Remaining gaps sit on canonical owner and budget-decision paths, so this slice is not ready to call parity-complete

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-005
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:12-20`
  - `control-plane/crates/engine/src/session/policy.rs:262-299`
  - `control-plane/crates/acp/src/session.rs:44-55`
  - `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact` (passed)
  - `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact` (passed)
- Why It Matters: The unresolved work is not peripheral. It sits on the two places where the proposal locked behavioral truth: who owns a generation, and when that generation should stop being reused. That is enough to block a parity-complete or handoff-ready verdict even though most of the slice now exists.
- Recommended Action: Close the owner-key representation gap, wire real runtime economics into generation updates, then rerun the canonical `proposal-047` gate on the same tree before claiming readiness.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Canonical lineage migration implemented | Pass | `006_session_lineage.sql` plus migration test passed |
| Live reuse path runtime-verified | Pass | Focused reuse integration test passed |
| Execution-side provenance available to report/recovery readers | Pass | `agent_executions` fields persisted and report/recovery readers consume them |
| Budget decisions use proposal-owned live economics | Partial | Evaluator exists, but economics inputs are still synthesized from estimates |
| Two-phase cancellation settlement runtime-verified | Pass | Focused cancellation finalize test passed |
| Canonical-vs-projection reader split verified | Pass | Focused DB / GraphQL / MCP tests passed |
| Same-tree full proposal gate rerun in this audit | Not Run | Skipped because objective conformance already landed Partial |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/047-session-reuse-and-cancellation-settlement.md'`
- `rg -n -i "superseded|deprecated|replaced by|obsolete" docs/proposals/047-session-reuse-and-cancellation-settlement.md docs/proposals docs/reference -g '*.md' -g '!**/*IMPLEMENTATION_AUDIT_*.md' -g '!**/*REVIEW_TRIAD_*.md'`
- `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md`
- `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`
- `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
- `cargo test -p engine --test integration test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it -- --exact`
- `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`
- `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
- `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
- `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact`

## Recommended Next Actions

- Make an explicit decision about `invocation_owner_key` storage: persist the proposal-defined tuple or amend the proposal/reference so the digest form becomes canonical.
- Extend ACP execution results and generation updates so budget economics consume real provider telemetry instead of prompt-derived proxies.
- After those two gaps are closed, rerun `./scripts/test-gate.sh proposal-047` on the same tree before claiming parity or readiness.
