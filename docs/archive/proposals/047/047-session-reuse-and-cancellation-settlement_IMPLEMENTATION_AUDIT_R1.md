# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/047-session-reuse-and-cancellation-settlement.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `db7d51aa91f7` |
| Working Tree | `modified=129 added=0 deleted=4410 unmerged=0 other=143992` |
| Audited At | `2026-04-15T21:33:54+03:00` |
| Platform Scope | `Universal` |
| Proposal State | `Active` (proposal file status is `Draft`, but no supersession/deprecation marker was found) |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 047 is only partially realized in the current tree. The repository now has the canonical session-lineage schema, reuse metadata propagation, execution-side provenance persistence, transport-backed live-session reuse, two-phase cancellation settlement, and the single-run vs list-reader cancellation split. The proposal is still not implemented overall because the runtime policy does not enforce the promised disposition model or fail-closed semantics, the owner/fingerprint contract is materially narrower than specified, the economics-driven budget layer is absent, `ResetSession` does not produce `FreshAfterReset`, no report/recovery surface consumes execution provenance as the canonical "what happened" source, and the required `proposal-047` test gate is missing.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Reuse-policy contract is materially narrower than Proposal 047 | High |
| Architecture | At Risk | DB lineage truth can still diverge from runtime live-handle truth | High |
| Product | At Risk | Reset/resume and budget behaviors promised by the proposal are absent | High |
| UI | Acceptable | No standalone UI surface is in scope beyond reader payload shape | Medium |
| UX | At Risk | Operator recovery semantics remain opaque because reset/reuse provenance is incomplete | High |
| Readiness | Not Ready | Proposal-specific gate is absent and several acceptance seams are still unimplemented | High |

## Proposal Contract

### Scope

- Durable session lineage with immutable generations, invocation owner keys, binding fingerprints, and append-only session events. Source: `§1a`, `§2a`, `§2b`, `§2c` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:16-32`, `:74-249`).
- Generation-scoped context budget evaluation driven by hard guardrails plus economic signals rather than prompt-size heuristics. Source: `§1b`, `§2e` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:34-51`, `:277-330`).
- Two-phase cancellation settlement with durable execution-first evidence, asynchronous session close outcomes, and reader-specific log vs summary exposure. Source: `§1c`, `§2f` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:53-68`, `:332-397`).

### Locked Decisions

- Existing installs must rename the legacy `session_lineages` table to `session_lineages_legacy`, create new canonical lineage tables, and avoid synthetic backfill. Source: `§2a` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`).
- `InvocationOwnerKey` must include `{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}`. Source: `§2b` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-153`).
- `BindingFingerprint` must hash the full stable binding surface, including prompt, skill injection, IO inventory, MCP inventory, permission profile, and output contract. Source: `§2b` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:155-166`).
- `SessionReusePolicy.evaluate()` must implement the explicit disposition taxonomy and scope-sensitive checks, including recovery-branch fail-closed behavior. Source: `§2c` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:168-213`).
- `SessionReuseDisposition::Reused` is valid only when lineage checks pass and `AcpRuntimeManager` still owns a matching live handle. Source: `§2d` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:230-249`).
- Execution provenance must be written onto `agent_executions` so report/recovery readers can answer reuse questions without lineage joins. Source: `§2d-ii` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`).
- Cancellation readers must expose the full canonical log on single-run reads and only a summary on list reads. Source: `§3` Northbound reader wiring, `§4` AC24-25 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `:471-472`).

### Primary User Flows

- Reuse a live ACP session safely across loop iterations when lineage, binding, scope, and runtime-handle checks all pass.
- Force a fresh generation when bindings drift, ownership/recovery constraints fail, or the context budget decides compaction/invalidation.
- Persist execution-first provenance so later report/recovery readers can explain what happened without reconstructing lineage joins.
- Cancel an in-flight run in two phases, close live sessions asynchronously, and settle the run only after durable outcomes exist.
- Read full cancellation truth on a single run while keeping list projections compact and summary-only.

### UI Commitments

- No dedicated visual session inspector is in scope. Source: `§6` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:509-513`).
- The only concrete reader-surface commitment is payload shape: single-run paths return `cancellation_settlement_log`, list paths return only `cancellation_settlement_summary`. Source: `§3`, `§4` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `:471-472`).

### UX Commitments

