# Session Lineage, Context Budget, and Cancellation Settlement (former P047)

## Status
- **Implemented**: 2026-04-16
- **Primary contract owners**: Rust control-plane engine/session/budget/cancellation modules
- **Evidence source**: `./scripts/test-gate.sh proposal-047`

## Purpose

This document replaces the proposal-level text for session lineage, context budget, and cancellation settlement. It defines the production contract for durable session lineage with immutable generations, invocation owner keys, binding fingerprints, generation-scoped context budget evaluation, two-phase cancellation settlement, and northbound reader projections in the Rust control plane.

The goal of this slice is:

- the daemon persists session lineage truth per run/agent pair and reuses sessions safely across loop iterations,
- binding fingerprint and invocation owner key verification keeps reuse fail-closed,
- `AcpRuntimeManager` is the process-lifetime owner of live ACP session handles,
- context budget evaluation is generation-scoped and economics-driven, not prompt-size heuristic,
- cancellation settles through a two-phase `Cancelling` to `Cancelled` contract with durable evidence,
- and northbound readers expose the right granularity (full settlement log for single-run, summary for list).

## Scope

This reference covers:

- durable session lineage, generation, and event persistence,
- invocation owner key and binding fingerprint construction and verification,
- reuse policy evaluation and the `SessionReuseDisposition` taxonomy,
- `AcpRuntimeManager` live session handle ownership and transport-backed reuse,
- execution-side session provenance on `agent_executions`,
- generation-scoped context budget evaluation with hard guardrails and economic signals,
- two-phase cancellation settlement with `CancellationSettlementEntry`,
- `WorkItemStatus::Cancelled` variant,
- and the canonical/list northbound reader split for cancellation settlement.

It does not replace:

- the broader session-lineage reuse and operator-reset contract in [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md),
- the run-control stop/archive/cancellation boundary in [run-control.md](run-control.md),
- the execution truth and recovery precedence in [execution-truth-and-recovery.md](execution-truth-and-recovery.md),
- the ACP runtime transport contract in [acp-runtime-transport.md](acp-runtime-transport.md),
- post-approval task execution in [044-post-approval-task-execution-and-release-gate-completion.md](044-post-approval-task-execution-and-release-gate-completion.md),
- or the proof inventory in [test-gates.md](test-gates.md).

## Core Rules

### Durable session lineage

Session lineage is persisted in three canonical tables created by the `006_session_lineage.sql` migration:

- **`session_lineages`**: one row per `(run_id, agent_id, lineage_id)` tuple. Tracks `session_reuse_scope` (`none`, `same_invocation_owner`, `same_agent_family_within_run`), optional `session_family_id`, and `active_generation_id` pointer.
- **`session_generations`**: immutable row per session lifecycle. Captures `invocation_owner_key`, `binding_fingerprint`, `provider_session_id`, runtime/model/working directory/workspace mode, `status` (`active`, `invalidated`, `closed`, `reset`), cumulative usage counters (`turn_count`, `cumulative_prompt_tokens`, `cumulative_cost_cents`), and optional `rehydrated_from_checkpoint_artifact_id`.
- **`session_events`**: append-only event log per generation (`created`, `reused`, `invalidated`, `closed`, `operator_reset`, `budget_exceeded`, `compacted`).

The legacy projection-era `session_lineages` table (from `002_projections.sql`) is renamed to `session_lineages_legacy` by the migration. No synthetic generation backfill is performed from legacy rows. Policy, budget, and reader code reads only from the canonical tables.

### Invocation owner key and binding fingerprint

**`InvocationOwnerKeyInput`** builds the owner tuple: `{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}`. It is constructed once per enqueue and immutable on the generation. `stage_lineage_id` is the stable stage identifier across retries. `owner_execution_lineage_id` ties ownership to the execution lineage that created or claimed the generation, so retry-created execution lineages fail closed under `same_invocation_owner` scope.

