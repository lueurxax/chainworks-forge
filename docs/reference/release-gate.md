# Release Gate

Stable reference for the manual release gate: post-approval task execution, deterministic native release operations (git push, sandbox/staging publish), and release-artifact receipt truth.

## Purpose

A release stage is a manual-approval gate that:

1. pauses for explicit operator approval,
2. executes a declared sequence of tasks **after** approval,
3. runs sensitive release side effects through native executor code (not ACP agents),
4. freezes delivery inputs at run start,
5. writes canonical release artifacts at catalog-defined paths,
6. produces a structured `delivery_receipt` that survives success, failure, retry, and terminal backfill paths.

Release side effects are irreversible, so the gate is designed to make human approval, task ordering, and receipt truth load-bearing rather than advisory.

## Scope

This reference covers:

- manual-gate approval handling for release stages,
- N-phase sequential ordering of post-approval tasks,
- `effective_tasks()` resolution and post-approval ownership,
- end-state execution for terminal finalizer stages,
- retry-with-reapproval semantics,
- worktree safety for post-approval agents,
- native `commit_and_push_to_github` and `build_archive_and_push_connect` execution,
- frozen `delivery_configuration_json` input truth,
- canonical release artifact paths,
- `delivery_receipt` preserve/backfill semantics,
- northbound readback of release configuration and evidence.

It does not replace:

- the broader engine topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run snapshots in [runtime-contract.md](runtime-contract.md),
- the repo-backed delivery slice overview in [full-mvp-delivery.md](full-mvp-delivery.md),
- or the daemon architecture in [rust-control-plane.md](rust-control-plane.md).

## Post-Approval Task Execution

### Approval-handling transfer

When the command handler processes a granted approval on a `manual_gate`:

- **With `post_approval_tasks`**: the stage transitions to `Running` instead of immediately settling as `Completed`. The orchestrator then enqueues phase 0 of the effective (post-approval) task list.
- **Without `post_approval_tasks`** (simple gates): the stage settles as `Completed` and transitions evaluate normally.

### Post-approval task ownership

`CompiledState` carries two task lists:

- `tasks` — the primary run block tasks,
- `post_approval_tasks` — tasks from `run_after_approval` in manual-gate states.

`effective_tasks()` resolves which list the orchestrator uses at runtime:

- if the stage is in post-approval execution (a `Running` manual-gate with a `Granted` approval) and `post_approval_tasks` is non-empty, use `post_approval_tasks`,
- otherwise use `tasks`.

All downstream accounting — phase detection, `task_index` mapping, completion counting, `total_tasks`, and the N-phase gating loop — operates on the effective list.

### N-phase sequential ordering

The compiler assigns incrementing `phase` numbers to enforce declared execution order within run blocks:

- **`sequence` blocks**: each task receives its positional index as its phase (0, 1, 2, …). The orchestrator enqueues phase 0, waits for all to complete, then enqueues phase 1, and so on.
- **`parallel` blocks**: all tasks share phase 0 and execute concurrently.
- **`then` blocks**: each task receives an incrementing phase starting after the highest phase in the preceding `parallel` or `sequence`. In a `parallel` + `then` composition, parallel tasks run at phase 0 and `then` tasks run at phases 1, 2, 3, … in strict sequence.

Orchestrator invariants:

- `task_index` values in `InvokeAgent` work items map into the effective task list, not an absolute index,
- phase completion is derived from settled work items: the current phase is the maximum phase among completed invocations,
- the next phase is the minimum phase strictly greater than the current phase that has not yet been enqueued,
- if the current phase had failures, later phases are not enqueued and the stage settles as `Failed`.

### End-state execution

`is_end` states with a non-empty `tasks` list do not short-circuit to immediate run completion. Instead:

- the stage is created and tasks are enqueued through the regular compute-state path,
- when all tasks complete, `evaluate_and_transition` sees `is_end` with no remaining transitions and marks the run completed.