- Session reuse must fail closed when lineage history or runtime-handle truth is unverifiable. Source: `§2c`, `§2d` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:195-249`).
- Reset, budget, timeout, transport, and invalidation cases must produce stable dispositions instead of ad hoc behavior. Source: `§2c`, `§2e` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:173-213`, `:315-330`).
- Operators should observe `Cancelling` until settlement finishes, then see durable cancellation results. Source: `§1c`, `§2f` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:63-68`, `:334-367`).

### Acceptance Criteria

- Session-lineage acceptance covers live reuse, fingerprint mismatch, operator reset, immutable owner/fingerprint fields, resume-from-checkpoint, recovery-branch mismatch, invalidation mapping, and no-live-handle fail-closed behavior. Source: `§4` AC1-10 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:439-449`).
- Legacy migration acceptance covers rename, canonical table creation, and no synthetic backfill. Source: `§4` AC11-12 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:451-453`).
- Execution-side provenance acceptance requires agent executions to carry reuse/session metadata and for report/recovery readers to answer reuse disposition from those rows alone. Source: `§4` AC13-14 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:455-457`).
- Context-budget acceptance covers both hard guardrails and economic signals. Source: `§4` AC15-20 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:459-465`).
- Cancellation acceptance covers two-phase settlement, reader split, and absence of active executions/work items after phase 2. Source: `§4` AC21-27 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:467-474`).

### Test / Evidence Requirements

- Proposal 047 requires a canonical `proposal-047|p047` test-gate entry in both `scripts/test-gate.sh` and `docs/reference/test-gates.md`. Source: `§5` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`).
- Any successful audit verdict would require same-tree full regression. This audit did not reach that state, so focused proof was sufficient.

### Explicit Exclusions

- Session checkpoint serialization format. Source: `§6` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:511`).
- Provider-specific budget tuning. Source: `§6` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:512`).
- UI for a session inspector. Source: `§6` (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:513`).

## Proposal Fidelity / Divergence

### Matches

- The migration renames the legacy `session_lineages` table and creates canonical `session_lineages`, `session_generations`, and `session_events`, with no synthetic backfill logic present. Evidence: `control-plane/crates/db/migrations/006_session_lineage.sql:1-59`, `control-plane/crates/db/tests/integration.rs:21`.
- Workflow parsing/compilation now carries `session_reuse_scope` and `session_family_id` from the catalog into resolved bindings used by execution. Evidence: `control-plane/crates/workflow/src/catalog.rs:102-122`, `control-plane/crates/workflow/src/compiler.rs:196-202`, `control-plane/crates/workflow/src/compiler.rs:346-355`, `control-plane/crates/workflow/tests/integration.rs:66`, `control-plane/crates/workflow/tests/integration.rs:97`.
- The executor persists session provenance directly on `agent_executions` before ACP start, and the repository round-trips those fields. Evidence: `control-plane/crates/engine/src/executor.rs:335-372`, `control-plane/crates/domain/src/agent.rs:40-58`, `control-plane/crates/db/src/repos/agent_executions.rs:8-35`, `:55-141`, `control-plane/crates/db/tests/integration.rs:51`.
- ACP reuse is now transport-backed through `AcpRuntimeManager`, and there is a passing end-to-end engine test for same-generation reuse. Evidence: `control-plane/crates/acp/src/lib.rs:13-74`, `control-plane/crates/acp/src/manager.rs:17-154`, `control-plane/crates/engine/tests/integration.rs:722`.
- Two-phase cancellation settlement is implemented, including live-session close attempts, durable settlement log updates, and post-finalize `Cancelled` status. Evidence: `control-plane/crates/engine/src/cancellation.rs:22-168`, `control-plane/crates/db/src/repos/runs.rs:121-190`, `control-plane/crates/db/src/repos/work_items.rs:130-146`, `control-plane/crates/engine/tests/integration.rs:194`, `:255`, `:922`.
- Northbound readers correctly split full cancellation JSON on single-run reads from summary-only list projections. Evidence: `control-plane/crates/db/src/repos/projections.rs:15-33`, `:59-99`, `:267-305`, `control-plane/crates/graphql-server/src/types/run.rs:5-75`, `control-plane/crates/mcp-server/src/tools/runs.rs:124-136`, `:249-348`, `control-plane/crates/graphql-server/src/schema.rs:699`, `:764`.

### Divergences

