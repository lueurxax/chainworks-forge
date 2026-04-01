# 20. Dynamic Cycle Addition

*   **Status**: Draft
*   **Author**: Goose
*   **Date**: 2026-04-01

## Summary

This proposal introduces a mechanism to dynamically add extra execution cycles to a `loop` state within an already running workflow. This allows operators to extend a run beyond its initially planned number of iterations without needing to restart it, preserving the current state and progress.

## Motivation

During a long-running or complex workflow, it may become apparent that the initially specified number of cycles for a looping state is insufficient. The current architecture compiles an immutable `RunPlan` where the maximum number of iterations (`resolvedMax`) is fixed. The only way to increase the cycle count is to stop the run, edit the workflow's YAML configuration, and start a new run, losing all progress. This is inefficient and disruptive.

This proposal addresses the user need to add cycles "on the fly" to a currently active run, providing flexibility and improving the operator experience.

## Proposed Solution

The solution is to introduce an override mechanism that is layered on top of the existing immutable `RunPlan` architecture.

### 1. Data Model Extension

We will add a new property to the `Run` SwiftData model (defined in `Chainworks Forge/Models/Run.swift`):

```swift
@Model
class Run {
    // ... existing properties
    var loopExtras: [String: Int] = [:]
}
```

This `loopExtras` dictionary will store the number of *additional* cycles requested by the user for a given state. The key will be the `ExecutableState.id` and the value will be the integer number of extra cycles to add.

### 2. Orchestrator Logic Modification

The `WorkflowOrchestrator` will be updated to account for these extra cycles. In the `executeState` function (within `Chainworks Forge/Engine/WorkflowOrchestrator.swift`), the logic that checks if a loop should terminate will be modified.

**Current Logic:**
```swift
// Pseudocode from analysis
let currentCount = run.loopCounters[state.id, default: 0]
let newCount = currentCount + 1
if newCount >= loop.resolvedMax {
    // End loop
}
```

**Proposed New Logic:**
```swift
// Pseudocode for proposal
let currentCount = run.loopCounters[state.id, default: 0]
let newCount = currentCount + 1

let extraCycles = run.loopExtras[state.id, default: 0]
let effectiveMax = loop.resolvedMax + extraCycles

if newCount >= effectiveMax {
    // End loop
} else {
    // Continue loop
}
```
This change ensures that the orchestrator calculates an `effectiveMax` by adding the user-requested `extraCycles` to the original `resolvedMax` from the `RunPlan`.

### 3. API for Updating Cycles

A new service method will be exposed, likely in `ExecutionService.swift`, to allow the UI and other clients to request additional cycles.

```swift
// Pseudocode for proposal
class ExecutionService {
    // ...
    func addCycles(toRun runId: UUID, forState stateId: String, count: Int) throws {
        guard let run = // fetch Run by runId
        else {
            throw RunNotFoundError(runId)
        }

        // Add to existing extras, or set new value
        let currentExtras = run.loopExtras[stateId, default: 0]
        run.loopExtras[stateId] = currentExtras + count
    }
}
```
This method will safely fetch the specified `Run`, update its `loopExtras` dictionary, and persist the change. The logic adds to any existing extra cycles that may have been requested previously.

## Impact

*   **Positive**: Provides a much-needed feature for operators, increasing flexibility and preventing data loss.
*   **Minimal Intrusion**: The change is additive and does not require a risky rewrite of the core `RunPlan` or compiler architecture. It respects the immutability of the original plan while allowing for controlled, stateful overrides.
*   **UI/Client Changes**: The UI will need to be updated to include a control (e.g., a button or text field) that calls the new `addCycles` API endpoint.

## Alternatives Considered

### Modifying the `RunPlan`
Directly mutating the live `RunPlan` was considered and rejected. The `RunPlan`'s immutability is a core design principle providing stability and predictability. Introducing mutability would create significant complexity and risk, with potential for race conditions and inconsistent state. The proposed solution avoids this entirely.
