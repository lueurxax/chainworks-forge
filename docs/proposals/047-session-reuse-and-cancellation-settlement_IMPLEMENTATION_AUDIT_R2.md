# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/047-session-reuse-and-cancellation-settlement.md |
| Repository Root | . |
| Git SHA | db7d51aa91f71f898c4e621c01523708ca7d3c1b |
| Working Tree | dirty (167107 paths reported during audit metadata capture) |
| Audited At | 2026-04-15T22:37:47+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Current `HEAD` closes most of the original P047 gaps. The repository now has the canonical session-lineage migration, full owner/fingerprint inputs, transport-backed live-session reuse, reset/resume/missing-live-handle fail-closed behavior, execution-side provenance persistence plus reader consumption, two-phase cancellation settlement, the single-run vs list-reader cancellation split, and the canonical `proposal-047` test gate. P047 is still only partially implemented because the live context-budget path remains under-ported: the budget evaluator supports the full proposal contract, but runtime policy still feeds only a subset of signals from persisted generations and does not update cumulative token/cost economics during real execution.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Economics-driven context-budget behavior is not fully wired into live session reuse | High |
| Architecture | At Risk | The budget evaluator is richer than the runtime signal-ingestion path that drives it | High |
| Product | At Risk | Long-lived sessions can continue past the intended cost/token/economic thresholds in real execution | High |
| UI | Acceptable | Proposal scope is northbound payload/read-surface wiring rather than a standalone UI | Medium |
| UX | Acceptable | No proposal-owned interactive UX surface is introduced beyond reader payloads | Medium |
| Readiness | Not Ready | Canonical regression is green, but one primary proposal flow remains only partially integrated | High |

## Proposal Contract

### Scope

- Durable session lineage with immutable generations, invocation owner keys, binding fingerprints, and append-only events. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:16-32`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:74-249`
- Generation-scoped context-budget evaluation driven by hard guardrails and economic signals rather than prompt-size heuristics. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:277-330`
- Two-phase cancellation settlement with durable evidence and distinct single-run vs list-reader exposure. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:53-68`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:332-432`

### Locked Decisions

- Existing installs must rename the legacy `session_lineages` table to `session_lineages_legacy`, create canonical lineage tables, and avoid synthetic backfill. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`
- `InvocationOwnerKey` is built from `{run_id, agent_id, stage_lineage_id, task_name, owner_execution_lineage_id}` and is immutable per generation. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-153`
- `BindingFingerprint` must hash the full agent binding including prompt, IO inventory, MCP inventory, skill inputs, permission profile, and output-contract details. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:155-166`
- `SessionReusePolicy` must implement the explicit disposition taxonomy and fail closed on unverifiable or mismatched reuse conditions. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:170-249`
- Execution provenance belongs on `agent_executions` so report and recovery readers can answer “what happened” without lineage joins. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`
- The full cancellation log belongs on canonical single-run reads; list readers get only a one-line projection summary. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`

### Primary User Flows

1. Reuse a live ACP session safely across loop iterations when lineage, binding, scope, and live-handle checks all pass.
2. Force a fresh or resumed generation when bindings drift, sessions are reset, the live handle is missing, or the last generation ended with a checkpoint-backed compaction.
3. Read session provenance from execution-first report and recovery surfaces without reconstructing lineage history.
4. Cancel an in-flight run in two phases and expose full cancellation truth on single-run reads while keeping list rows summary-only.
5. Compact or invalidate reused sessions when the generation budget exceeds hard or economic thresholds.

### UI Commitments

- No dedicated session-inspector UI is in scope. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:509-513`
- The concrete northbound commitment is reader payload shape: canonical single-run reads expose the full `cancellation_settlement_log`; list reads expose only `cancellation_settlement_summary`. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:471-472`

### UX Commitments

- Reuse must fail closed when lineage truth or runtime live-session truth is unverifiable. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:195-249`
- Reset, resume, budget, and cancellation outcomes must be explicit and durable rather than implicit runtime behavior. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:173-330`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:334-367`

### Acceptance Criteria

- Session-lineage acceptance covers live reuse, fingerprint mismatch, reset, immutable owner/fingerprint fields, resume provenance, fail-closed behavior, and lineage migration. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:439-457`
- Context-budget acceptance covers hard guardrails plus economics-driven invalidation and compaction. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:459-465`
- Cancellation acceptance covers two-phase settlement, supporting work-item cleanup, no residual active executions, and northbound reader split. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`

### Test / Evidence Requirements

- Proposal 047 defines a canonical `proposal-047|p047` gate in `scripts/test-gate.sh` and `docs/reference/test-gates.md`. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`
- A successful audit verdict would require passing same-tree full regression evidence. This audit executed the canonical `proposal-047` gate on the same tree and `HEAD`.