- The policy implementation does not realize the proposal's scope-sensitive disposition model. It currently only supports a narrow `Fresh`/`Reused` path plus turn-budget compaction and a generic invalidation branch. Evidence: `control-plane/crates/engine/src/session/policy.rs:67-183`.
- Reuse can still be chosen from DB state alone without confirming that `AcpRuntimeManager` owns the live handle. If the handle is gone, the manager errors instead of failing closed into resume/fresh behavior. Evidence: `control-plane/crates/engine/src/session/policy.rs:86-90`, `control-plane/crates/engine/src/executor.rs:408-419`, `control-plane/crates/acp/src/manager.rs:65-72`, `:143-150`.
- `InvocationOwnerKey` and `BindingFingerprint` are materially narrower than the proposal contract; they do not include stage lineage, task identity, owner execution lineage, prompt text, skill inventory, MCP inventory, output contract, or related binding inputs. Evidence: `control-plane/crates/engine/src/session/fingerprint.rs:4-41`.
- The context-budget layer is still a simple max-turn compaction check. No economic signals, no `BudgetConfig`, and no `Invalidate` path exist. Evidence: `control-plane/crates/engine/src/session/budget.rs:1-19`.
- `ResetSession` does not terminate a generation with reset semantics or create `FreshAfterReset`; it only requeues repair work. Evidence: `control-plane/crates/engine/src/command_handler.rs:412-436`.
- The explicit `proposal-047` gate required by the proposal is absent from both gate script and gate documentation. Evidence: `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md -S` returned no matches.

### Ambiguities / Evidence Gaps

- Execution provenance is persisted, but no production report or recovery path inspected in this audit consumes `session_reuse_disposition` / `session_reset_reason` as the canonical reader truth.
- No checkpoint-backed resume path was proven in the current implementation; `rehydrated_from_checkpoint_artifact_id` persists as schema, but `ReusedAfterResume` behavior was not found.
- The working tree is heavily dirty; this audit is scoped to the inspected implementation surfaces on the current tree and passing focused tests on the current `HEAD`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 0 |
| Missing | 7 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical migration replaces the legacy session-lineage schema without synthetic backfill

- Proposal Source: `§2a` migration contract and `§4` AC11-12 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:87-94`, `:451-453`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1-59`
  - `control-plane/crates/db/tests/integration.rs:21` (`session_lineage_migration_renames_legacy_table_and_creates_canonical_tables`)
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`
- Gap / Note: The migration and current session repos use canonical tables only; no synthetic backfill path was found.

### REQ-002 Workflow compilation carries reuse metadata from catalog to runtime bindings

- Proposal Source: `§1a` reuse-policy catalog contract and `§3` workflow file changes (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:18-32`, `:414-415`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/workflow/src/catalog.rs:102-122`
  - `control-plane/crates/workflow/src/compiler.rs:196-202`
  - `control-plane/crates/workflow/src/compiler.rs:346-355`
  - `control-plane/crates/workflow/tests/integration.rs:66` (`test_parse_agent_catalog`)
  - `control-plane/crates/workflow/tests/integration.rs:97` (`test_compile_full_mvp_live_plan`)
  - `cargo test -p workflow test_parse_agent_catalog -- --exact`
  - `cargo test -p workflow test_compile_full_mvp_live_plan -- --exact`
- Gap / Note: This closes the YAML-to-runtime propagation piece only; the policy semantics themselves are audited separately below.

### REQ-003 Agent executions persist session provenance as execution-first truth

- Proposal Source: `§2d-ii` execution-side provenance and `§4` AC13 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:251-275`, `:455-456`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:48-55`
  - `control-plane/crates/domain/src/agent.rs:40-58`
  - `control-plane/crates/engine/src/executor.rs:335-372`
  - `control-plane/crates/db/src/repos/agent_executions.rs:8-35`
  - `control-plane/crates/db/src/repos/agent_executions.rs:55-141`
  - `control-plane/crates/db/tests/integration.rs:51` (`agent_execution_provenance_round_trips_without_lineage_joins`)
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact`
- Gap / Note: Persistence is present; whether northbound readers actually consume it is a separate requirement and is not satisfied.

### REQ-004 Live ACP session reuse is transport-backed and manager-owned end to end

