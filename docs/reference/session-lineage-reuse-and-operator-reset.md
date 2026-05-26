# Session Lineage, Reuse, and Operator Reset

Stable reference for reusable ACP session lineage, invocation-owner keys, binding fingerprints, reuse policy, context-budget evaluation, live session ownership, checkpoint rehydration, and operator-triggered per-agent reset.

## Purpose

The runtime must be able to reuse a provider session when the same logical agent work continues inside one run, while still keeping execution truth, recovery ownership, and operator control fail-closed.

This document is the contract for:

- session-lineage ownership and reuse boundaries,
- immutable generation and append-only history semantics,
- binding and invocation-owner compatibility checks,
- the `SessionReuseDisposition` taxonomy and policy evaluation,
- `AcpRuntimeManager` live session handle ownership and transport-backed reuse,
- generation-scoped context budget evaluation with hard guardrails and economic signals,
- checkpoint-based fresh rehydration,
- operator-triggered per-agent reset from the existing recovery shell,
- and receipt/report surfaces that expose fresh vs reused vs fresh-after-reset truth.

## Scope

This reference covers:

- reuse within one run for one logical agent owner,
- opt-in family reuse inside one run,
- persisted session lineage / generation / event truth,
- execution-side session provenance on `agent_executions`,
- live ACP session handle ownership,
- budget-driven compaction and invalidation,
- checkpoint persistence before refresh or reset,
- recovery-shell reset and inspection surfaces.

It does not introduce:

- cross-run memory,
- cross-agent session sharing,
- provider-routing redesign,
- or any replacement for persisted artifacts and execution truth as the durable source of truth.

## Core Rules

### Reuse is bounded by immutable ownership

Reuse is allowed only when all of these stay compatible:

- same `runID`,
- same `agentID`,
- same `invocationOwnerKey`,
- same binding fingerprint,
- same allowed reuse scope.

The canonical default scope is `same_invocation_owner`. The only wider scope allowed here is explicit `same_agent_family_within_run`.

### `invocationOwnerKey` construction

`invocationOwnerKey` is the persisted tuple:

```
{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}
```

- `stage_lineage_id` is the stable stage identifier across retries,
- `owner_execution_lineage_id` ties ownership to the execution lineage that created or claimed the generation, so retry-created execution lineages fail closed under `same_invocation_owner` scope.

The tuple is constructed once per enqueue and is immutable on the generation row.

Reuse is not "same agent called again somewhere in the run." It is "same logical invocation owner inside the same run," unless a family-reuse contract explicitly widens it.

### `ownerExecutionLineageID` is imported authority, not session-owned truth

Session reuse reads execution-branch identity from the execution-truth layer documented in [execution-truth-and-recovery.md](execution-truth-and-recovery.md).

Session code may:

- read `ownerExecutionLineageID`,
- persist it inside `invocationOwnerKey`,
- compare it during reuse decisions,
- surface it in inspection/report output.

Session code may not:

- mint new execution lineage,
- repair execution branch truth,
- infer branch identity from session history alone.

If trustworthy owner lineage is missing or contradictory, the runtime must fail closed to a fresh session or an `UnverifiableSessionHistory` surface.

### Binding fingerprint compatibility

Binding fingerprint is a SHA-256 of sorted canonical binding components:

- `agent_id`, `provider`, `model`, `effort`,
- full system prompt text,
- `retry_instruction_sha256` (when P065 guided retry is active),
- working directory, workspace mode,
- worktree write policy / strategy,
- inputs/outputs inventory,
- backend profile, permission profile,
- MCP server inventory,
- skill snapshot hash, `skillRef`, `skillRole`,
- `output_contract`, `max_turns`, `temperature`.

The fingerprint is built at prompt construction time. If the fingerprint changes between loop iterations (prompt modified, MCP inventory changed, workspace mode changed, etc.), reuse is rejected with `FreshSessionRequired`.

Both `invocation_owner_key` and `binding_fingerprint` are write-once on the generation row. Update paths mutate status, end reason, usage counters, and provider session ID — but never these two fields.

### Reuse never replaces durable truth

Even when a provider session is reused:

- artifacts remain canonical,
- receipts remain persisted,
- `AgentExecution` and `StageExecution` remain the durable execution truth,
- reports must remain reconstructable without hidden provider memory.

Session reuse is an execution optimization and continuity aid, not a second truth system.

## Persistence Model

### Canonical tables

Session lineage is persisted in three canonical tables:

- **`session_lineages`**: one row per `(run_id, agent_id, lineage_id)` tuple. Tracks `session_reuse_scope` (`none`, `same_invocation_owner`, `same_agent_family_within_run`), optional `session_family_id`, and `active_generation_id` pointer.
- **`session_generations`**: immutable row per session lifecycle. Captures `invocation_owner_key`, `binding_fingerprint`, `provider_session_id`, runtime/model/working directory/workspace mode, `status` (`active`, `invalidated`, `closed`, `reset`), cumulative usage counters (`turn_count`, `cumulative_prompt_tokens`, `cumulative_cost_cents`), and optional `rehydrated_from_checkpoint_artifact_id`.
- **`session_events`**: append-only event log per generation (`created`, `reused`, `invalidated`, `closed`, `operator_reset`, `budget_exceeded`, `compacted`).

The active pointer may move, but historical rows must not be rewritten into a mutable "latest state" record.

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

## Reuse Policy Evaluation

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

1. No lineage → `Fresh`.
2. Lineage exists, no active generation → map the last ended generation's `end_reason` to the corresponding `FreshAfter*` variant. If a checkpoint exists, `ReusedAfterResume`.
3. Active generation exists:
   - Not found in lineage's generations → `UnverifiableSessionHistory` (fail closed).
   - Fingerprint mismatch → `FreshSessionRequired`.
   - Scope is `none` → `FreshSessionRequired`.
   - Scope is `same_invocation_owner` → owner key mismatch or recovery branch mismatch both yield `FreshSessionRequired`.
   - Scope is `same_agent_family_within_run` → owner key check is relaxed (multiple legitimate owners may share), recovery branch check is relaxed, but fingerprint check remains mandatory.
   - All checks pass → `Reused`.

### Family reuse is opt-in only

`same_agent_family_within_run` exists for adjacent same-agent work where the product deliberately wants continuity wider than one invocation owner.

It still requires:

- same run,
- same agent,
- same `sessionFamilyID`,
- compatible binding fingerprint,
- explicit opt-in from the workflow/catalog path.

Security-, review-, or audit-style agents should remain on `same_invocation_owner` or `none`.

Implementation writer stages are the important exception to the conservative default. `code_writer` work for
initial implementation, continuation, and refinement must opt into `same_agent_family_within_run` with a stable
implementation session family. These stages intentionally build on prior edits, review findings, failed output
repairs, and partial work in the same run-owned worktree. If `code_writer` falls back to `none`, every retry becomes
`fresh_session_required / policy_forbid`; the writer loses continuity and must rediscover the proposal, audit,
prepush report, implementation summary, and worktree before it can make the next targeted edit.

For implementation/refinement retry loops, this is a regression signal, not an acceptable steady state:

- `agent_executions.session_reuse_disposition = fresh_session_required` with `session_reset_reason = policy_forbid`
  for `code_writer`;
- `session_lineages.session_reuse_scope = none` for `(run_id, code_writer)`;
- catalog or frozen catalog snapshots where `code_writer` lacks `session_reuse_scope: same_agent_family_within_run`;
- repeated writer transcripts that start by rereading the full proposal and review bundle instead of continuing from
  the prior implementation context.

Operator repair for an already-active run must update both the live/frozen catalog snapshot and the existing
`session_lineages` row. Updating only `examples/agents/agents.yaml` fixes new runs, but active runs continue to use
their frozen snapshot and persisted lineage policy until those are repaired.

## Live ACP Session Ownership

`AcpRuntimeManager` is the process-lifetime owner of reusable live ACP sessions. It holds an in-memory `active_sessions` map keyed by `session_generation_id` to `ActiveAcpSessionHandle`, which owns the live subprocess, stdio pipes, initialized transport state, provider session ID, and adapter family.

The engine never owns raw ACP subprocess handles. It asks `AcpRuntimeManager` to start a fresh session, submit a prompt into an existing session, close a session, or invalidate and drop a stale handle.