**Binding fingerprint** is a SHA-256 of sorted canonical binding components: `agent_id`, `provider`, `model`, `effort`, full system prompt text, working directory, workspace mode, worktree write/strategy, inputs/outputs inventory, backend profile, permission profile, MCP server inventory, skill snapshot hash, `skillRef`, `skillRole`, `output_contract`, `max_turns`, and `temperature`. Built at prompt construction time. If the fingerprint changes between loop iterations (prompt modified, MCP inventory changed, etc.), reuse is rejected with `FreshSessionRequired`.

Both fields are write-once on the generation row. Update paths mutate status, end reason, usage counters, and provider session ID, but never `invocation_owner_key` or `binding_fingerprint`.

### Reuse policy evaluation

`session::policy::evaluate()` produces a `SessionReuseDisposition`:

| Disposition | Meaning |
|---|---|
| `Fresh` | Cold start, no lineage exists |
| `Reused` | Active generation matched all criteria |
| `ReusedAfterResume` | Resumed from checkpoint after prior close |
| `FreshAfterReset` | Operator reset via `ResetSession` command |
| `FreshAfterInvalidation` | Generic invalidation (no specialized reason) |
| `FreshAfterBudget` | Budget guardrail triggered invalidation |
| `FreshAfterCompaction` | Compaction event on prior generation |
| `FreshAfterTransportError` | Transport failure on prior generation |
| `FreshAfterTimeout` | Timeout on prior generation |
| `FreshSessionRequired` | Binding fingerprint mismatch or scope rejection |
| `UnverifiableSessionHistory` | Active generation not found in lineage; fail closed |

Policy logic:

1. No lineage: `Fresh`.
2. Lineage exists, no active generation: map the last ended generation's `end_reason` to the corresponding `FreshAfter*` variant. If a checkpoint exists, `ReusedAfterResume`.
3. Active generation exists:
   - Not found in lineage's generations: `UnverifiableSessionHistory` (fail closed).
   - Fingerprint mismatch: `FreshSessionRequired`.
   - Scope is `none`: `FreshSessionRequired`.
   - Scope is `same_invocation_owner`: owner key mismatch or recovery branch mismatch both yield `FreshSessionRequired`.
   - Scope is `same_agent_family_within_run`: owner key check is relaxed (multiple legitimate owners may share), recovery branch check is relaxed, but fingerprint check remains mandatory.
   - All checks pass: `Reused`.

### ACP live session reuse

`AcpRuntimeManager` is the process-lifetime owner of reusable live ACP sessions. It holds an in-memory `active_sessions` map keyed by `session_generation_id` to `ActiveAcpSessionHandle`, which owns the live subprocess, stdio pipes, initialized transport state, provider session ID, and adapter family.

The engine never owns raw ACP subprocess handles. It asks `AcpRuntimeManager` to start a fresh session, submit a prompt into an existing session, close a session, or invalidate and drop a stale handle.

**Transport-backed reuse invariant**: DB lineage truth is necessary but not sufficient for live reuse. `SessionReuseDisposition::Reused` is valid only when both the lineage's active generation matches policy checks and `AcpRuntimeManager` still holds a matching live `ActiveAcpSessionHandle`. If DB says a generation is active but no live handle exists, the generation is invalidated and policy re-runs, yielding either `ReusedAfterResume` (checkpoint-backed) or a `FreshAfter*` path.

**Checkpoint resume**: when disposition is `ReusedAfterResume`, the executor rehydrates from the last checkpoint artifact, creates a new generation with `rehydrated_from_checkpoint_artifact_id` persisted on `session_generations`, starts a fresh ACP session through `AcpRuntimeManager`, and sends `session/prompt` with checkpoint context.

### Execution-side session provenance

Eight columns on `agent_executions` persist the session provenance snapshot after policy evaluation and before ACP session start:

- `session_lineage_id`
- `session_generation_id`
- `invocation_owner_key`
- `session_reuse_scope`
- `session_family_id`
- `session_reuse_disposition`
- `session_reset_reason`
- `rehydrated_from_checkpoint_artifact_id`

Report builders, recovery readers, and comparison surfaces read `agent_executions` as the primary truth for what happened during an execution. Lineage tables are the "why it happened" history; the execution record is the "what happened" surface. No lineage table join is required for disposition truth.

