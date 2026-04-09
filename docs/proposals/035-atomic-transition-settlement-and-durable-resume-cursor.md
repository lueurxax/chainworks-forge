# Proposal 035: Atomic Transition Settlement and Durable Resume Cursor

| Field | Value |
|---|---|
| Date | 2026-04-08 |
| Status | Draft |
| Author | Codex |
| Depends on | [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/runtime-contract.md](../reference/runtime-contract.md), [../reference/domain-model.md](../reference/domain-model.md), [021-run-transition-notifications-and-attention-routing.md](021-run-transition-notifications-and-attention-routing.md) |
| Scope | Eliminate heuristic reconstruction of interrupted stage boundaries by introducing one run-owned durable transition cursor and atomic transition settlement between `state N completed` and `state N+1 scheduled`. |
| Goal | Make resume, reports, recovery, and operator surfaces read one persisted continuation truth instead of reconstructing workflow progression from stage rows, `run_state` artifacts, live timeline tails, and restart heuristics. |

---

## 1. Context and Motivation

The current execution slice can settle agent attempts and stage outcomes, but it still has a weak point at the workflow boundary between:

1. the current state completing successfully, and
2. the next state becoming the durable continuation point.

This gap is now the dominant failure pattern behind repeated interruptions in long-running real runs such as `EA93E855-3BEA-4D86-B287-205A7A32AA1C`.

Observed symptoms are all variants of the same defect:

- the runtime session closes after a successful stage, but before the next stage is durably settled,
- restart normalization and recovery then reconstruct continuation truth from mixed sources,
- reports can show a later stage that has no matching artifact tree,
- resume can target the wrong stage,
- stale `ready` or `blocked` stage rows can outweigh the actual intended continuation path,
- and the same run appears to "fail again at the same place" even though the visible symptoms differ.

This is not primarily a provider bug and not primarily a UI bug.
It is a persistence and truth-ownership bug:

- workflow progression does not currently have one canonical, atomic, durable checkpoint,
- so recovery falls back to heuristics,
- and those heuristics disagree once the persisted graph is even slightly polluted.

Proposal 035 fixes that boundary directly.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the system persist exactly one canonical continuation point whenever a state completes and the workflow advances?
2. Can relaunch and manual resume continue from that continuation point without parsing `run_state` artifacts or inferring intent from stage ordering?
3. Can reports and blocked/recovery surfaces describe an interrupted transition without inventing phantom downstream stage truth?
4. Can startup normalization avoid converting "scheduled but not yet started" continuation rows into misleading blocked failures?
5. Can the system distinguish three different truths cleanly:
   - last completed stage,
   - next scheduled stage,
   - and actually started stage work?

---

## 3. Scope

This proposal includes:

- a run-owned durable continuation cursor,
- atomic transition settlement at the end of each successful state,
- explicit persistence for "next scheduled state" versus "current completed state",
- resume and report readers that prefer the durable cursor over heuristic reconstruction,
- startup normalization rules that respect scheduled-but-not-started continuation truth,
- migration of operator/recovery/report surfaces away from `run_state` artifact authority for resume targeting.

This proposal does **not** include:

- redesign of workflow YAML,
- changes to agent output contracts,
- broader ACP transport work,
- loop-budget policy changes,
- or cleanup of every historical stale run already persisted before the new contract exists.

---

## 4. Problem Statement

### 4.1 Current continuation truth is split across parallel owners

Today the system can reconstruct continuation intent from several places:

- `StageExecution` status and ordering,
- `Run.currentStageID` / derived stage accessors,
- `run_state` artifacts emitted by orchestrator agents,
- live timeline tail events such as `sessionClosed`,
- restart normalization that converts stale in-flight work into blocked/failure rows,
- and ad-hoc report/recovery synthesis.

Each of those surfaces may be locally reasonable, but they do not form one atomic contract.

### 4.2 The boundary failure is deterministic, not random

The recurring production shape is:

1. `state N` completes successfully.
2. Transition evaluation chooses `state N+1`.
3. The provider session closes or the process restarts before the next stage is durably settled.
4. The system later tries to reconstruct "where the run really was".

At that point, multiple contradictory truths may exist:

- `state N` has complete artifacts and receipts,
- a downstream `StageExecution` row for `state N+1` may already exist as `ready`,
- an older `run_state` artifact may still advertise a different `next_stage`,
- and startup normalization may block whichever in-flight stage row it sees first.

The result is a self-reinforcing recovery loop.