- Proposal Source: `§2d` ACP session resume / manager ownership and `§4` AC1 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:214-249`, `:440`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:13-74`
  - `control-plane/crates/acp/src/manager.rs:17-154`
  - `control-plane/crates/engine/src/executor.rs:395-419`
  - `control-plane/crates/engine/tests/integration.rs:722` (`test_invoke_agent_reuses_live_session_generation_end_to_end`)
  - `cargo test -p engine test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
- Gap / Note: The runtime owner exists and the happy-path reuse proof passes. Fail-closed handling when that live owner is missing is not implemented and is audited separately.

### REQ-005 Reuse policy implements the full disposition taxonomy and scope-aware matching rules

- Proposal Source: `§2c` reuse policy evaluation and `§4` AC2, AC8, AC9 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:168-213`, `:441-449`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/domain/src/session.rs:27-39`
  - `control-plane/crates/engine/src/session/policy.rs:67-183`
- Gap / Note: The enum exists in the domain model, but the actual policy only emits `Reused` or `Fresh`, with a generic invalidation path and turn-budget compaction. There is no implementation for `FreshAfterReset`, `FreshAfterBudget`, `FreshAfterCompaction`, `FreshAfterTransportError`, `FreshAfterTimeout`, `FreshSessionRequired` as a scope-driven result, `ReusedAfterResume`, or `UnverifiableSessionHistory`.

### REQ-006 DB-active generations without a live runtime handle fail closed into resume or fresh-generation behavior

- Proposal Source: `§2d` required runtime invariant and `§4` AC10 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:230-249`, `:449`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:86-90`
  - `control-plane/crates/engine/src/executor.rs:408-419`
  - `control-plane/crates/acp/src/manager.rs:65-72`
  - `control-plane/crates/acp/src/manager.rs:143-150`
- Gap / Note: The policy decides `should_reuse_live_session = true` from DB lineage state alone. If the manager no longer owns the handle, `execute()` errors with "No live ACP session registered..." instead of falling through to checkpoint-backed resume or a fresh fail-closed path.

### REQ-007 Invocation owner key and binding fingerprint implement the full stable contract

- Proposal Source: `§2b` owner/fingerprint contract and `§4` AC5 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:144-166`, `:444`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:4-41`
  - `control-plane/crates/engine/src/executor.rs:300-314`
- Gap / Note: The current owner key only hashes `run_id`, `agent_id`, `session_reuse_scope`, and `session_family_id`. The current binding fingerprint only hashes provider/model/effort and coarse workspace/worktree settings. It omits stage lineage, task name, owner execution lineage, prompt text, skill injection, IO inventory, backend profile, permission profile, MCP inventory, output contract, and other proposal-locked inputs.

### REQ-008 Context budget evaluation is generation-scoped and economics-driven across hard and economic signals

- Proposal Source: `§1b`, `§2e`, and `§4` AC15-20 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:36-51`, `:277-330`, `:459-465`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:1-19`
  - `control-plane/crates/domain/src/session.rs:54-74`
  - `rg -n "estimated_input_tokens|cached_token_share|normalized_savings_versus_fresh|effective_prompt_size_fraction|compaction_churn_count|idle_age_seconds|BudgetConfig|Invalidate" control-plane/crates/engine/src/session/budget.rs control-plane/crates/domain/src/session.rs control-plane/crates/db/migrations/006_session_lineage.sql -S`
- Gap / Note: The implementation only compacts at 20 turns. No persisted or computed budget signals exist for estimated input tokens, idle age, transcript growth, cache share, normalized savings, or invalidate decisions.

### REQ-009 `ResetSession` ends the current generation with reset semantics and yields `FreshAfterReset` on the next invocation

- Proposal Source: `§2c` disposition model and `§4` AC3 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:173-178`, `:442`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:412-436`
- Gap / Note: `ResetSession` currently marks the stage `Pending`, enqueues `StartupRepair`, and rebuilds projections. It does not end any generation as `reset`, does not insert an `operator_reset` event, and does not populate `session_reset_reason` or force `FreshAfterReset`.

### REQ-010 Cancellation settlement follows the promised two-phase contract and closes live sessions before final cancel

- Proposal Source: `§1c`, `§2f`, and `§4` AC21-23, AC26-27 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:63-68`, `:334-367`, `:467-474`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs:22-168`
  - `control-plane/crates/db/src/repos/runs.rs:121-190`
  - `control-plane/crates/db/src/repos/work_items.rs:130-146`
  - `control-plane/crates/engine/tests/integration.rs:194` (`test_cancel_run_phase1_cancels_agent_executions_and_running_work_items`)
  - `control-plane/crates/engine/tests/integration.rs:255` (`test_cancel_run_eventually_finalizes_to_cancelled`)
  - `control-plane/crates/engine/tests/integration.rs:922` (`test_cancel_run_finalize_closes_live_session_via_runtime_manager`)
  - `cargo test -p engine test_cancel_run_phase1_cancels_agent_executions_and_running_work_items -- --exact`
  - `cargo test -p engine test_cancel_run_eventually_finalizes_to_cancelled -- --exact`
  - `cargo test -p engine test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`
- Gap / Note: This requirement is satisfied for the inspected paths and focused tests.

### REQ-011 Single-run readers expose the full cancellation log while list readers expose summary only

- Proposal Source: `§3` northbound reader wiring and `§4` AC24-25 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:428-432`, `:471-472`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:15-33`
  - `control-plane/crates/db/src/repos/projections.rs:59-99`
  - `control-plane/crates/db/src/repos/projections.rs:267-305`
  - `control-plane/crates/graphql-server/src/types/run.rs:5-75`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:124-136`
  - `control-plane/crates/graphql-server/src/schema.rs:699` (`run_query_exposes_cancellation_settlement_log`)
  - `control-plane/crates/graphql-server/src/schema.rs:764` (`runs_query_exposes_cancellation_settlement_summary_only`)
  - `control-plane/crates/mcp-server/src/tools/runs.rs:249` (`runs_get_returns_cancellation_settlement_log`)
  - `control-plane/crates/mcp-server/src/tools/runs.rs:302` (`runs_list_returns_projection_summary_not_full_log`)
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`
- Gap / Note: This is the one reader-surface contract in P047 that is fully closed on the current tree.

### REQ-012 Report and recovery readers determine reuse disposition from `agent_executions` alone

- Proposal Source: `§2d-ii` execution-first reader contract and `§4` AC14 (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:268-275`, `:457`)
- Status: `Missing`
- Evidence Type: `code`, `inference`
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:36-171`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:32-46`
  - `rg -n "session_reuse_disposition|session_reset_reason|rehydrated_from_checkpoint_artifact_id" control-plane/crates/engine control-plane/crates/mcp-server control-plane/crates/graphql-server control-plane/crates/db -S`