### Context budget evaluation

`BudgetSignals` aggregates both hard-guardrail and economic inputs from the persisted generation and runtime telemetry:

**Hard guardrails** (from persisted generation state):
- Turn count (`max_turns` default 20)
- Estimated input tokens (`max_estimated_input_tokens` default 128,000)
- Cumulative prompt tokens (`max_cumulative_prompt_tokens` default 1,000,000)
- Cumulative cost in cents (`max_cumulative_cost_cents` default 500)
- Idle age in seconds (`max_idle_age_seconds` default 14,400 / 4h)

**Economic signals** (from provider runtime metadata):
- Transcript growth ratio (current input vs fresh-session baseline)
- Cached token share (fraction of input tokens cached by provider)
- Normalized savings versus fresh (net cost difference; positive means reuse is cheaper)
- Effective prompt size fraction (fraction of context window used)
- Compaction churn count (prior compaction events on this generation)

`BudgetDecision` is one of:
- `ContinueReuse` -- all guardrails and economics are within bounds.
- `Compact { reason }` -- triggered by turn count, estimated input tokens, prompt size fraction > 0.5, low cached share on large inputs, or transcript growth exceeding the configured ratio (default 2.0x). The current generation is ended, a checkpoint event is recorded, and a new generation starts.
- `Invalidate { reason }` -- triggered by cumulative token, cost, or idle guardrails; negative savings versus fresh (reuse 5+ cents more expensive); or compaction churn count >= 3. The generation is ended with `budget_exceeded` and `FreshAfterBudget` triggers on next invocation.

Budget evaluation runs before sending `session/prompt` on a reused session. Budget signals are read from the persisted generation row and runtime usage snapshots, not reconstructed from events.

### Two-phase cancellation settlement

**Phase 1** (`begin_settlement`, synchronous in `CancelRun`):

1. `run.cancellation_requested_at` is set.
2. All active agent executions (`Running`/`Pending`/`Ready`) transition to terminal `Cancelled`.
3. A `CancellationSettlementEntry` is built per agent execution with `session_close_succeeded: None`.
4. Entries are serialized as JSON into `run.cancellation_settlement_log`.
5. All `Running` work items are marked `Cancelled` (`WorkItemStatus::Cancelled`).
6. All `Running` stages with terminal agent executions are marked `Failed`.
7. Run status stays `Cancelling`.

**Async session close** (background task after Phase 1):

Per-session: SIGTERM to the ACP subprocess, wait up to 10s timeout, SIGKILL if needed. Returns `SessionCloseOutcome { session_id, attempted, succeeded }` per session.

**Phase 2** (`finalize_settlement`, after async close completes):

1. Read preliminary settlement entries from `cancellation_settlement_log`.
2. Update each entry with actual `session_close_succeeded` from close outcomes.
3. Re-serialize entries to `run.cancellation_settlement_log`.
4. `run.cancellation_settled_at` is set.
5. `run.status` transitions to `Cancelled`.

Only after Phase 2 does the run appear as `Cancelled` to operator and report readers.

**`CancellationSettlementEntry`** carries: `agent_execution_id`, `agent_id`, `prior_status`, `terminal_status` (`"cancelled"`), `session_close_attempted`, `session_close_succeeded` (`None` in Phase 1, `Some` in Phase 2), and `settled_at`.

### Northbound reader split

Single-run readers (GraphQL `run(id)`, MCP `runs.get`) read the canonical `Run` and expose the full `cancellation_settlement_log` JSON and `cancellation_settled_at`.

List readers (GraphQL `runs`, MCP `runs.list`) read `RunProjectionRow` and expose only `cancellation_settlement_summary` -- a human-readable one-line string derived during projection rebuild (e.g., `"3/3 agents settled, 2 sessions closed"`). The full JSON log is not projected into list rows.

## Proof Obligations

The canonical same-tree proof lane for this slice is:

```bash
./scripts/test-gate.sh proposal-047
```

That gate runs `cargo test --workspace` from `control-plane` and targets these focused tests (among others):