### 4.3 `run_state` artifacts are useful evidence, but not canonical continuation truth

`run_state` should remain evidence authored by workflow logic and useful for operator inspection.
But it should not be the primary authority for restart/resume targeting once the engine itself has a run-owned progression cursor.

The engine must own continuation truth directly.

---

## 5. Core Product Behavior

### 5.1 Introduce one run-owned continuation cursor

Add one canonical persisted continuation contract on `Run`, conceptually:

```swift
struct TransitionCursor: Codable, Sendable {
    let schemaVersion: Int
    let sequenceNumber: Int
    let lastCompletedStateID: String?
    let lastCompletedStageExecutionID: UUID?
    let nextScheduledStateID: String?
    let nextScheduledIteration: Int?
    let nextScheduledAttemptNumber: Int?
    let scheduledStageExecutionID: UUID?
    let settlementState: TransitionSettlementState
    let updatedAt: Date
}
```

Persistence contract:

- `TransitionCursor` is not a separate SwiftData model.
- It is a `Codable` value stored on `Run` in one persisted JSON field, conceptually `transitionCursorJSON`.
- This keeps cursor ownership aligned with the existing run-owned snapshot pattern and makes atomic settlement part of the same `Run` save boundary rather than a cross-entity transaction.

Field semantics:

- `schemaVersion` is the schema version of the cursor payload.
- `sequenceNumber` is a monotonic per-run settlement counter incremented on each durable transition settlement.

Where `TransitionSettlementState` is intentionally limited to transition-boundary truth only:

- `idle`
- `next_state_scheduled_not_started`
- `next_state_started`
- `terminal`

Exact field names can change, but ownership cannot:

- continuation truth belongs to one persisted run-owned cursor.

### 5.2 Transition settlement must be atomic

When a state completes and a transition is selected:

1. Persist the current stage as completed.
2. Persist the chosen next state as the scheduled continuation point.
3. Persist or create the associated downstream `StageExecution` row if that is part of the design.
4. Update the run-owned continuation cursor in the same save boundary.
5. Only then consider the workflow advanced.

This is one atomic settlement operation, not a multi-step partially durable sequence.
Implementation may use an internal helper such as `settleTransition(...)`, but the contract is:

- all transition mutations are batched,
- one `ModelContext.save()` commits them,
- and no notification, resume target, or recovery reader may observe an intermediate half-settled state.

The crucial rule is:

- the system must never have to infer whether the next state was chosen.

It must already be durably recorded.

### 5.3 Scheduled is not started

The design must preserve a strict distinction between:

- completed current stage,
- scheduled next stage,
- and started next stage.

A `ready` row must not be treated as equivalent to:

- `running`,
- `blocked`,
- or "this stage already began and then failed".

If a run is interrupted after scheduling the next state but before any agent work starts, startup normalization must preserve that as resumable continuation truth, not rewrite it into a generic blocked failure.

### 5.4 Resume must read only the durable cursor

After this proposal:

- `ResumeManager`,
- `ExecutionService.resumeInterruptedRuns`,
- approval re-entry,
- manual resume,
- and blocked-run recovery

must target continuation from the run-owned cursor first.

They may still show supporting context from:

- `run_state`,
- stage evidence packets,
- recovery snapshots,
- or reports,

but those are secondary evidence, not the primary continuation target.

### 5.5 Live stalled-run reconciliation must also become cursor-first

The defect seam is not only post-restart resume.
Current code already contains an in-process stalled-run reconciliation path that can fire while the app is still open:

- it observes the latest live event,
- sees `.sessionClosed`,
- infers a stall from the current stage,
- and can block the run plus fail stage and agent rows before relaunch ever happens.

Proposal 035 must explicitly rebind that live path as well.

After this proposal:

- `ExecutionService` stalled-run reconciliation reads the run-owned transition cursor first,
- `.sessionClosed` remains transport evidence only,
- and a run whose cursor says "current stage completed, next stage scheduled but not started" must not be demoted into blocked or failed truth just because the session ended.

This proposal is incomplete if it only fixes post-restart resume while leaving live false-demotion active.

### 5.6 Reports must describe interrupted transition truth honestly

If interruption happens after `state N` completed but before `state N+1` started, the run report should say exactly that.

It must not:

- invent downstream failure evidence for a stage with no real execution,
- conflate "scheduled next stage" with "failed started stage",
- or let an old blocked row dominate over the durable continuation cursor.

The report should instead distinguish:

- last completed stage,
- scheduled continuation state,
- whether next-stage agent execution ever started,
- and the interruption cause (`app restart`, `session closed before stage start`, etc.).

---

## 6. Architecture

### 6.1 `Run` becomes the owner of transition progression truth

Add a persisted transition cursor field to `Run`.

This proposal intentionally does **not** make:

- `Run.currentStageID`,
- `run_state`,
- latest stage ordering,
- or live timeline tails

the canonical owner of resume targeting.

Those remain derived or supporting evidence.

### 6.2 `WorkflowOrchestrator` owns atomic settlement

`WorkflowOrchestrator.executeStateMachine()` currently:

- completes a state,
- evaluates a transition,
- mutates `currentStateID`,
- and later re-enters execution.

Proposal 035 requires an explicit atomic settlement step between transition evaluation and subsequent execution:

```swift
settleTransition(
    completedStage: stateN,
    selectedTransition: transition,
    nextStateID: stateNPlus1
)
```

That step is responsible for:

- marking stage `N` complete,
- recording `nextStateID`,
- optionally materializing the scheduled downstream stage row,
- updating the cursor,
- and saving all of it durably before execution continues.

If the implementation materializes a downstream `StageExecution` row before stage start, then:

- `scheduledStageExecutionID` points to that row,
- and that row must remain distinguishable from a started stage.

If the implementation chooses not to materialize the downstream row until actual stage start, then:

- `scheduledStageExecutionID` remains `nil`,
- and the cursor is still the canonical owner of scheduled continuation truth.

### 6.3 `ResumeManager` startup normalization must stop flattening scheduled continuation truth

Startup normalization currently converts stale `.pending/.ready/.running` work into blocked/failed rows too aggressively.

After this proposal:

- if the continuation cursor says `next_state_scheduled_not_started`,
- startup normalization must preserve that state as resumable,
- not rewrite it into generic blocked failure truth.

Normalization should only mark work as interrupted-failed when there is actual evidence the stage had started and remained unsettled.

### 6.4 `RunReportBuilder` and recovery readers must consume the cursor first

`RunReportBuilder`, `RecoveryCoordinator`, `BlockedRunRecoveryView`, and related readers should follow this precedence:

1. run-owned transition cursor,
2. stage-level persisted recovery and evidence payloads,
3. supporting artifacts such as `run_state`,
4. heuristic reconstruction only as explicit fallback with degraded trust.

This keeps operator surfaces aligned with one durable progression source.

### 6.5 `ExecutionService` live reconciliation must consume the cursor first

`ExecutionService.reconcileStalledOrchestratorsIfNeeded()` and related stalled-run paths are currently part of the same truth problem.

After this proposal they must follow this order:

1. read the run-owned transition cursor,
2. determine whether the current stage was still running, or had already completed and scheduled the next state,
3. only then decide whether stalled-run demotion is valid,
4. use live `.sessionClosed` only as supporting evidence for why transport stopped.

This means:

- completed-stage boundaries are never demoted just because the last live event was `.sessionClosed`,
- scheduled-but-not-started next stages are preserved as continuation truth,
- and live reconciliation uses the same owner chain as relaunch resume.

### 6.6 Operator shell read models must be rebound explicitly

The shell currently projects run truth through a heuristic single-stage lens centered on `run.currentStageID`.
That is no longer sufficient once the system distinguishes:

- last completed stage,
- next scheduled stage,
- and actually started stage.

Proposal 035 therefore requires explicit read-model migration for:

- `WorkflowMapProjectionService`,
- `RunsHomeView`,
- `IdeaListView`,
- and any shell summary surfaces that currently label one stage as "current" by reading `run.currentStageID`.

The proposal does not require every UI surface to expose all three values verbatim, but it does require one explicit mapping rule.

Recommended shell mapping:

- list surfaces may continue to show a single primary stage label, but that label must derive from the cursor state machine rather than heuristic stage ordering,
- detail surfaces should be able to distinguish "last completed", "scheduled next", and "started now" when those differ,
- `run.currentStageID` must either become a cursor-derived compatibility view or be demoted from canonical authority entirely.

Without this migration, implementation could fix resume and recovery while leaving the rest of the shell on stale heuristic stage truth.

### 6.7 Existing `run_state` artifacts remain evidence only

`run_state` stays useful for:

- operator context,
- workflow-authored rationale,
- export and audit history,
- and human-readable next-step explanation.

For clarity, this refers to the existing `run_state` artifact contract emitted by orchestrator review/aggregation tasks.

