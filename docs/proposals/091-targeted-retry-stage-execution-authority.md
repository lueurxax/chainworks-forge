# Proposal 091: Targeted Retry Stage Execution Authority

| Field | Value |
|---|---|
| Date | 2026-05-13 |
| Status | Draft |
| Author | Codex |
| Depends on | [rust-control-plane.md#capacity-aware-scheduling-and-backpressure](../reference/rust-control-plane.md#capacity-aware-scheduling-and-backpressure), P065 operator retry instruction bindings, P083 execution-truth ownership invariants |
| Related | P086 agent continuation, [execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md), `docs/reference/rust-control-plane.md`, `docs/reference/workflow-execution-engine.md` |
| Scope | Fix the retry lineage defect where `RetryStage` creates a new `stage_execution`, but subsequent orchestration continues at `stage_id` scope and can strand or supersede the intended retry attempt. |
| Non-goal | No change to proposal/implementation business logic, no redefinition of approval semantics, no broad workflow transition redesign, and no silent retroactive healing without explicit recovery rules. |

## 1. Problem

`P086 / 976f3d1b-31d8-43ef-a3d7-c9940c7086ab` exposed a retry-truth defect in the control plane:

1. operator-issued `RetryStage` completed successfully;
2. the retry created a fresh `stage_execution` for `state_9_implementation_reviewed`;
3. that fresh execution remained `pending`;
4. the run later reflected a different sibling execution of the same `stage_id`;
5. no live work items or agent executions remained for the orphaned `pending` stage;
6. run truth and stage summaries continued to treat that orphan as authoritative blocked state.

This is not a code-writer failure and not a provider timeout.
It is a lineage/authority bug in the way the engine handles stage retries.

The expensive effect is that an operator can request a retry of a concrete stage attempt, but the runtime only preserves logical `stage_id` authority, not the concrete `stage_execution_id` authority of the new retry attempt.

## 2. Observed Evidence Baseline

For `P086`, the following facts are already proven:

- `RetryStage` command journal entry completed successfully:
  - `96ad2abe-e906-4422-a0d3-2f7a30912f0a`
- the retry created:
  - `stage_execution_id = 780a94ce-8ae6-4b0c-99c6-eb79118ee640`
  - `stage_id = state_9_implementation_reviewed`
  - `status = pending`
  - `attempt_number = 8`
- the run also has another execution for the same state:
  - `36ce1846-8d80-4ebf-afb3-7ba6fcaa75ad`
  - `status = completed`
- there are no live `InvokeAgent` or other work items for `780a94ce...`
- there are no active agent executions for `780a94ce...`
- `stage_summaries` still surface `780a94ce...` as `pending`
- the run remains `blocked` on `state_9_implementation_reviewed`

This proves the system is preserving an execution-level ghost state after retry.

P091 must preserve this local evidence as a deterministic fixture before implementation:

- `docs/evidence/091/targeted-retry-authority/p086-orphaned-retry-readback.fixture.json`
- source diagnostic paths:
  - `.chainworks/runs/976f3d1b-31d8-43ef-a3d7-c9940c7086ab/state/run-state.json`
  - `.chainworks/runs/976f3d1b-31d8-43ef-a3d7-c9940c7086ab/artifacts/active-index.json`
  - `.chainworks/runs/976f3d1b-31d8-43ef-a3d7-c9940c7086ab/review/implementation-summary.json`

If those local diagnostics are incomplete or later unavailable, the implementation gate must recreate the shape synthetically and mark the historical P086 evidence as `diagnostic_only`.

## 3. Current Root Cause

The bug comes from a mismatch between retry creation semantics and orchestration authority semantics.

### 3.1 `RetryStage` creates a concrete new stage execution

The command handler already creates a fresh retry attempt as a concrete `StageExecution` row with a new UUID.

### 3.2 `RetryStage` enqueues `AdvanceRun` only by logical state

After creating the retry attempt, the command handler enqueues an `AdvanceRun` work item whose payload includes only:

- `run_id`
- `stage_id`

It does not carry the newly created retry attempt's `stage_execution_id`.

### 3.3 `advance_run` resolves work at state scope, not retry-attempt scope

The orchestrator then:

- loads all stages for the run;
- filters by `stage_id`;
- chooses the "current" stage for that state using list order / latest-state heuristics.

That means the retry is not anchored to the specific execution the operator just created.

### 3.4 Sibling stage executions of the same state can steal authority

Once more than one `stage_execution` exists for the same `stage_id`, state-level orchestration can:

- activate the wrong execution,
- settle the wrong execution,
- leave the intended retry orphaned as `pending`,
- let summaries and blocked truth point at the wrong sibling.

## 4. Goals

- Make `RetryStage` authoritative for one concrete retry attempt, not only for a logical state name.
- Ensure `AdvanceRun` created by a retry continues exactly the retry attempt the operator chose.
- Prevent sibling executions of the same `stage_id` from taking over retry authority.
- Ensure projections and run truth reflect the targeted retry execution while it is authoritative.
- Add deterministic recovery for already orphaned `pending` or `running` retry attempts.

## 5. Non-Goals

- Do not redesign ordinary workflow transitions.
- Do not remove state-level orchestration for non-retry flows.
- Do not change auto-retry semantics unrelated to concrete retry-attempt ownership.
- Do not rewrite stage loop semantics or iteration numbering.
- Do not treat every `pending` stage as invalid; only orphaned retry attempts are in scope.

## 6. Alternatives Considered

### Option A: Keep state-level `AdvanceRun` and try to repair projections only

Rejected because the root defect is not projection-only.
The engine can actually continue the wrong execution.
Projection repair alone would hide the bug, not remove it.

### Option B: Infer the intended retry attempt from "latest stage for state"

Rejected because this is the current failure mode.
Once multiple executions exist for the same `stage_id`, "latest by state" is not a sufficient retry authority model.

### Option C: Make `RetryStage` and its resulting `AdvanceRun` carry explicit `target_stage_execution_id`

Recommended.
This preserves intent from command creation through orchestration and recovery.

## 7. Decision

Adopt targeted retry authority:

1. `RetryStage` creates one concrete retry attempt.
2. A durable retry-authority row records that retry attempt as the active authority for `(run_id, stage_id)`.
3. Full-stage retry starts `AdvanceRun`-first; targeted-agent retry starts `InvokeAgent`-first.
4. Every follow-on `AdvanceRun` that belongs to the retry carries `target_stage_execution_id` and `retry_authority_id`.
5. `advance_run` enters targeted mode when that authority is present or can be resolved from the completed invoke work item.
6. Targeted mode gives the referenced retry execution authority over sibling executions of the same state.
7. Recovery and projections recognize active retry authority and clean only orphaned retry attempts that are not legitimate waits.

## 8. Proposed Design

### 8.1 Add durable retry-authority records

P091 introduces a durable retry-authority record. It cannot rely only on transient work item JSON, because projections and startup recovery must be able to reconstruct authority after the work item has completed, failed, or been pruned.

Minimum table shape:

```sql
CREATE TABLE retry_stage_execution_authorities (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  target_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  entry_kind TEXT NOT NULL,
  source_command_journal_id TEXT,
  source_retry_work_item_id TEXT,
  source_invoke_work_item_id TEXT,
  source_agent_execution_id TEXT,
  authority_state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  terminal_reason TEXT
);

CREATE UNIQUE INDEX retry_stage_execution_authorities_one_active
ON retry_stage_execution_authorities(run_id, stage_id)
WHERE authority_state = 'active';

CREATE INDEX retry_stage_execution_authorities_target
ON retry_stage_execution_authorities(target_stage_execution_id);
```

`authority_state` values:

- `active`
- `terminalized`
- `superseded`
- `recovered_orphan`
- `invalid`

`entry_kind` values:

- `full_stage_retry`
- `targeted_agent_retry`
- `historical_orphan_recovery`

There must be at most one `active` authority per `(run_id, stage_id)`. This is not only an application invariant: it must be enforced by a DB partial unique index or an equivalent transaction-level constraint. A new operator retry for the same state supersedes the previous active authority before creating the next one.

Concurrent retry attempts for the same `(run_id, stage_id)` must use one transaction that supersedes the old active row and inserts the new active row. If two commands race, one must win and the other must fail or retry against the new active authority; the system must not leave duplicate active authority rows.

P065 `retry_operator_instruction_bindings.retry_stage_execution_id` remains an optional instruction binding, not the general authority source. P091 may link to it when present, but must not require it.

### 8.2 Extend `AdvanceRun` payload with target authority

When full-stage `RetryStage` creates `new_stage.id`, the enqueued `AdvanceRun` payload must include:

- `run_id`
- `stage_id`
- `target_stage_execution_id`
- `retry_authority_id`

`target_stage_execution_id` and `retry_authority_id` are optional for backward compatibility, but mandatory for retry-created `AdvanceRun` items after P091.

Targeted-agent retry is different: it already creates a `running` stage and enqueues `InvokeAgent` directly. P091 must preserve that lifecycle. For targeted-agent retry, the transaction must create:

- the target `StageExecution`;
- the active retry-authority row with `entry_kind = targeted_agent_retry`;
- the targeted `InvokeAgent` work item carrying `target_stage_execution_id` and `retry_authority_id`.

The first `AdvanceRun` for targeted-agent retry is the post-invoke follow-up, not the initial wake.

### 8.3 Propagate target authority after invoke completion/failure

Target authority must survive the first retry wake. The following follow-up enqueue paths must preserve or recover the target:

- `RetryStage` full-stage retry initial `AdvanceRun`;
- targeted-agent retry that creates a running stage and a targeted `InvokeAgent`;
- post-`InvokeAgent` completion `AdvanceRun`;
- post-`InvokeAgent` failure `AdvanceRun`;
- recovery requeue of abandoned `AdvanceRun` work that belongs to an active retry authority.

Propagation rule:

1. `InvokeAgent` work items created under targeted retry authority must carry `target_stage_execution_id` and `retry_authority_id`.
2. When an invoke settles, the follow-up `AdvanceRun` must copy those fields from the source invoke work item.
3. If the source invoke payload is legacy or incomplete, executor must resolve authority from `agent_executions.stage_execution_id` plus the active `retry_stage_execution_authorities` row.
4. If resolution finds no active authority, the follow-up `AdvanceRun` must remain legacy state-level, not silently invent a target.
5. If resolution finds conflicting active authorities, the follow-up must fail closed with a typed scheduler error and must not fall back to state-level settlement.

Acceptance must prove both full-stage retry and targeted-agent retry settle the intended `stage_execution_id` after provider completion and after provider failure.

### 8.4 Target-aware work-item repository semantics

P091 must update work-item repository helpers so targeted advances are never reduced back to run scope.

Required repository behavior:

- `AdvanceRunPayloadV1` is the source of truth for target fields, not nullable `work_items.stage_id`;
- claim, requeue, cancel, and abandoned-work recovery helpers preserve the full JSON payload;
- helpers that currently operate only by `run_id` must gain target-aware variants or explicitly filter by parsed `target_stage_execution_id` and `retry_authority_id`;
- post-invoke `AdvanceRun` items may have `work_items.stage_id = NULL`, but their typed payload must still include `stage_id`, `target_stage_execution_id`, `retry_authority_id`, and `source_stage_execution_id`;
- cancel/requeue of a targeted retry must affect only work tied to the same authority, not every same-run `AdvanceRun`.

If typed payload parsing fails during repository recovery, the item must be quarantined with a typed payload error instead of being replayed as a run-scoped legacy advance.

### 8.5 Add targeted mode to `advance_run`

If `target_stage_execution_id` is present:

1. load that exact `StageExecution`;
2. verify it belongs to the supplied `run_id`;
3. verify it belongs to the supplied `stage_id`;
4. verify the active `retry_authority_id` points to the same target when present;
5. use that exact execution as the active subject for orchestration logic.

In targeted mode, sibling executions of the same `stage_id` must not displace or overshadow the target.

If `target_stage_execution_id` is absent, the engine may continue using legacy state-level selection.

### 8.6 Change current-stage resolution precedence

The current precedence must become:

1. explicit `target_stage_execution_id` plus active `retry_authority_id`, if present and valid;
2. active durable retry authority for `(run_id, stage_id)`, when the current work item can be linked to it;
3. otherwise legacy state-level latest-stage heuristics.

This rule applies to:

- stage activation
- stage settlement
- stage in-flight checks
- run blocked/running truth for the targeted retry

### 8.7 Preserve event truth at execution scope

During targeted retry orchestration:

- `StageStatusChanged` events must refer to the target execution;
- `RunStatusChanged` must reflect the target execution's actual lifecycle;
- stage-status rebuilds must not promote a sibling execution over a non-terminal target execution.

### 8.8 Stage summary selection rules

For a state with multiple executions:

- if an active durable retry authority points at a non-terminal target execution, it is the authoritative active attempt for summaries;
- otherwise, existing terminal/latest rules may apply.

This prevents a stale sibling from reappearing as the current state truth.

Projection rebuild must read durable authority records. It must not depend on a live `AdvanceRun` work item being present.

### 8.9 Recovery for orphaned retry attempts

Add a repair path for already-broken data:

An execution qualifies as an orphaned retry candidate when all are true:

- `status in (pending, running)`
- no live `work_items` reference its `stage_execution_id`
- no active `agent_executions` belong to it
- a sibling execution of the same `stage_id` is newer and settled, or the run already moved on
- no durable retry authority is active for that execution

Such an orphan must be deterministically settled as:

- `status = skipped`
- `terminal_reason = stale_retry_recovered`

Recovery must create a durable provenance row even when the orphan predates P091 and has no authority row:

- `entry_kind = historical_orphan_recovery`
- `authority_state = recovered_orphan`
- `target_stage_execution_id = <orphan execution id>`
- `terminal_reason = stale_retry_recovered`
- source fields populated from the recovery pass and any historical command/work-item evidence that exists

This row is not active and must not participate in current-stage selection. It exists so GraphQL/MCP/report readback can explain why a historical retry stopped holding blocked truth.

Then:

- rebuild `stage_summaries`
- rebuild run summary/readback projections

### 8.10 Recovery exclusions: legitimate waits must not be mutated

The orphan repair pass must explicitly exclude valid waiting states. It must not settle a pending/running stage when any of these are true:

- a pending approval/manual gate exists for the run or stage;
- unresolved side effects exist in the side-effect ledger for the run/stage;
- a transition cursor or startup catch-up marker indicates the run is intentionally parked;
- recovery/readback state marks the run as in recovery, backpressured, or intentionally queued;
- a live or backpressured work item exists for the run/stage even if it is not currently executing;
- provider capacity/backpressure is the only reason work has not started;
- the target stage is the current active durable retry authority;
- a retry-after/quota/backoff record is still active for the associated agent execution;
- the stage has a durable recovery snapshot whose recommended next action is wait, approve, or retry-later rather than settle.

Negative acceptance tests must prove these exclusions keep legitimate pending/waiting retries unchanged.

### 8.11 Compatibility rules

Historical `AdvanceRun` work items without `target_stage_execution_id` must remain processable.

P091 therefore introduces:

- new targeted behavior when the field is present;
- backward-compatible legacy behavior when absent.

### 8.12 Startup repair integration

Startup recovery must include the orphaned retry repair pass before preserving blocked truth.

Required startup ordering:

1. load persisted run/stage/work-item/agent-execution state;
2. run orphaned retry detection and recovery for P091 candidates;
3. write `status = skipped`, `terminal_reason = stale_retry_recovered`, and the non-active recovered authority provenance row for each confirmed orphan;
4. rebuild `stage_summaries`, run summary/readback projections, and authority history projections from the post-repair state;
5. only then enqueue generic `startup_catchup` / abandoned-run `AdvanceRun` work.

Generic startup catch-up must not enqueue a run-scoped `AdvanceRun` for a run whose only blocker was a recovered P091 orphan. If catch-up still needs to enqueue work for the same run after P091 repair, it must use the rebuilt projection state and must not resurrect the recovered stage execution.

This is necessary so runs like `P086` do not remain permanently blocked on an abandoned retry execution after daemon restart or recovery.

## 9. Data Model and Contract Changes

### 9.1 Typed `AdvanceRunPayload` contract

P091 must replace ad hoc parsing of `AdvanceRun` payload JSON with a typed contract used by command handler, work queue, executor, recovery, and tests.

`AdvanceRunPayloadV1`:

```json
{
  "schema_version": "advance_run_payload.v1",
  "run_id": "<uuid>",
  "stage_id": "state_9_implementation_reviewed",
  "target_stage_execution_id": "<uuid-or-null>",
  "retry_authority_id": "<uuid-or-null>",
  "source_work_item_id": "<uuid-or-null>",
  "source_stage_execution_id": "<uuid-or-null>",
  "source_invoke_work_item_id": "<uuid-or-null>",
  "enqueue_reason": "retry_stage"
}
```

Allowed `enqueue_reason` values:

- `normal_advance`
- `retry_stage`
- `targeted_agent_retry`
- `post_invoke_completion`
- `post_invoke_failure`
- `startup_recovery`
- `abandoned_advance_requeue`

Parse/error semantics:

| Case | Behavior |
|---|---|
| malformed JSON | fail the work item with typed `advance_run_payload_malformed`; do not fall back to state-level behavior |
| missing `run_id` | fail the work item with typed `advance_run_payload_missing_run_id` |
| missing `stage_id` in targeted mode | fail the work item with typed `advance_run_payload_missing_stage_id` |
| target row missing | fail closed with `advance_run_target_missing` |
| target row belongs to another run | fail closed with `advance_run_target_wrong_run` |
| target row belongs to another stage | fail closed with `advance_run_target_wrong_stage` |
| target row terminal before activation | no-op only if the matching authority is already terminalized; otherwise fail `advance_run_target_unexpected_terminal` |
| stale/superseded authority | no-op with typed `advance_run_authority_superseded`; do not mutate sibling stages |
| duplicate active authorities for same `(run_id, stage_id)` | fail closed with `advance_run_authority_conflict` |
| legacy payload with only `run_id` | use legacy state-level behavior |
| legacy payload with `run_id` and `stage_id` but no target | use legacy behavior unless an active authority can be unambiguously linked from `source_work_item_id` |
| targeted post-invoke payload missing `source_stage_execution_id` | fail closed with `advance_run_payload_missing_source_stage_execution_id` |
| targeted requeue/cancel helper drops target fields | fail closed with `advance_run_payload_target_lost` |
| `retry_authority_id` present but `target_stage_execution_id` missing | fail closed with `advance_run_payload_missing_target_for_authority` |
| `target_stage_execution_id` present but `retry_authority_id` missing for retry enqueue reasons | fail closed with `advance_run_payload_missing_retry_authority` |
| retry enqueue reason has null target while source links to a retry authority | fail closed with `advance_run_payload_target_required` |
| `retry_authority_id` points at a different target than `target_stage_execution_id` | fail closed with `advance_run_authority_target_mismatch` |
| `source_stage_execution_id` differs from `target_stage_execution_id` for targeted post-invoke work | fail closed with `advance_run_source_target_mismatch` |
| `source_invoke_work_item_id` is not linked to the same authority | fail closed with `advance_run_source_authority_mismatch` |
| `enqueue_reason = targeted_agent_retry` on an `AdvanceRun` work item | fail closed with `advance_run_invalid_entry_kind`; targeted-agent retry starts with `InvokeAgent` |
| `enqueue_reason = normal_advance` with any target field present | fail closed with `advance_run_payload_target_not_allowed_for_normal_advance` |

No invalid targeted payload may silently fall back to state-level selection. Fallback is allowed only for explicitly legacy payloads that contain no target fields.

### 9.2 Durable retry-authority contract

The `retry_stage_execution_authorities` table is required. It is the durable source for:

- projection rebuild active/current authority;
- startup recovery orphan classification;
- post-invoke target resolution when source work item payload is incomplete;
- operator diagnostics explaining why a retry target was or was not authoritative.

Authority lifecycle:

Full-stage retry lifecycle:

1. `RetryStage` transaction creates the target stage execution.
2. Same transaction creates an `active` authority row pointing at the target with `entry_kind = full_stage_retry`.
3. Same transaction enqueues the first targeted `AdvanceRun`.
4. Targeted stage terminal settlement marks the authority `terminalized`.

Targeted-agent retry lifecycle:

1. Targeted-agent retry transaction creates or reuses the target running stage execution according to the existing targeted-agent retry contract.
2. Same transaction creates an `active` authority row pointing at the target with `entry_kind = targeted_agent_retry`.
3. Same transaction enqueues the targeted `InvokeAgent`.
4. `InvokeAgent` completion/failure enqueues the first targeted follow-up `AdvanceRun`.
5. Targeted stage terminal settlement marks the authority `terminalized`.

Shared lifecycle:

1. A later retry for the same `(run_id, stage_id)` marks the previous active authority `superseded` before inserting the new active row.
2. Historical orphan repair creates a non-active `recovered_orphan` authority provenance row when no authority row exists.

The `RetryStage` transaction must be atomic: no authority row may point at a missing stage execution, and no retry-created target may be left without an authority row.
For targeted-agent retry, the same atomicity requirement applies to stage execution, authority row, and first `InvokeAgent` work item.

### 9.3 Stage summary and run readback contract

Projection/readback additions:

- stage terminal metadata `terminal_reason` nullable;
- `stage_summaries.retry_authority_id` nullable;
- `stage_summaries.is_retry_authoritative` boolean;
- `stage_summaries.retry_authority_state` nullable enum;
- `stage_summaries.terminal_reason` nullable for terminal rows when present;
- run readback `retryAuthority` object for current stage when present.
- run readback `retryAuthorityHistory` array or equivalent GraphQL/MCP/report object for terminalized, superseded, and recovered retry authorities.

If implementation prefers a separate projection table instead of columns on `stage_summaries`, the same fields must be exposed in GraphQL/MCP/readback and rebuilt from `retry_stage_execution_authorities`.

`terminal_reason = stale_retry_recovered` is both stage-owned and authority-history-owned:

- stage-owned storage/projection records why the `StageExecution` became terminal instead of relying on generic `settlement_kind`;
- authority-history storage records why retry authority stopped being considered active/current;
- GraphQL/MCP/run-report readback must expose both when available and they must agree for recovered orphan rows.

`retryAuthorityHistory` must include:

- `retry_authority_id`;
- `entry_kind`;
- `stage_id`;
- `target_stage_execution_id`;
- `authority_state`;
- `terminal_reason`;
- source command/work-item/agent-execution ids when known;
- `created_at` and `updated_at`.

For historical P086-class recovery, readback must show `authority_state = recovered_orphan` and `terminal_reason = stale_retry_recovered`.

### 9.4 Stage rows remain compatible

The core bug is not missing relational columns.
The target execution id already exists as a first-class identifier.
The fix is in durable authority semantics, typed payload semantics, selection rules, and recovery logic.

P091 chooses an explicit stale-retry recovery marker: `terminal_reason = stale_retry_recovered`. It does not require a new stage status enum value; the stage can settle as `skipped` as long as the terminal reason is durable in stage terminal metadata, projected in readback, and mirrored in recovered authority provenance.

## 10. Invariants After P091

After `RetryStage`:

- there is exactly one intended retry target;
- a durable active authority row references that exact target;
- DB constraints prevent two active authorities for the same `(run_id, stage_id)`;
- every follow-on `AdvanceRun` that belongs to that retry must reference or resolve that exact target;
- targeted-agent retry carries authority through `InvokeAgent` first and only then through post-invoke `AdvanceRun`;
- sibling executions of the same state cannot intercept retry authority;
- an orphaned `pending` retry without live work cannot remain authoritative blocked truth.
- projection rebuild after daemon restart must identify the same active retry target without relying on a live work item.
- invalid targeted payloads must fail closed rather than mutating state through legacy heuristics.
- historical orphan recovery without an existing authority row must create a non-active provenance row with `terminal_reason = stale_retry_recovered`.
- work-item cancel/requeue/recovery helpers must preserve typed target fields and must not collapse targeted retries to run scope.
- startup orphan repair must run before projection rebuild and before generic startup catch-up enqueue.
- stage terminal metadata and authority history must agree on `terminal_reason` for recovered retry orphans.

## 11. Acceptance Criteria

P091 is done when all of the following are true:

1. Full-stage `RetryStage` creates target stage execution, durable retry authority, and initial targeted `AdvanceRun` in one transaction.
2. Full-stage retry post-invoke completion and failure enqueue follow-up `AdvanceRun` with the same target authority.
3. Targeted-agent retry creates target stage execution, durable retry authority, and initial targeted `InvokeAgent` in one transaction.
4. Targeted-agent retry post-invoke completion and failure enqueue follow-up `AdvanceRun` with the same target authority.
5. `advance_run` in targeted mode always acts on the explicit retry execution.
6. A sibling execution of the same `stage_id` cannot steal active truth from a targeted retry.
7. Projection rebuild after restart with no live work item still identifies the active retry target from durable authority.
8. Stage summaries/readback surface a non-terminal targeted retry as the current authoritative attempt.
9. Readback exposes terminalized, superseded, and recovered authority history.
10. Existing legacy `AdvanceRun` work items without target execution ids still function.
11. Malformed/wrong-target targeted payloads fail closed and do not fall back to state-level behavior.
12. Work-item requeue/cancel/recovery paths preserve typed target payloads and do not smear targeted advances across the run.
13. Recovery repair excludes legitimate waits: approvals, side effects, transition cursors, recovery holds, backpressure/capacity waits, retry-after/quota waits, and wait-oriented recovery snapshots.
14. `P086`-class orphaned `pending` retry attempts are recovered deterministically as `status = skipped`, `terminal_reason = stale_retry_recovered`, with a non-active recovered authority provenance row.
15. Duplicate active authorities for the same `(run_id, stage_id)` are impossible under concurrent retry commands.
16. Startup recovery runs P091 orphan repair before projection rebuild and before any generic run-scoped `startup_catchup` `AdvanceRun` enqueue.
17. `terminal_reason = stale_retry_recovered` is persisted in stage terminal metadata and retry authority history readback.
18. Partial-target `AdvanceRunPayloadV1` cases fail closed according to the matrix in section 9.1.
19. `./scripts/test-gate.sh proposal-091` exists and covers the contract above.

## 12. Test Plan

### 12.1 Retry transaction and authority contract test

Assert that full-stage `RetryStage`:

- creates a new `StageExecution`
- creates one active `retry_stage_execution_authorities` row pointing at that execution
- enqueues `AdvanceRun`
- includes `target_stage_execution_id = new_stage.id`
- includes `retry_authority_id`
- commits those writes atomically
- enforces the partial unique active-authority constraint under concurrent retries

### 12.2 Targeted-agent retry entry lifecycle test

Assert that targeted-agent retry:

- creates or reuses the target running `StageExecution` according to the current targeted-agent retry contract
- creates one active `retry_stage_execution_authorities` row with `entry_kind = targeted_agent_retry`
- enqueues targeted `InvokeAgent`, not an initial `AdvanceRun`
- includes `target_stage_execution_id` and `retry_authority_id` in the `InvokeAgent` payload
- commits those writes atomically

### 12.3 Post-invoke target propagation tests

Create full-stage retry and targeted-agent retry cases. For each:

- execute/settle an `InvokeAgent` under targeted authority;
- assert post-completion `AdvanceRun` carries the same target;
- assert post-failure `AdvanceRun` carries the same target;
- assert `advance_run` settles the intended target after provider completion/failure.

### 12.4 Targeted orchestration precedence test

Create:

- one older sibling execution
- one newer retry target execution

Then assert `advance_run` with `target_stage_execution_id` activates and settles the target, not the sibling.

### 12.5 Projection rebuild authority test

Create:

- an active retry authority;
- no live `AdvanceRun` work item;
- multiple sibling stage executions.

Then rebuild projections and assert summaries/readback still identify the target as authoritative.

### 12.6 Legacy compatibility test

Create an `AdvanceRun` payload without `target_stage_execution_id` and assert legacy state-level behavior still works.

### 12.7 Typed payload negative tests

Assert fail-closed behavior for:

- malformed payload JSON;
- missing target row;
- target wrong run;
- target wrong stage;
- terminal target with active authority;
- stale/superseded authority;
- duplicate active authorities.
- targeted post-invoke payload missing source stage execution;
- targeted requeue/cancel path dropping target fields.
- authority id present with missing target;
- target present with missing authority for retry enqueue reasons;
- retry enqueue reason with null target when source links to authority;
- authority-target mismatch;
- source-stage/target mismatch;
- source-invoke/authority mismatch;
- targeted-agent retry reason incorrectly enqueued as `AdvanceRun`;
- normal advance containing target fields.

### 12.8 Target-aware work-item repository tests

Create multiple `AdvanceRun` items for one run, including one targeted retry and one legacy run-scoped advance. Assert:

- claim/requeue/cancel by authority affects only the targeted item;
- abandoned-work recovery preserves typed target payloads;
- post-invoke `AdvanceRun` with nullable `work_items.stage_id` still resolves from payload;
- malformed targeted payloads are quarantined rather than replayed as legacy advances.

### 12.9 Orphaned retry recovery test

Create:

- a `pending` retry execution
- no live work items
- no active agent executions
- a sibling settled execution
- no preexisting authority row

Assert startup recovery settles the orphan with `status = skipped`, `terminal_reason = stale_retry_recovered`, creates a non-active `recovered_orphan` authority provenance row, and rebuilds summaries correctly.

Also assert ordering:

- orphan repair runs before projection rebuild;
- projection rebuild observes the post-repair terminal state;
- generic startup catch-up enqueue runs only after projection rebuild;
- no run-scoped `startup_catchup` `AdvanceRun` is enqueued solely because of the pre-repair orphan state.

### 12.10 Recovery negative tests

Create pending/running retries that must not be settled because they have:

- pending approval/manual gate;
- unresolved side effect ledger rows;
- transition cursor/startup catch-up hold;
- queued/backpressured work item;
- active retry-after/quota wait;
- recovery snapshot recommending wait/approval/retry-later.

Assert orphan repair leaves each unchanged.

### 12.11 Authority history readback test

Create active, terminalized, superseded, and recovered authority rows. Assert GraphQL/MCP/run-report readback exposes the current authority and history fields without making terminal history authoritative.

For recovered orphan rows, assert stage terminal metadata and authority history both expose `terminal_reason = stale_retry_recovered`.

### 12.12 P086 regression fixture

Materialize a deterministic fixture from the `P086` shape:

- successful `RetryStage`
- orphaned retry `pending`
- no live work
- sibling settled execution
- blocked run truth

Assert P091 removes the ghost-authority condition.

### 12.13 Proposal gate

Add `./scripts/test-gate.sh proposal-091` with at least:

- db retry-authority persistence and projection rebuild tests;
- db partial unique active-authority concurrency test;
- engine retry transaction and post-invoke propagation tests;
- targeted-agent retry `InvokeAgent`-first lifecycle test;
- target-aware work-item requeue/cancel/recovery tests;
- startup recovery ordering test proving P091 repair precedes projection rebuild and generic catch-up enqueue;
- recovery negative tests for legitimate waits;
- typed `AdvanceRunPayloadV1` parse/validation tests.
- GraphQL/MCP/run-report retry authority history readback tests.

## 13. Rollout

Roll out in four steps:

1. Add durable retry-authority schema, typed `AdvanceRunPayloadV1`, and readback fields behind backward-compatible readers.
2. Enable full-stage retry targeted `AdvanceRun` emission, targeted-agent retry targeted `InvokeAgent` emission, and post-invoke propagation.
3. Enable projection rebuild authority and readback display.
4. Add startup orphaned retry recovery and projection rebuild guardrails.

Historical runs are not silently mutated except through the explicit orphaned-retry recovery criteria.

## 14. Risks and Mitigations

### Risk: Breaking legacy `AdvanceRun` processing

Mitigation:
- keep `target_stage_execution_id` optional;
- preserve current state-level fallback when absent.

### Risk: Over-constraining same-state loops

Mitigation:
- targeted mode applies only when an explicit target id is present;
- ordinary non-retry state progression remains unchanged.

### Risk: Misclassifying valid `pending` stages as orphaned

Mitigation:
- recovery requires both:
  - no live work items
  - no active agent executions
- and also evidence of a newer settled sibling or workflow progression.
- recovery must apply the explicit exclusions in section 8.10 before it writes any terminal repair.

### Risk: Durable authority conflicts after partial writes or old retries

Mitigation:
- `RetryStage` creates stage execution, authority, and first targeted work item in one transaction;
- duplicate active authorities for the same `(run_id, stage_id)` fail closed and surface `advance_run_authority_conflict`;
- a new operator retry supersedes the previous active authority before creating the next authority.

## 15. Open Questions

None for implementation readiness.

Closed decisions:

- full-stage retry is `AdvanceRun`-first.
- targeted-agent retry is `InvokeAgent`-first.
- orphaned historical retries recover as `status = skipped`, `terminal_reason = stale_retry_recovered`.
- `terminal_reason` is stage-owned and authority-history-owned; both readbacks must agree for recovered orphan rows.
- startup orphan repair runs before projection rebuild and before generic startup catch-up enqueue.
- partial-target `AdvanceRunPayloadV1` cases fail closed and cannot fall back to legacy run-scoped selection.
- duplicate active authority prevention is enforced by a DB partial unique index or equivalent transaction-level invariant.
- requeue/cancel/recovery helpers must preserve typed target payloads instead of operating only by `run_id`.
- P091 requires durable retry-authority records, typed `AdvanceRunPayloadV1`, target propagation after invoke completion/failure, and projection/readback fields rebuilt from durable authority.

## 16. Recommendation

Proceed with P091 before attempting ad-hoc repair of `P086`-class runs.

The defect is not merely stale projection.
It is a contract hole between:

- retry creation,
- orchestration activation,
- and execution-level truth ownership.

P091 closes that hole by making retry authority explicit and durable.