When the engine requests `keep_session_alive`, a live session may remain registered after a `failed` prompt status as well as after `completed`. That failed-session keep-alive exists only for the bounded same-session output repair path documented in [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md#missing-outputs-get-one-same-session-repair-turn). If repair fails, the generation is invalidated and the live handle is closed before normal retry/reuse policy continues.

### Transport-backed reuse invariant

DB lineage truth is necessary but not sufficient for live reuse. `SessionReuseDisposition::Reused` is valid only when **both**:

- the lineage's active generation matches policy checks, **and**
- `AcpRuntimeManager` still holds a matching live `ActiveAcpSessionHandle`.

If DB says a generation is active but no live handle exists, the generation is invalidated and policy re-runs, yielding either `ReusedAfterResume` (checkpoint-backed) or a `FreshAfter*` path.

### Checkpoint rehydration

When disposition is `ReusedAfterResume`, the executor:

1. rehydrates from the last checkpoint artifact,
2. creates a new generation with `rehydrated_from_checkpoint_artifact_id` persisted on `session_generations`,
3. starts a fresh ACP session through `AcpRuntimeManager`,
4. sends `session/prompt` with checkpoint context.

The checkpoint must be persisted only after the primary execution path has validated and persisted the canonical structured outputs it depends on.

## Context Budget Evaluation

`BudgetSignals` aggregates both hard-guardrail and economic inputs from the persisted generation and runtime telemetry.

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

- `ContinueReuse` — all guardrails and economics are within bounds.
- `Compact { reason }` — triggered by turn count, estimated input tokens, prompt size fraction > 0.5, low cached share on large inputs, or transcript growth exceeding the configured ratio (default 2.0×). The current generation is ended, a checkpoint event is recorded, and a new generation starts.
- `Invalidate { reason }` — triggered by cumulative token, cost, or idle guardrails; negative savings versus fresh (reuse 5+ cents more expensive); or compaction churn count ≥ 3. The generation is ended with `budget_exceeded` and `FreshAfterBudget` triggers on next invocation.

Budget evaluation runs **before** sending `session/prompt` on a reused session. Budget signals are read from the persisted generation row and runtime usage snapshots, not reconstructed from events.

## Operator Surfaces

### Reset remains shell-owned

`Reset Agent Session` belongs to the existing recovery spine, not a parallel settings flow. The canonical operator surfaces are:

- the existing recovery coordinator,
- blocked-run and recovery sheet surfaces,
- `AgentSessionInspector`,
- run/report/export surfaces that show session disposition truth.

Notably, GraphQL explicitly does not expose a `resetSession` mutation or any equivalent session reset or control mutation; such operations are restricted to MCP-only interfaces.

### Reset must be deterministic

After operator reset:

- the current lineage/generation is retired through append-only history,
- the next invocation for that owner must start fresh,
- later receipts/reports must show `fresh_after_reset` rather than pretending the lineage continued unchanged.

## Read and Report Order

Readers should prefer:

1. persisted execution-side session provenance on `agent_executions`,
2. persisted lineage / generation / event records,
3. checkpoint and receipt metadata,
4. UI heuristics only as presentation fallback.

Session history must never override execution truth, and reports must not infer reuse from provider receipts alone when canonical session provenance is already persisted.

## Implementation Surface

| File | Role |
|---|---|
| `control-plane/crates/db/migrations/006_session_lineage.sql` | Canonical `session_lineages`/`session_generations`/`session_events`, execution provenance columns |
| `control-plane/crates/db/migrations/007_session_budget_signals.sql` | Budget signal columns on `session_generations` |
| `control-plane/crates/db/migrations/008_session_runtime_usage.sql` | Runtime usage snapshot persistence |
| `control-plane/crates/db/migrations/009_owner_execution_lineage.sql` | Owner execution lineage column |
| `control-plane/crates/domain/src/session.rs` | `SessionLineage`, `SessionGeneration`, `SessionReuseDisposition` |
| `control-plane/crates/domain/src/agent.rs` | Session provenance fields on `AgentExecution` |
| `control-plane/crates/db/src/repos/sessions.rs` | CRUD for lineage, generations, events |
| `control-plane/crates/db/src/repos/agent_executions.rs` | Session provenance persistence |
| `control-plane/crates/engine/src/session/policy.rs` | `SessionReusePolicy::evaluate()`, `ensure_policy()` |
| `control-plane/crates/engine/src/session/budget.rs` | `BudgetSignals`, `BudgetConfig`, `BudgetDecision` |
| `control-plane/crates/engine/src/session/fingerprint.rs` | `InvocationOwnerKeyBuilder`, `BindingFingerprintBuilder` |
| `control-plane/crates/engine/src/executor.rs` | Lineage lookup, policy evaluation, provenance persistence, ACP dispatch |
| `control-plane/crates/engine/src/command_handler.rs` | Operator reset handling |
| `control-plane/crates/acp/src/manager.rs` | `AcpRuntimeManager`, `ActiveAcpSessionHandle`, live session registry |

Swift-side owners (canonical operator shell): `AgentSessionInspector`, `AgentSessionTests`, `RuntimeAgentExecutorTests`.

## Adjacent References

- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) — canonical execution lineage and recovery truth
- [runtime-contract.md](runtime-contract.md) — run snapshots and artifact boundaries
- [provider-binding-truth.md](provider-binding-truth.md) — binding provenance and trust downgrade semantics
- [acp-runtime-transport.md](acp-runtime-transport.md) — ACP transport families and runtime selection
- [run-control.md](run-control.md) — stop/cancel and cancellation-settlement boundary
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) — contract validation and failed-stage evidence
