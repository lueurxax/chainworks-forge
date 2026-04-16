# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | docs/proposals/047-session-reuse-and-cancellation-settlement.md |
| Repository Root | . |
| Git SHA | db7d51aa91f71f898c4e621c01523708ca7d3c1b |
| Working Tree | dirty (existing build/output artifacts and in-flight control-plane changes remain present) |
| Audited At | 2026-04-16T00:01:09+0300 |
| Platform Scope | Rust control-plane |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready |
| Audit Confidence | High |

## Executive Verdict

Current `HEAD` now closes the two remaining R4 proposal-owned blockers. The runtime no longer manufactures `owner_execution_lineage_id` from a fresh per-invoke execution UUID; it derives the owner lineage from the stable `stage_execution_id`, persists that value on `agent_executions`, and proves the resulting same-owner chain in focused integration coverage. The ACP runtime also now surfaces usage snapshots that include `cost_cents`, the executor persists that runtime cost onto `session_generations`, and the next policy cycle can invalidate reuse on real provider cost telemetry rather than only synthesized prompt-size estimates.

The canonical same-tree proof gate was rerun on this tree and passed: `bash ./scripts/test-gate.sh proposal-047`. On current evidence, P047 is implemented and ready.

## Lens Scorecard

| Lens | Assessment | Top Note | Confidence |
|---|---|---|---|
| Conformance | Implemented | All proposal-owned control paths are present and directly evidenced | High |
| Architecture | Ready | Owner lineage is now stable across a stage attempt and diverges correctly on retry via new stage execution identity | High |
| Product | Ready | Live budget invalidation now consumes persisted provider cost when ACP supplies telemetry | High |
| UI | Not Applicable | No direct UI surface is in scope for P047 | High |
| UX | Not Applicable | Reader truth is a control-plane payload contract, not an interactive flow | High |
| Readiness | Ready | The canonical `proposal-047` gate passed on the same tree after the final control-path fixes | High |

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical lineage migration, immutable generations, and append-only events exist in the Rust control plane
- Status: Implemented
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql`
  - `control-plane/crates/domain/src/session.rs`
  - `control-plane/crates/db/src/repos/sessions.rs`
  - `cargo test -p db session_lineage_migration_renames_legacy_table_and_creates_canonical_tables -- --exact`

### REQ-002 Invocation-owner identity and binding identity follow the proposal’s explicit owner chain and fail-closed reuse contract
- Status: Implemented
- Evidence:
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/db/migrations/009_owner_execution_lineage.sql`
  - `cargo test -p engine --test integration test_invoke_agent_uses_stage_execution_id_as_owner_execution_lineage -- --exact`
- Note:
  - The runtime now binds `owner_execution_lineage_id` to the stable `stage_execution_id` for the active stage attempt. Retry creates a new `StageExecutionId`, so retry-branch drift naturally produces a different owner lineage without reopening the old per-invoke UUID problem called out in `R4`.

### REQ-003 Live ACP session reuse, reset, resume, and missing-live-handle fail-closed behavior are implemented through the runtime manager
- Status: Implemented
- Evidence:
  - `control-plane/crates/acp/src/manager.rs`
  - `control-plane/crates/engine/src/session/policy.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `cargo test -p engine --test integration test_invoke_agent_reuses_live_session_generation_end_to_end -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation -- --exact`

### REQ-004 Execution-side session provenance is persisted on `agent_executions` and reader code can answer “what happened” without lineage joins
- Status: Implemented
- Evidence:
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/recovery.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact`

### REQ-005 Context-budget evaluation is generation-scoped and economics-driven in the live runtime path
- Status: Implemented
- Evidence:
  - `control-plane/crates/acp/src/lib.rs`
  - `control-plane/crates/acp/src/session.rs`
  - `control-plane/crates/acp/src/transport.rs`
  - `control-plane/crates/db/migrations/008_session_runtime_usage.sql`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/session/policy.rs`
  - `cargo test -p acp --test integration test_claude_adapter_surfaces_usage_snapshot_from_stream_updates -- --exact`
  - `cargo test -p engine --test integration test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget -- --exact`
- Note:
  - The proposal-owned blocker from `R4` is closed because ACP execution now returns real `cost_cents` when the provider stream supplies them, the executor persists that value on the active generation, and the next policy cycle can invalidate with `FreshAfterBudget` from canonical runtime telemetry. Providers that omit cost telemetry still fall back to the other budget signals, which is a provider-coverage caveat rather than a proposal gap.

### REQ-006 Cancellation settlement follows the promised two-phase contract and cleans up active execution/work-item truth before final cancel
- Status: Implemented
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/db/src/repos/runs.rs`
  - `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --exact`

