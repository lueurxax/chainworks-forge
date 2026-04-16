# Post-Approval Task Execution and Release Gate Completion (former P044)

## Status
- **Implemented and stabilized**: 2026-04-15
- **Primary contract owners**: Rust control-plane compiler, orchestrator, command handler
- **Evidence source**: `./scripts/test-gate.sh proposal-044` and `control-plane/crates/engine/tests/integration.rs`

## Purpose

This document replaces the proposal-level text for post-approval task execution
and release gate completion. It defines the production contract for N-phase
sequential ordering, effective task resolution, manual-gate approval handling,
end-state task execution, and retry-with-reapproval in the Rust control plane.

The goal of this slice is:

- `sequence` and multi-task `then` blocks execute tasks in strict declared order,
- manual-release gates with `post_approval_tasks` enter `Running` after approval
  instead of prematurely settling,
- `effective_tasks()` resolution gives the orchestrator the correct task list for
  post-approval execution,
- end states with `run` blocks execute their tasks before the run completes,
- failed post-approval retries return the stage to `WaitingApproval` with a fresh
  `Approval` record,
- and worktree safety inspects both `state.tasks` and `state.post_approval_tasks`.

## Scope

This reference covers:

- N-phase sequential ordering in the compiler for `sequence` and multi-task `then`,
- post-approval task ownership via `post_approval_tasks` and `effective_tasks()`,
- approval-handling transfer in `ApproveStage` for manual-release gates,
- orchestrator phase gating with `task_index` / `phase` invariants,
- end-state execution for `is_end` states with `run` blocks,
- retry-with-reapproval semantics,
- worktree safety for post-approval release agents,
- and native release agent routing that bypasses ACP.

It does not replace:

- the broader engine topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run snapshots in [runtime-contract.md](runtime-contract.md),
- deterministic release execution in [045-deterministic-release-operations.md](045-deterministic-release-operations.md),
- or the proof inventory in [test-gates.md](test-gates.md).

## Core Rules

### N-phase sequential ordering

The compiler assigns incrementing `phase` numbers to enforce declared execution
order within run blocks.

**`sequence` blocks**: each task receives its positional index as its phase
(0, 1, 2, ...). The orchestrator enqueues phase 0 tasks first, waits for all to
complete, then enqueues phase 1, and so on.

**`parallel` blocks**: all tasks share phase 0. They execute concurrently.

**`then` blocks**: each task receives an incrementing phase starting after the
highest phase in the preceding `parallel` or `sequence` block. In a
`parallel` + `then` composition, parallel tasks run at phase 0 and `then` tasks
run at phases 1, 2, 3, etc. in strict sequence.

Phase assignment is handled in `control-plane/crates/workflow/src/compiler.rs`.
The orchestrator's N-phase gating logic in
`control-plane/crates/engine/src/orchestrator.rs` generalizes beyond the previous
binary phase 0/1 model: it determines the current completed phase from settled
work items, finds the next unenqueued phase, and enqueues tasks for that phase.
If any task in a phase fails, later phases are skipped and the stage settles
as `Failed`.

### Post-approval task ownership

`CompiledState` carries two task lists:

- `tasks` -- the primary run block tasks,
- `post_approval_tasks` -- tasks from `run_after_approval` in manual-gate states.

`effective_tasks()` resolves which list the orchestrator uses at runtime:

- if the stage is in post-approval execution (a `Running` manual-gate with a
  `Granted` approval) and `post_approval_tasks` is non-empty, use
  `post_approval_tasks`,
- otherwise use `tasks`.

All downstream accounting -- phase detection, `task_index` mapping, completion
counting, `total_tasks`, and the N-phase gating loop -- operates on the
effective list.

### Approval-handling transfer

When `ApproveStage` processes a granted approval on a `manual_gate`:

- **With `post_approval_tasks`**: the stage transitions to `Running` instead of
  immediately settling as `Completed`. `AdvanceRun` then picks up the running
  stage and enqueues phase 0 of the effective (post-approval) task list.
- **Without `post_approval_tasks`** (simple gates): the stage settles as
  `Completed` and `AdvanceRun` evaluates transitions. No behavioral change from
  the pre-P044 contract.

This conditional is in `control-plane/crates/engine/src/command_handler.rs`.

### Orchestrator phase gating

The orchestrator maintains these invariants for multi-phase execution:

- `task_index` values in `InvokeAgent` work items map into the effective task
  list, not an absolute index.
- Phase completion is derived from settled work items: the current phase is the
  maximum phase among completed invocations.
- The next phase is the minimum phase strictly greater than the current phase that
  has not yet been enqueued.
- If the current phase had failures, later phases are not enqueued and the stage
  settles as `Failed`.

### End-state execution

`is_end` states with a non-empty `tasks` list no longer short-circuit to
immediate run completion. Instead:

- the stage is created and tasks are enqueued through the regular compute-state
  path,