Bare end states (no `tasks`) still settle immediately.

This ensures that terminal workflow states (e.g., `state_12_workflow_complete`) execute their finalizer tasks (`finalize_run_and_produce_receipts`) and produce artifacts such as `delivery_receipt`, `run_report`, and `run_state` before the run is marked `Completed`.

### Retry-with-reapproval

When a post-approval task fails and the operator retries the stage:

- the stage is reset with a new attempt and `StageStatus::Pending`,
- because the state is a `manual_gate`, the orchestrator re-enters the manual-gate path,
- a fresh `Approval` record is created with `Requested` decision,
- the stage moves to `WaitingApproval` — the operator must approve again.

This is intentional: release side effects are irreversible, so fresh human approval ensures the operator has reviewed the failure before another attempt.

### Worktree safety

Post-approval release tasks may require a provisioned worktree (e.g., agents with `worktree_policy.strategy: dedicated`). The `RepoSafetyGuard` worktree readiness check operates on the effective task list, inspecting both `state.tasks` and `state.post_approval_tasks`. Missing worktree blocks execution.

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

### Release gate UI

`ReleaseGateView` is the operator-facing approval surface for the final gate. It must show enough context to make an informed decision:

- proposal and workflow context,
- review artifact availability,
- repo identity, branch, and worktree,
- release target,
- quick actions around diff/worktree inspection.

**Verification Truth:**
When `implementation_self_assessment_v2` is available, `ReleaseGateView` uses its `verification_green` signal instead of legacy test results. This ensures the release gate does not show conflicting legacy test truth.

**Blocked Status:**
If the implementation self-assessment status is `blocked` (meaning code work is finished but verification is not green), `ReleaseGateView` displays a high-visibility warning row at the top of the Change Summary section.

**Handoff Tasks:**
Pending handoff tasks from the self-assessment are displayed when applicable. Handoff tasks with `blocking_review: true` use warning treatment and provide links to the full artifact or assessment panel.

### Native Release Execution

### Release agents bypass ACP

`commit_and_push_to_github` and `build_archive_and_push_connect` are executor-owned release operations. They bypass ACP completely and run through native Rust release services.

This is the hard safety boundary for the release slice: agents may recommend release, but they do not improvise commit, push, archive, or upload mechanics through free-form LLM shelling.

### Delivery configuration is frozen at run start

Repo-backed release execution depends on the run's frozen `delivery_configuration_json`. That payload is accepted at the northbound start surfaces, validated by delivery preflight, persisted on `Run`, and deserialized fail-closed when the executor enters a release step.

Workflows that contain release agents (`commit_and_push_to_github` or `build_archive_and_push_connect`) require `delivery_configuration_json` at `StartRun`. If the payload is absent, `StartRun` returns a blocked delivery-preflight result and does not create a `Run` row.

The frozen configuration is the only release input owner for:

- repository identity,
- repository root,
- base branch,
- target branch,
- release target ID,
- release mode.

### Canonical artifact paths

Release artifacts resolve through the compiled workflow/catalog artifact map so transition conditions such as `exists('git_push_receipt')` and operator readback stay on one authority lane.

| Artifact | Canonical path |
|---|---|
| `release_manifest` | `.chainworks/release/release-manifest.json` |
| `git_push_receipt` | `.chainworks/release/git-push-receipt.json` |
| `release_bundle_manifest` | `.chainworks/release/release-bundle-manifest.json` |
| `connect_upload_receipt` | `.chainworks/release/connect-upload-receipt.json` |
| `delivery_receipt` | `.chainworks/release/delivery-receipt.json` |

### Git release step

The git release service:

1. inspects worktree status and diff stats,
2. stages all changes,
3. creates the release commit,
4. resolves `HEAD`,
5. pushes to the configured target branch,
6. persists `release_manifest` and `git_push_receipt`.

Protected branches such as `main` and `master` are rejected by the release path.

### Publish step