- `test_runtime_manager_reuses_live_session_handle` -- `AcpRuntimeManager` live handle reuse
- `test_invoke_agent_reuses_live_session_generation_end_to_end` -- end-to-end reuse through a live session generation
- `test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact` -- checkpoint resume with provenance
- `test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation` -- missing handle fail-closed behavior
- `test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget` -- cost-budget invalidation round-trip
- `test_cancel_run_finalize_closes_live_session_via_runtime_manager` -- two-phase cancellation through live session close
- `test_claude_adapter_surfaces_usage_snapshot_from_stream_updates` -- runtime usage snapshot persistence

The gate runs the full Rust workspace test suite to catch regressions in adjacent orchestrator, settlement, session, and reader logic.

## Implementation Surface

| File | Role |
|---|---|
| `control-plane/crates/db/migrations/006_session_lineage.sql` | Legacy rename, canonical `session_lineages`/`session_generations`/`session_events` creation, execution provenance columns, cancellation columns |
| `control-plane/crates/db/migrations/007_session_budget_signals.sql` | Budget signal columns on `session_generations` |
| `control-plane/crates/db/migrations/008_session_runtime_usage.sql` | Runtime usage snapshot persistence |
| `control-plane/crates/db/migrations/009_owner_execution_lineage.sql` | Owner execution lineage column |
| `control-plane/crates/domain/src/session.rs` | `SessionLineage`, `SessionGeneration`, `SessionReuseDisposition` domain types |
| `control-plane/crates/domain/src/agent.rs` | Session provenance fields on `AgentExecution` |
| `control-plane/crates/domain/src/run.rs` | `cancellation_settlement_log` on `Run` |
| `control-plane/crates/db/src/repos/sessions.rs` | CRUD for lineage, generations, events |
| `control-plane/crates/db/src/repos/agent_executions.rs` | Session provenance persistence on execution records |
| `control-plane/crates/db/src/repos/work_items.rs` | `WorkItemStatus::Cancelled` variant and cancellation cleanup |
| `control-plane/crates/db/src/repos/runs.rs` | Settlement log and settled-at persistence |
| `control-plane/crates/db/src/repos/projections.rs` | `cancellation_settlement_summary` projection |
| `control-plane/crates/engine/src/session/policy.rs` | `SessionReusePolicy::evaluate()`, `ensure_policy()` |
| `control-plane/crates/engine/src/session/budget.rs` | `BudgetSignals`, `BudgetConfig`, `BudgetDecision`, `evaluate()` |
| `control-plane/crates/engine/src/session/fingerprint.rs` | `InvocationOwnerKeyBuilder`, `BindingFingerprintBuilder` |
| `control-plane/crates/engine/src/executor.rs` | Lineage lookup, policy evaluation, provenance persistence, ACP session dispatch |
| `control-plane/crates/engine/src/cancellation.rs` | `begin_settlement()`, `close_runtime_sessions()`, `finalize_settlement()` |
| `control-plane/crates/engine/src/command_handler.rs` | Operator reset handling, `CancelRun` phase 1 entry |
| `control-plane/crates/acp/src/manager.rs` | `AcpRuntimeManager`, `ActiveAcpSessionHandle`, live session registry |
| `control-plane/crates/graphql-server/src/schema.rs` | Single-run vs list reader routing for settlement |
| `control-plane/crates/graphql-server/src/types/run.rs` | `cancellation_settlement_log` and `cancellation_settlement_summary` on `GqlRun` |
| `control-plane/crates/mcp-server/src/tools/runs.rs` | `runs.get` full log, `runs.list` summary only |

## Archived Proposal History

The proposal draft, consolidated review, evidence pack, and implementation audit
rounds R1 through R6 for P047 are retained under
[../archive/proposals/README.md](../archive/proposals/README.md) for provenance
only. They are not the canonical current-head contract anymore.

## Related Stable Docs

- [runtime-contract.md](runtime-contract.md)
- [rust-control-plane.md](rust-control-plane.md)
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)
- [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md)
- [044-post-approval-task-execution-and-release-gate-completion.md](044-post-approval-task-execution-and-release-gate-completion.md)
- [test-gates.md](test-gates.md)