### Explicit Exclusions

- Session checkpoint serialization format. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:509-513`
- Provider-specific budget tuning. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:511-512`
- UI for a session inspector. Source: `docs/proposals/047-session-reuse-and-cancellation-settlement.md:513`

## Proposal Fidelity / Divergence

### Matches

- The migration renames the legacy `session_lineages` table, creates canonical `session_lineages` / `session_generations` / `session_events`, and avoids synthetic backfill.
- Invocation owner keys and binding fingerprints now include the proposal-owned inputs rather than the earlier narrow subset.
- Policy and executor behavior now cover reset, resume, owner mismatch, family-scope owner relaxation, missing-live-handle fallback, and checkpoint provenance.
- Execution-side provenance is persisted on `agent_executions` and consumed by recovery and report readers without lineage joins.
- Cancellation settlement follows the promised two-phase `cancelling -> cancelled` contract and northbound readers correctly split full log vs summary-only surfaces.
- The canonical `proposal-047` gate now exists and passes on the current tree.

### Divergences

- The budget evaluator implements the proposal’s signal model, but the live policy path still feeds only a subset of those signals from persisted generations.
- Runtime generation updates still record only `provider_session_id` and `turn_count`, so real executions do not accumulate prompt-token, cost, or economics-driven budget history as the proposal describes.

### Ambiguities / Evidence Gaps

- Platform scope is `Ambiguous` because this is a daemon/control-plane slice rather than a platform-specific UI proposal.
- No standalone operator session-inspector UI is proposal-owned, so UI/UX review is limited to payload/read-surface behavior rather than runtime screens.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Legacy migration replaces the projection-era lineage schema with canonical lineage tables and no synthetic backfill

- Proposal Source: `§2a` and AC11-12 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:451-453`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1-21`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`
- Gap / Note: Current reads and writes target the canonical lineage tables only; no synthetic generation backfill path was found.

### REQ-002 Invocation owner key, binding fingerprint, and generation immutability follow the proposal-owned binding contract

- Proposal Source: `§2b` and AC4-5 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-166`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:443-445`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:4-75`
  - `control-plane/crates/engine/src/executor.rs:345-385`
  - `control-plane/crates/db/src/repos/sessions.rs:194-210`
  - `cargo test --workspace`
- Gap / Note: Current Rust stores a deterministic hashed owner key rather than the literal colon-delimited tuple shown in the proposal text, but it includes the same proposal-owned components and current runtime updates do not mutate owner key or fingerprint after generation creation.

### REQ-003 Reuse policy, live-handle ownership, reset/resume, and fail-closed transport behavior match the durable session-owner contract

- Proposal Source: `§2c`, `§2d`, and AC1-10 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:170-249`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:439-449`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:36-260`
  - `control-plane/crates/engine/src/executor.rs:388-427`
  - `control-plane/crates/acp/src/manager.rs:18-205`
  - `control-plane/crates/engine/tests/integration.rs:539`
  - `control-plane/crates/engine/tests/integration.rs:1183`
  - `control-plane/crates/engine/tests/integration.rs:1576`
  - `control-plane/crates/engine/tests/integration.rs:1798`
  - `cargo test -p engine session::policy::tests::owner_mismatch_in_same_invocation_owner_scope_requires_fresh_session --lib -- --exact`
  - `cargo test -p engine session::policy::tests::resumes_from_checkpointed_generation_when_last_end_reason_carries_artifact_id --lib -- --exact`
  - `cargo test -p engine --test integration test_reset_session_marks_generation_reset_and_next_policy_is_fresh_after_reset -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact -- --exact`
- Gap / Note: Current implementation now covers the previously-missing reset, resume, and missing-live-handle paths. Recovery-branch drift is realized through the `owner_execution_lineage_id` component of the owner key rather than as a separately stored field.

### REQ-004 Execution-side provenance is persisted on `agent_executions` and consumed by report/recovery readers without lineage joins

- Proposal Source: `§2d-ii` and AC13-14 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:455-457`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:48-55`
  - `control-plane/crates/engine/src/executor.rs:455-492`
  - `control-plane/crates/db/src/repos/agent_executions.rs:8-32`
  - `control-plane/crates/engine/src/recovery.rs:84-90`
  - `control-plane/crates/engine/src/recovery.rs:182-206`
  - `control-plane/crates/graphql-server/src/types/artifact.rs:141-165`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:64-90`
  - `control-plane/crates/db/tests/integration.rs:51`
  - `control-plane/crates/engine/tests/integration.rs:194-237`
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact`
  - `cargo test --workspace`
- Gap / Note: Current recovery and validation-failure report readers decorate their output from `agent_executions`, so the “what happened” question no longer depends on lineage-table joins.

### REQ-005 Context-budget evaluation is generation-scoped and economics-driven in live reuse decisions

- Proposal Source: `§1b`, `§2e`, and AC15-20 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:277-330`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:459-465`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:1-127`
  - `control-plane/crates/engine/src/session/policy.rs:262-276`
  - `control-plane/crates/db/src/repos/sessions.rs:194-210`
  - `control-plane/crates/engine/src/executor.rs:558-574`
  - `cargo test -p engine session::budget::tests::invalidates_when_reuse_is_economically_worse_than_fresh --lib -- --exact`
  - `cargo test --workspace`