But after Proposal 035 it must no longer be able to override the engine's own persisted continuation cursor for resume targeting.

### 6.8 Migration and fallback for pre-cursor runs

Historical runs created before this proposal will not have `transitionCursorJSON`.

The migration contract must therefore be explicit:

- if a run has no persisted cursor, resume/report/recovery paths may fall back to the existing heuristic reconstruction path,
- that fallback must be visibly degraded in trust and log that cursor truth is unavailable,
- and implementation may opportunistically backfill a cursor from the best available persisted stage truth when safe to do so.

Proposal 035 does not require bulk migration of all historical runs.
It does require that pre-cursor runs remain readable and resumable through an explicit fallback path rather than undefined behavior.

### 6.9 Concurrency and cancellation

Transition settlement must be protected by orchestrator-owned serialization.

The implementation contract is:

- the orchestrator remains the only writer of transition progression truth during normal execution,
- cancellation and interruption logic must check the cursor settlement state before demoting in-flight work,
- and cancellation must not tear down a just-completed stage boundary as if it were an unfinished transition.

This proposal does not require a new locking model beyond existing orchestrator isolation, but it does require settlement and cancellation code to share the same owner chain.

### 6.10 Notification ordering

Proposal 021 notification delivery must remain subordinate to durable transition truth.

Therefore:

- any `stageTransitioned`, `runBlocked`, or similar transition-derived notification must fire only after the atomic settlement save succeeds,
- never before settlement commit,
- and never from transient in-memory `currentStateID` mutation alone.

---

## 7. Rollout Sequence

1. Add the persisted run-owned transition cursor model and storage.
2. Teach `WorkflowOrchestrator` to settle transitions atomically.
3. Rebind `ExecutionService` live stalled-run reconciliation to the cursor-first owner chain.
4. Add migration and fallback behavior for pre-cursor runs.
5. Update resume targeting to prefer the cursor.
6. Update startup normalization to preserve scheduled-but-not-started continuation truth.
7. Update report/recovery readers to read the cursor first.
8. Rebind shell projection and read-model surfaces away from heuristic `run.currentStageID` authority.
9. Ensure transition-derived notifications fire only after settlement commit.
10. Add focused proof cases for:
   - interruption after stage completion but before next-stage start,
   - interruption after next-stage scheduling with a stale older `run_state`,
   - live `.sessionClosed` after current-stage completion but before next-stage start,
   - app restart before the scheduled stage executes,
   - report generation for that boundary,
   - shell projection of last-completed vs next-scheduled vs started-now truth,
   - and the `EA93E855`-class state-9-review -> state-10-refinement interrupted-transition scenario.

---

## 8. Acceptance Criteria

Proposal 035 is complete when:

1. A successful `state N -> state N+1` transition is durably represented by one run-owned cursor.
2. Manual resume and relaunch resume choose the same `nextScheduledStateID` without consulting `run_state` as primary authority.
3. A downstream `ready` stage created before interruption does not get silently demoted into misleading blocked failure truth.
4. Live `ExecutionService` reconciliation does not demote a completed-stage boundary solely because the latest live event is `.sessionClosed`.
5. `run_report`, recovery UI, and shell projections distinguish:
   - last completed stage,
   - next scheduled stage,
   - and whether next-stage execution ever started.
6. Interrupted-transition proof tests pass for the canonical non-UI lane.
7. The focused `EA93E855`-class interrupted-transition proof scenario is green on the same tree.
8. The recurring class of failures where a run "falls again at the same place" after session close and restart is no longer reproducible on that proof scenario.

---

## 9. Alternatives Considered

### 9.1 Keep improving recovery heuristics

Rejected.
This is what the system has already been doing: prefer one stale row, then another, then `run_state`, then report synthesis.
It does not remove the underlying ambiguity.

### 9.2 Trust `run_state` as canonical continuation owner

Rejected.
`run_state` is workflow-authored evidence, not engine-owned progression truth.
It can lag behind or disagree with already-materialized stage rows.

### 9.3 Remove scheduled downstream `StageExecution` rows entirely

Not chosen as the primary proposal.
That may become a later simplification, but this proposal does not require it.
The first priority is authoritative continuation ownership, not whether the downstream row exists before stage start.

### 9.4 Continue to treat `sessionClosed` as the strongest resume signal

Rejected.
`sessionClosed` is transport evidence, not workflow progression truth.
It may explain why execution stopped, but it must not decide where the run should continue.