- when all tasks complete, `evaluate_and_transition` sees `is_end` with no
  remaining transitions and marks the run completed.

Bare end states (no `tasks`) still settle immediately as before.

This ensures that terminal workflow states like `state_12_workflow_complete`
execute their finalizer tasks (e.g., `finalize_run_and_produce_receipts`)
and produce artifacts such as `delivery_receipt`, `run_report`, and `run_state`
before the run is marked `Completed`.

### Retry-with-reapproval

When a post-approval task fails and the operator retries the stage:

- the stage is reset with a new attempt and `StageStatus::Pending`,
- because the state is a `manual_gate`, the orchestrator re-enters the
  manual-gate path,
- a fresh `Approval` record is created with `Requested` decision,
- the stage moves to `WaitingApproval` -- the operator must approve again.

This is intentional: release side effects are irreversible, so fresh human
approval ensures the operator has reviewed the failure before another attempt.

### Worktree safety

Post-approval release tasks may require a provisioned worktree (e.g., agents
with `worktree_policy.strategy: dedicated`). The `RepoSafetyGuard` worktree
readiness check operates on the effective task list, inspecting both
`state.tasks` and `state.post_approval_tasks`. Missing worktree blocks
execution.

### Native release agent routing

`commit_and_push_to_github` and `build_archive_and_push_connect` are native
release operations that bypass ACP. They execute through Rust release services,
not through free-form LLM shelling. The deterministic execution contract for
these agents is owned by
[045-deterministic-release-operations.md](045-deterministic-release-operations.md).

## Stage Status Lifecycle

**Simple manual gate (no `post_approval_tasks`):**
```
Pending -> WaitingApproval -> [approval] -> Completed -> evaluate_and_transition
```

**Release gate with `post_approval_tasks`:**
```
Pending -> WaitingApproval -> [approval] -> Running
  -> phase 0 tasks execute -> [complete]
  -> phase 1 tasks execute -> [complete]
  -> ... -> all phases complete
  -> Completed -> evaluate_and_transition
```

**End state with `run` block:**
```
Pending -> Running -> tasks execute -> [complete] -> run marked Completed
```

## Proof Obligations

The canonical same-tree proof lane for this slice is:

```bash
./scripts/test-gate.sh proposal-044
```

That gate targets these focused tests:

- `test_compile_n_phase_ordering` (workflow crate) -- compiler assigns correct
  phases to `sequence` and `then` blocks
- `test_approve_manual_gate_with_post_approval_tasks_sets_running` -- approval
  on a release gate sets `Running` instead of `Completed`
- `test_approve_simple_manual_gate_settles_completed` -- simple gates still
  settle as `Completed` on approval
- `test_post_approval_tasks_enqueued_after_approval` -- post-approval phase 0
  tasks are enqueued after approval
- `test_end_state_with_tasks_does_not_short_circuit` -- end states with tasks
  execute before run completion
- `test_n_phase_sequence_ordering` -- phases execute in strict numeric order
- `test_post_approval_retry_requires_fresh_approval` -- retry returns to
  `WaitingApproval` with a new `Approval` record
- `test_simple_manual_gate_no_regression` -- existing simple-gate behavior is
  preserved
- `test_state_11_to_state_12_happy_path` -- full happy path from manual release
  through post-approval tasks to end-state finalization

The gate runs the full Rust workspace test suite to catch regressions in
adjacent orchestrator, settlement, and approval logic.

Focused runtime coverage lives primarily in
`control-plane/crates/engine/tests/integration.rs`, with compiler proof in
`control-plane/crates/workflow/tests/integration.rs`.

## Implementation Surface

| File | Role |
|---|---|
| `control-plane/crates/workflow/src/compiler.rs` | N-phase assignment for `sequence` and `then` blocks |
| `control-plane/crates/workflow/src/plan.rs` | `CompiledState.post_approval_tasks`, `CompiledTask.phase` |
| `control-plane/crates/engine/src/orchestrator.rs` | `effective_tasks()`, N-phase gating, post-approval enqueuing, end-state fix |
| `control-plane/crates/engine/src/command_handler.rs` | Conditional `Running` vs `Completed` on `ApproveStage` |
| `control-plane/crates/engine/src/worktree.rs` | `RepoSafetyGuard` effective-list inspection |

## Archived Proposal History

The proposal draft, consolidated review, evidence pack, and implementation audit
rounds R1 through R7 for P044 are retained under
[../archive/proposals/README.md](../archive/proposals/README.md) for provenance
only. They are not the canonical current-head contract anymore.

## Related Stable Docs

- [runtime-contract.md](runtime-contract.md)
- [workflow-execution-engine.md](workflow-execution-engine.md)
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md)
- [rust-control-plane.md](rust-control-plane.md)
- [045-deterministic-release-operations.md](045-deterministic-release-operations.md)
- [test-gates.md](test-gates.md)