The publish service consumes prior git artifacts and frozen delivery config. It:

1. verifies git success,
2. attempts a local `xcodebuild build` compilability check,
3. records build warnings without treating sandbox build failure as fatal,
4. derives deterministic archive/checksum metadata,
5. persists `release_bundle_manifest` and `connect_upload_receipt`.

### Publish is safe-mode only

The current release slice supports `sandbox` and `staging` release modes only. `build_archive_and_push_connect` performs deterministic local build/archive evidence work and writes a safe-mode upload receipt. It does not perform real App Store Connect communication and does not enable production release mode.

## Receipt Settlement

### First valid `delivery_receipt` writer wins

`delivery_receipt` is preserved, not endlessly regenerated.

The rule:

- git failure may write it,
- publish failure may write it,
- publish success may write it,
- terminal finalization may backfill it only when still absent.

Once the canonical receipt path already exists, later write sites must preserve the existing file instead of overwriting it.

### Terminal backfill is lineage-gated

The terminal finalization state is only a fallback writer. It may backfill `delivery_receipt` only when finalization still has the full eligibility chain:

- frozen delivery config,
- worktree root,
- prior release-agent lineage strong enough to derive release-result truth.

Pre-release failures without release lineage do not get a metadata-only receipt.

## Failure Semantics

### Missing delivery configuration fails closed

If a repo-backed release step still starts without valid `delivery_configuration_json`, the executor fails closed as a defensive last line of protection. The pre-release failure path settles the native release `AgentExecution` and `StageExecution` as failed, and does not synthesize an executor-side `delivery_receipt`.

### Git failure is terminal for publish

If `commit_and_push_to_github` fails:

- publish is not attempted,
- git/publish happy-path artifacts are not fabricated,
- `delivery_receipt` records `failure_stage = "commit_and_push"` when eligible,
- the run blocks with operator-visible failure truth.

### Publish failure preserves git truth

If publish fails after git succeeds:

- git artifacts remain authoritative,
- `delivery_receipt` records `failure_stage = "build_archive_and_push"`,
- the run blocks rather than rolling back silently.

### Receipt preservation outranks later convenience writes

If a canonical `delivery_receipt` already exists, later writer sites must skip. This keeps failure-path truth and earlier success-path truth stable under subsequent finalization or retry bookkeeping.

## Northbound Readback

Release truth is exposed northbound through the same read stack used by other run data:

- GraphQL run reads,
- MCP `runs.get`,
- MCP `reports.get`,
- collection/resource readers tied to run projections and report material.

The northbound contract:

- frozen delivery configuration remains readable after start,
- release artifacts remain discoverable at canonical paths,
- structured release-result truth survives into operator/report surfaces.

## Implementation Surface

| File | Role |
|---|---|
| `control-plane/crates/workflow/src/compiler.rs` | N-phase assignment for `sequence` and `then` blocks |
| `control-plane/crates/workflow/src/plan.rs` | `CompiledState.post_approval_tasks`, `CompiledTask.phase` |
| `control-plane/crates/engine/src/orchestrator.rs` | `effective_tasks()`, N-phase gating, post-approval enqueuing, end-state execution |
| `control-plane/crates/engine/src/command_handler.rs` | Conditional `Running` vs `Completed` on `ApproveStage` |
| `control-plane/crates/engine/src/worktree.rs` | `RepoSafetyGuard` effective-list inspection |
| `control-plane/crates/engine/src/executor.rs` | Native release agent routing |
| `control-plane/crates/engine/tests/release.rs` | Release-execution integration coverage |
| `control-plane/crates/engine/tests/integration.rs` | Post-approval orchestration coverage |

## Related Docs

- [workflow-execution-engine.md](workflow-execution-engine.md)
- [runtime-contract.md](runtime-contract.md)
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md)
- [full-mvp-delivery.md](full-mvp-delivery.md)
- [rust-control-plane.md](rust-control-plane.md)
- [test-gates.md](test-gates.md)
