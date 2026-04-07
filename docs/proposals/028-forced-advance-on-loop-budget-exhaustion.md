# Proposal 028: Forced Advance on Loop Budget Exhaustion

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Claude / Andrey Khasanov |
| Depends on | [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/domain-model.md](../reference/domain-model.md) |
| Scope | Add a third loop-budget-exhaustion policy that forces the run forward to the next logical state instead of blocking or failing. |
| Goal | When a looped stage exceeds its iteration budget, the system should be able to automatically advance to the next state (e.g., review) rather than requiring manual operator intervention. |

---

## 1. Context and Motivation

The orchestrator supports looped states where an agent repeats until a self-assessed completion condition is met. Each loop has a budget (`max`) that caps the number of iterations. When the budget is exhausted, the current `failure_policy.on_loop_budget_exhausted` offers two options:

- **`pause_and_require_human`** (default) — blocks the run and waits for manual intervention.
- **`fail_run`** — terminates the entire run.

Neither option serves the common case where the agent has made substantial progress but keeps reporting itself as incomplete. In run `EA93E855`, the `code_writer` looped 22 times through `state_8_implementation_continued`, producing green tests on every iteration, but never set `seemingly_complete: true` because it always saw more work it could do. The operator had to manually edit the `implementation_self_assessment` artifact to unblock the run.

This is a workflow design problem, not an agent quality problem. A conservative agent will always find more work to do. The system needs a policy that says: "you've had enough iterations — let the reviewer decide if the work is sufficient."

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the orchestrator automatically advance a run past a looped state when the iteration budget is exhausted, without operator intervention?
2. Is the forced advance recorded in run provenance so operators and reviewers know it happened?
3. Can workflow authors choose per-loop whether exhaustion should block, fail, or advance?
4. Does the reviewer stage receive enough context to know that the work was force-advanced, not self-assessed as complete?
5. Does the forced advance preserve all accumulated artifacts from the exhausted loop?

---

## 3. Scope

This proposal includes:

- a new `on_loop_budget_exhaustion` policy value: `advance_to_next_state`
- per-loop override of the global exhaustion policy
- a synthetic `budget_exhausted` transition condition for workflow authors who want explicit control
- provenance recording: timeline event, run report entry, and artifact annotation
- operator notification that a forced advance occurred

This proposal does **not** include:

- changes to how agents self-assess completion (that remains the agent's contract)
- automatic budget increases or dynamic budget adjustment
- timeout-based forced advance (only iteration-count-based)
- changes to the `pause_and_require_human` or `fail_run` policies
- cross-run learning about optimal budgets (that belongs in Proposal 023 analytics)

---

## 4. Core Product Behavior

### 4.1 New exhaustion policy: `advance_to_next_state`

When a loop's iteration counter reaches `max` and the exhaustion policy is `advance_to_next_state`:

1. The orchestrator completes the current iteration normally (agent runs, artifacts are written).
2. Instead of blocking or failing, the orchestrator injects a synthetic `budget_exhausted` flag into the transition evaluation context.
3. Transition evaluation proceeds with the agent's actual outputs **plus** the synthetic flag.
4. If no transition matches with the actual outputs alone, the orchestrator re-evaluates transitions with overridden completion fields (e.g., `seemingly_complete` forced to `true`).
5. The first matching transition is taken — advancing the run to the next state.

### 4.2 Per-loop exhaustion override

Workflow authors can override the global policy at the loop level:

```yaml
state_8_implementation_continued:
  owner: code_writer
  loop:
    counter: implementation_progress_count
    max: vars.max_implementation_progress_cycles
    on_budget_exhausted: advance_to_next_state
  transitions:
    - to: state_9_implementation_reviewed
      when: implementation_self_assessment.seemingly_complete == true
    - to: state_8_implementation_continued
      when: implementation_self_assessment.seemingly_complete == false
```

If `loop.on_budget_exhausted` is present, it takes precedence over `failure_policy.on_loop_budget_exhausted`. If absent, the global policy applies.

### 4.3 Synthetic `budget_exhausted` transition condition

For workflow authors who want explicit transition control on exhaustion:

```yaml
transitions:
  - to: state_9_implementation_reviewed
    when: budget_exhausted == true
  - to: state_9_implementation_reviewed
    when: implementation_self_assessment.seemingly_complete == true
  - to: state_8_implementation_continued
    when: implementation_self_assessment.seemingly_complete == false
```

When `advance_to_next_state` is active and the budget is exhausted, the orchestrator sets `budget_exhausted = true` in the evaluation context. This allows workflows to define a distinct transition path for exhaustion (e.g., advancing to a different review state or adding a warning stage).

If no explicit `budget_exhausted` transition exists, the fallback behavior from 4.1 applies (override the blocking field).

### 4.4 Provenance and operator visibility

A forced advance must not be silent. The system records:

1. **Timeline event**: A `budgetExhaustedAdvance` event in the live timeline with the loop counter name, final count, max, and the transition taken.
2. **Run report entry**: The run report includes a `forced_advances` section listing each forced advance with timestamp, state, counter, and destination state.
3. **Artifact annotation**: The overridden artifact (e.g., `implementation_self_assessment`) is **not mutated**. Instead, a sibling annotation artifact `implementation_self_assessment.budget_override` records the original value and the override applied.
4. **Stage execution metadata**: The `StageExecution` for the exhausted state records `budgetExhaustedAt` and `forcedAdvanceDestination`.

### 4.5 Reviewer context

When the destination state (e.g., `state_9_implementation_reviewed`) receives a force-advanced run:

- The reviewer agent's inputs include the original (non-overridden) `implementation_self_assessment` showing `seemingly_complete: false`.
- The reviewer also receives the `budget_override` annotation so it knows the advance was forced.
- This allows the reviewer to make an informed judgment: the work may be sufficient despite the agent's conservative self-assessment, or it may genuinely need more iteration — in which case the reviewer can route back through `state_10_implementation_refined`.

---

## 5. Architecture

### 5.1 `WorkflowOrchestrator.handleLoopBudgetExhausted()`

Current implementation (line 2759) handles two cases. Add a third:

```swift
case "advance_to_next_state":
    // Record provenance
    recordBudgetExhaustedAdvanceEvent(state: state, counter: counter, count: count)
    // Inject budget_exhausted flag into transition context
    artifactFields["_system"] = ["budget_exhausted": .bool(true)]
    // Do NOT block or fail — let transition evaluation proceed
```

If transition evaluation still fails to find a match (e.g., agent output says `false` and no `budget_exhausted` transition exists), the orchestrator applies field overrides to completion-gate fields and re-evaluates.

### 5.2 `TransitionEvaluator`

No changes to evaluation logic needed. The `budget_exhausted` flag is injected as a regular artifact field and evaluated through existing expression parsing.

### 5.3 `RunPlan.ResolvedLoopConfig`

Add optional `onBudgetExhausted: String?` field. When present, overrides `plan.failurePolicy.onLoopBudgetExhausted` for this specific loop.

### 5.4 Workflow YAML schema

Add optional `on_budget_exhausted` key under `loop:`. Valid values: `pause_and_require_human`, `fail_run`, `advance_to_next_state`.

### 5.5 `StageExecution` model

Add optional fields:
- `budgetExhaustedAt: Date?`
- `forcedAdvanceDestination: String?`

### 5.6 Run report

Add `forced_advances: [ForcedAdvanceEntry]` to the report model:
```swift
struct ForcedAdvanceEntry: Codable {
    let stateID: String
    let counter: String
    let count: Int
    let max: Int
    let destinationStateID: String
    let timestamp: Date
}
```

---

## 6. Acceptance Criteria

1. A workflow with `on_budget_exhausted: advance_to_next_state` on a loop advances to the next state when the iteration budget is exhausted, without operator intervention.
2. The forced advance is recorded in the run timeline, run report, and stage execution metadata.
3. The destination state's agent receives the original (non-overridden) self-assessment plus a budget override annotation.
4. Per-loop `on_budget_exhausted` overrides the global `failure_policy` when present.
5. The synthetic `budget_exhausted` condition is available in transition expressions.
6. Existing `pause_and_require_human` and `fail_run` behavior is unchanged.
7. A focused `proposal-028` gate in `test-gate.sh` passes on the canonical tree.

---

## 7. Alternatives Considered

### 7.1 Just increase the budget

Rejected. The problem is not that 10 iterations is too few — it's that a conservative agent will never self-assess as complete regardless of budget. Increasing the budget from 10 to 100 would waste compute without changing the outcome.

### 7.2 Timeout-based forced advance

Rejected for this proposal. Wall-clock timeouts introduce non-determinism and are harder to reason about in workflows. Iteration-count budgets are deterministic and already tracked. A future proposal could add time-based policies as a complement.

### 7.3 Teach agents to be less conservative

Not a system-level solution. Agent behavior depends on the LLM, the prompt, and the task. The system should handle the case where an agent is doing good work but never declares itself done, rather than relying on all agents being perfectly calibrated.

### 7.4 Mutate the self-assessment artifact directly

Rejected. Overwriting `seemingly_complete: false` with `true` in the actual artifact destroys provenance truth. The reviewer should see the original self-assessment to make an informed judgment. The system records the override as a sibling annotation, not a mutation.