- Gap / Note: The evaluator itself now supports the proposal’s hard and economic signals, but the live policy path still feeds `None` or `0` for `cached_token_share`, `normalized_savings_versus_fresh`, `effective_prompt_size_fraction`, and `compaction_churn_count`, and runtime generation updates still only persist `provider_session_id` plus `turn_count`. In practice, the real execution path only has reliable turn-based budget behavior today.

### REQ-006 Cancellation settlement follows the promised two-phase contract and cleans up active executions/work items before final cancel

- Proposal Source: `§2f` and AC21-23, AC26-27 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:334-397`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs:22-168`
  - `control-plane/crates/db/src/work_item.rs:53-77`
  - `control-plane/crates/db/src/repos/work_items.rs:130-146`
  - `control-plane/crates/db/src/repos/runs.rs:122-190`
  - `control-plane/crates/engine/tests/integration.rs:241`
  - `control-plane/crates/engine/tests/integration.rs:302`
  - `control-plane/crates/engine/tests/integration.rs:1383`
  - `cargo test -p engine --test integration test_cancel_run_phase1_cancels_agent_executions_and_running_work_items -- --exact`
  - `cargo test -p engine --test integration test_cancel_run_eventually_finalizes_to_cancelled -- --exact`
  - `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`
- Gap / Note: The current implementation now satisfies the previously-missing execution-first settlement evidence and async close-outcome contract.

### REQ-007 Single-run readers expose the full cancellation log while list readers expose only the projection summary

- Proposal Source: `§3` northbound reader wiring and AC24-25 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `docs/proposals/047-session-reuse-and-cancellation-settlement.md:471-472`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/types/run.rs:5-76`
  - `control-plane/crates/db/src/repos/projections.rs:15-33`
  - `control-plane/crates/db/src/repos/projections.rs:59-99`
  - `control-plane/crates/db/src/repos/projections.rs:267-305`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:124-136`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:249-348`
  - `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact`
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`
- Gap / Note: Current northbound readers now match the proposal’s canonical-vs-projection split.

### REQ-008 The canonical `proposal-047` gate exists and passes on the audited tree

- Proposal Source: `§5 Test Gate` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:1490-1497`
  - `docs/reference/test-gates.md:541-568`
  - `./scripts/test-gate.sh proposal-047`
- Gap / Note: The proposal-defined canonical proof path now exists and passed on this exact tree and `HEAD`.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Budget evaluator and runtime signal ingestion still diverge

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:1-127`
  - `control-plane/crates/engine/src/session/policy.rs:262-276`
  - `control-plane/crates/db/src/repos/sessions.rs:194-210`
  - `cargo test -p engine session::budget::tests::invalidates_when_reuse_is_economically_worse_than_fresh --lib -- --exact`
- Why It Matters: The proposal explicitly makes budget decisions an execution-time owner of session reuse safety. Today the evaluator is richer than the signal pipeline that feeds it, so the durable lineage substrate can exist while real runtime decisions still ignore most of the economics-driven contract.
- Recommended Action: Extend generation updates to persist cumulative prompt tokens, cumulative cost, and any available runtime-economics metadata; then feed those persisted/runtime values into `budget_signals_from_generation()` instead of defaulting to `None` / `0`.