### REQ-007 Single-run northbound readers expose the full cancellation log while list readers expose only the projection summary
- Status: Implemented
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/graphql-server/src/types/run.rs`
  - `control-plane/crates/mcp-server/src/tools/runs.rs`
  - `control-plane/crates/db/src/repos/projections.rs`
  - `cargo test -p graphql-server schema::tests::run_query_exposes_cancellation_settlement_log -- --exact`
  - `cargo test -p graphql-server schema::tests::runs_query_exposes_cancellation_settlement_summary_only -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_get_returns_cancellation_settlement_log -- --exact`
  - `cargo test -p mcp-server tools::runs::tests::runs_list_returns_projection_summary_not_full_log -- --exact`

### REQ-008 The canonical `proposal-047|p047` proof gate is defined for the control-plane workspace
- Status: Implemented
- Evidence:
  - `docs/reference/test-gates.md`
  - `scripts/test-gate.sh`
  - `bash ./scripts/test-gate.sh proposal-047`

## Delta From R4

- Closed: `owner_execution_lineage_id` is now stable and recovery-branch-safe on the runtime path.
  - Previous `R4` blocker: executor seeded owner lineage from a fresh `AgentExecutionId`.
  - Current evidence:
    - `control-plane/crates/engine/src/executor.rs`
    - `control-plane/crates/domain/src/agent.rs`
    - `control-plane/crates/db/src/repos/agent_executions.rs`
    - `cargo test -p engine --test integration test_invoke_agent_uses_stage_execution_id_as_owner_execution_lineage -- --exact`
- Closed: live budget decisions can now use real provider runtime cost telemetry.
  - Previous `R4` blocker: ACP prompt execution returned no real `cost_cents`, so live economics were forced through estimates.
  - Current evidence:
    - `control-plane/crates/acp/src/lib.rs`
    - `control-plane/crates/acp/src/session.rs`
    - `control-plane/crates/acp/src/transport.rs`
    - `control-plane/crates/engine/src/executor.rs`
    - `control-plane/crates/engine/src/session/policy.rs`
    - `cargo test -p acp --test integration test_claude_adapter_surfaces_usage_snapshot_from_stream_updates -- --exact`
    - `cargo test -p engine --test integration test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget -- --exact`
- Closed: same-tree canonical proof gate was rerun and passed.
  - `bash ./scripts/test-gate.sh proposal-047`

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | The control-plane Rust workspace compiled during the canonical proposal gate rerun |
| Core user flow runtime-validated | Pass | Reuse, reset, resume, missing-live-handle, owner-lineage, runtime-cost budget invalidation, and cancellation settlement all have focused proof |
| Critical tests executed | Pass | Focused ACP/DB/engine/GraphQL/MCP tests plus the full `proposal-047` gate passed |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `bash ./scripts/test-gate.sh proposal-047` passed on current `HEAD` |
| Remaining proposal-owned blockers | Pass | None found on current tree |

## Verification Log

- `cargo test -p acp --test integration test_claude_adapter_surfaces_usage_snapshot_from_stream_updates -- --exact` -> passed
- `cargo test -p db agent_execution_provenance_round_trips_without_lineage_joins -- --exact` -> passed
- `cargo test -p engine --test integration test_invoke_agent_uses_stage_execution_id_as_owner_execution_lineage -- --exact` -> passed
- `cargo test -p engine --test integration test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget -- --exact` -> passed
- `bash ./scripts/test-gate.sh proposal-047` -> passed

## Recommended Next Action

Promote `R5` as the current audit baseline and treat `R4` as superseded. No remaining proposal-owned implementation work is required for P047 on current `HEAD`.