- Gap / Note: Recovery logic operates on runs/stages/work items, not execution provenance. `reports.get` enumerates artifacts and validation payloads only. The search shows persistence sites, but no inspected production reader that answers the proposal's "what happened" question from `agent_executions` alone.

### REQ-013 Proposal 047 is wired into the canonical test gate and gate documentation

- Proposal Source: `§5` test gate (`docs/proposals/047-session-reuse-and-cancellation-settlement.md:478-505`)
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md -S`
- Gap / Note: The search returned no matches. The explicit operationalization required by the proposal does not exist on the current tree.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Runtime reuse still trusts DB state more than live runtime ownership

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `§2d`, `REQ-006`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:86-90`
  - `control-plane/crates/engine/src/executor.rs:408-419`
  - `control-plane/crates/acp/src/manager.rs:65-72`
  - `control-plane/crates/acp/src/manager.rs:143-150`
- Why It Matters: Proposal 047 treats live-session reuse as a two-part invariant: canonical DB lineage plus a currently-owned live transport handle. The current implementation still promotes DB-active generations into reuse before proving runtime ownership, so runtime truth can diverge from persistence truth and produce hard failures instead of fail-closed recovery.
- Recommended Action: Move live-handle verification into policy evaluation or add an explicit runtime-bridge check that downgrades reuse into `ReusedAfterResume` or a fresh-generation path before ACP execution begins.

### ARCH-002 The stable owner/binding identity contract is under-modeled

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§2b`, `REQ-007`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:4-41`
  - `control-plane/crates/engine/src/executor.rs:300-314`
- Why It Matters: Proposal 047 uses owner identity and binding identity as the backbone for safe reuse across retries and loop iterations. The current reduced hash inputs can collapse materially different invocations into the same effective reuse boundary, which is exactly the class of stale-context bug the proposal is trying to eliminate.
- Recommended Action: Expand both builders to the proposal-locked field set and add direct tests for prompt drift, skill drift, task drift, and owner-lineage drift.

## Product Review

**Summary:** At Risk

### PROD-001 Reset and resume are not real operator-facing session actions yet

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§2c`, `§2d`, `REQ-009`, `REQ-012`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:412-436`
  - `control-plane/crates/engine/src/recovery.rs:72-171`
- Why It Matters: From an operator perspective, "reset the session" should produce durable reset truth and a predictable next execution mode. Today it only requeues repair work, so neither the runtime nor the reporting surface can distinguish a clean reset from an incidental retry.
- Recommended Action: End the active generation with reset semantics, persist `session_reset_reason`, insert an `operator_reset` event, and ensure the next policy evaluation emits `FreshAfterReset`.