## Product Review

**Summary:** At Risk

### PROD-001 Long-running reuse can drift from the proposal’s intended economic guardrails

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:558-574`
  - `control-plane/crates/engine/src/session/policy.rs:262-276`
  - `control-plane/crates/engine/src/session/budget.rs:45-127`
  - `cargo test --workspace`
- Why It Matters: The session-lineage and reuse system now works, but the proposal’s value proposition includes economic safety. Without live token/cost/economics updates, operators can keep reusing sessions in cases where the proposal says the daemon should compact or invalidate.
- Recommended Action: Wire real runtime token/cost/economic measurements into generation updates and add end-to-end tests that prove AC16-20 from actual executor behavior rather than from seeded unit inputs alone.

## UI Review

**Summary:** Acceptable

No material UI findings discovered. P047 changes daemon behavior and northbound payload shape rather than introducing a standalone interface.

## UX Review

**Summary:** Acceptable

No material UX findings discovered beyond the budget-gap behavior already captured above. The concrete user-facing interaction contract in this proposal is primarily about durable reader truth, and that slice is implemented.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Canonical regression is green, but the budget slice is still not proposal-complete

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-008
- Evidence Type: code, tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-047`
  - `control-plane/crates/engine/src/session/policy.rs:262-276`
  - `control-plane/crates/db/src/repos/sessions.rs:194-210`
- Why It Matters: The repo now has the proposal gate and it passes, so this is no longer a regression-evidence problem. The remaining blocker is an implementation completeness problem in one of the proposal’s primary flows.
- Recommended Action: Treat the budget-signal ingestion path as the remaining blocking slice, then rerun `./scripts/test-gate.sh proposal-047` after adding end-to-end proof for the live economic thresholds.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Canonical control-plane gate passed via `./scripts/test-gate.sh proposal-047` |
| Core user flow runtime-validated | Partial | Reuse/reset/resume/missing-live-handle/cancel flows are runtime-tested; live economic-budget behavior is not |
| Empty/loading/error states covered | Not Checked | No proposal-owned UI state machine in scope |
| Accessibility risk acceptable | Not Checked | No proposal-owned UI surface in scope |
| Localization risk acceptable | Not Checked | No proposal-owned UI strings in scope |
| Critical tests executed | Pass | Focused engine/db/graphql/mcp tests executed for lineage, cancellation, and northbound readers |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-047` |
| Privacy/permissions/entitlements reviewed | Not Checked | Not applicable to this control-plane Rust slice |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/047-session-reuse-and-cancellation-settlement.md'`
- `rg -n -i 'superseded|deprecated|replaced by|obsolete' docs/proposals/047-session-reuse-and-cancellation-settlement.md docs/proposals docs/reference docs/reviews -g '*.md' -g '!**/*IMPLEMENTATION_AUDIT_*.md' -g '!**/*REVIEW_TRIAD_*.md'`
- `cargo test -p engine session::policy::tests::resumes_from_checkpointed_generation_when_last_end_reason_carries_artifact_id --lib -- --exact`
- `cargo test -p engine session::policy::tests::owner_mismatch_in_same_invocation_owner_scope_requires_fresh_session --lib -- --exact`
- `cargo test -p engine session::budget::tests::invalidates_when_reuse_is_economically_worse_than_fresh --lib -- --exact`
- `cargo test -p engine --test integration test_reset_session_marks_generation_reset_and_next_policy_is_fresh_after_reset -- --exact`
- `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
- `cargo test -p engine --test integration test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation -- --exact`
- `cargo test -p engine --test integration test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact -- --exact`
- `cargo test -p engine --test integration test_cancel_run_phase1_cancels_agent_executions_and_running_work_items -- --exact`
- `cargo test -p engine --test integration test_cancel_run_eventually_finalizes_to_cancelled -- --exact`
- `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`
- `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`
- `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact`
- `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact`
- `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
- `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
- `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
- `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`
- `./scripts/test-gate.sh proposal-047`

## Recommended Next Actions

1. Persist live generation metrics beyond `turn_count`, specifically prompt-token and cost counters plus any available economics-driven reuse signals.
2. Replace the `None` / `0` placeholders in `budget_signals_from_generation()` with real persisted/runtime values and prove AC16-20 through executor-level integration tests.
3. Re-run `./scripts/test-gate.sh proposal-047` after the budget-signal wiring is complete and promote the audit from `Partial` to `Implemented` only if that live budget slice is fully closed.