### PROD-002 The promised economics-driven budget control plane is still absent

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§1b`, `§2e`, `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:1-19`
  - `control-plane/crates/domain/src/session.rs:54-74`
- Why It Matters: The proposal's product value is not "max 20 turns"; it is durable cost-aware session management that knows when reuse is economically irrational or operationally stale. That value is not currently present.
- Recommended Action: Add the missing signal model, persistence fields, evaluation config, and invalidate/compact actions before treating the budget layer as proposal-complete.

## UI Review

**Summary:** Acceptable

### UI-001 Reader payload shape is the only concrete UI-facing contract, and it is correctly split

- Severity: `Note`
- Confidence: `High`
- Related Proposal Items / Requirements: `§3`, `REQ-011`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:267-305`
  - `control-plane/crates/graphql-server/src/types/run.rs:16-19`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:124-136`
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`
- Why It Matters: This preserves drill-down fidelity on single-run inspection without bloating list readers with raw JSON settlement payloads.
- Recommended Action: No P047 change is needed here; keep the UI thin and proposal-scoped.

## UX Review

**Summary:** At Risk

### UX-001 Operator recovery semantics remain unclear because reset/reuse provenance is incomplete

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§2c`, `§2d-ii`, `REQ-009`, `REQ-012`
- Evidence Type: `code`, `inference`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:412-436`
  - `control-plane/crates/engine/src/recovery.rs:72-171`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:32-46`
- Why It Matters: Operators need to understand whether the next attempt is a safe reuse, a deliberate fresh reset, or a fail-closed fallback. The current implementation persists some provenance but does not close the loop into the operator-facing recovery/reporting flow.
- Recommended Action: Make reset/resume dispositions durable in execution records and surface them through recovery/report tooling as the canonical narrative.

## Readiness Review

**Summary:** Weak

### READY-001 Proposal 047 has no canonical gate entrypoint

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§5`, `REQ-013`
- Evidence Type: `code`
- Evidence:
  - `rg -n "proposal-047|p047" scripts/test-gate.sh docs/reference/test-gates.md -S`
- Why It Matters: The proposal explicitly defines how it should be regression-tested. Without that entrypoint, there is no stable operational contract for re-verifying the feature slice after future changes.
- Recommended Action: Add the `proposal-047|p047` case to `scripts/test-gate.sh` and the matching docs entry in `docs/reference/test-gates.md`.

### READY-002 Focused proof passes, but successful-audit gating is not yet available because core requirements are still missing

- Severity: `Note`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005` through `REQ-009`, `REQ-012`, `REQ-013`
- Evidence Type: `tests-run`
- Evidence:
  - `cargo test -p workflow test_parse_agent_catalog -- --exact`
  - `cargo test -p workflow test_compile_full_mvp_live_plan -- --exact`
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact`
  - `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact`
  - `cargo test -p engine test_cancel_run_phase1_cancels_agent_executions_and_running_work_items -- --exact`
  - `cargo test -p engine test_cancel_run_eventually_finalizes_to_cancelled -- --exact`
  - `cargo test -p engine test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
  - `cargo test -p engine test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`
- Why It Matters: The passing focused tests prove the implemented slices, but they do not convert the audit into a green verdict because the proposal contract still has clear missing requirements.
- Recommended Action: Implement the missing requirements first, then rerun the canonical full regression or proposal gate on the same tree before claiming proposal readiness.

## Verification Log

Focused same-tree verification executed because the audit verdict was already non-green:

- `cargo test -p workflow test_parse_agent_catalog -- --exact` -> passed
- `cargo test -p workflow test_compile_full_mvp_live_plan -- --exact` -> passed
- `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact` -> passed
- `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact` -> passed
- `cargo test -p db run_projection_derives_cancellation_settlement_summary_from_canonical_log -- --exact` -> passed
- `cargo test -p engine test_cancel_run_phase1_cancels_agent_executions_and_running_work_items -- --exact` -> passed
- `cargo test -p engine test_cancel_run_eventually_finalizes_to_cancelled -- --exact` -> passed
- `cargo test -p engine test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact` -> passed
- `cargo test -p engine test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact` -> passed
- `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact` -> passed
- `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact` -> passed
- `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact` -> passed
- `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact` -> passed

Full regression was not required or executed because the roll-up was already `Not Implemented` / `Not Ready`.
