# Execution Truth and Recovery

Stable reference for the implemented execution-truth, settlement, and recovery slice that was previously tracked by Proposal 016.

## Purpose

The runtime must be able to say, once and only once, what actually happened in an agent attempt after output, timeout, cancellation, limit exhaustion, relaunch, and recovery.

This document is the stable contract for:

- canonical terminal outcomes on `AgentExecution`,
- stage-level settlement and recovery evidence on `StageExecution`,
- approval restoration and resume behavior,
- frozen-vs-runtime binding truth in reports,
- and recovery/report readers that must prefer persisted truth over heuristic reconstruction.

For implementation/proof status, use [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md).

## Scope

This reference covers:

- agent-level terminal outcome classification,
- persisted truth columns versus supporting diagnostic envelopes,
- stage-level failure and recovery evidence,
- resume / approval-restore behavior after interruption,
- runtime binding truth as read by reports and operator surfaces,
- and the current proof-owning test suites for this slice.

It does not replace:

- the broader engine topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run snapshots in [runtime-contract.md](runtime-contract.md),
- provider setup/platform behavior in [provider-platform.md](provider-platform.md),
- or operator-shell interaction rules in [operator-experience.md](operator-experience.md).

## Core Rules

### One canonical terminal outcome per agent attempt

Every settled `AgentExecution` uses exactly one canonical terminal outcome:

- `completed`
- `completed_with_transport_error`
- `failed_before_output`
- `failed_after_output_validation`
- `timed_out_before_output`
- `timed_out_after_output`
- `cancelled_before_output`
- `cancelled_after_output`
- `limit_exhausted_before_output`
- `limit_exhausted_after_output`

These values live in [`Chainworks Forge/Models/ExecutionTruth.swift`](<../../Chainworks Forge/Models/ExecutionTruth.swift>) and are persisted on `AgentExecution.canonicalOutcome`.

### Neutral finish markers are not success on their own

Transport finish markers such as `stop` or `session_closed` describe how streaming ended.
They do not by themselves prove successful completion.

Current classification rules in `RuntimeAgentExecutor` therefore require more than a neutral finish marker:

- durable output plus later transport failure becomes `completed_with_transport_error`,
- timeout before output becomes `timed_out_before_output`,
- timeout after output becomes `timed_out_after_output`,
- provider/app limit exhaustion becomes one of the explicit `limit_exhausted_*` outcomes,
- neutral stop with no durable output remains failure, not silent success.

### Flattened persisted columns outrank envelopes and receipts

The primary persisted execution-truth columns on `AgentExecution` are:

- `canonicalOutcome`
- `supervisionClassification`
- `transportErrorKind`
- `providerStopReason`
- `outputPresence`
- `settledAt`
- `runtimeProvider`
- `runtimeModel`

`outcomeEnvelopeJSON` is supporting diagnostic evidence.
It exists to explain the settled outcome, not to compete with it.

Readers must use this precedence:

1. flattened persisted execution-truth columns,
2. supporting evidence such as `outcomeEnvelopeJSON`, `providerReceiptJSON`, and validation payloads,
3. coarse legacy fields like `AgentStatus` only when canonical columns are absent.

Raw receipts or transcripts must never silently override canonical persisted outcome truth.

### Watchdog-specific truth refines, but does not replace, canonical outcome

`supervisionClassification` is the durable refinement field for watchdog-specific execution truth.

The stable contract is:

- `canonicalOutcome` remains the terminal execution state,
- `supervisionClassification` carries watchdog-specific or integrity-specific refinement such as:
  - `idleHangBeforeFirstProgress`
  - `idleHangAfterProgress`
  - `idleHangReadLoop`
  - `idleHangAfterFirstEdit`
  - `mutationSideEffectMissing`
- `transportErrorKind` and `providerStopReason` remain orthogonal transport/provider evidence,
- `outcomeEnvelopeJSON` and receipts explain the settled truth but do not redefine it.

Readers must therefore interpret agent-level execution truth in this order:

1. `canonicalOutcome` for terminal state,
2. `supervisionClassification` for watchdog-specific refinement,
3. `transportErrorKind` and `providerStopReason` for transport/provider context,
4. evidence payloads only as supporting detail.

## Stage Truth and Recovery Evidence

### `StageExecution` is the stage-level owner

Stage-level truth remains anchored on `StageExecution`.
The current persisted stage fields for this slice are:

- `lineageID`
- `settlementKind`
- `settledAt`
- `activeOwnerToken`
- `validationFailureJSON`
- `evidencePacketJSON`
- `recoverySnapshotJSON`

The important contract is ownership, not file shape:

- stage terminality belongs to the stage record,
- failed-stage evidence belongs to the stage record,
- recovery recommendations belong to the stage record,
- reports and recovery surfaces read the stage record first instead of inferring truth from loose artifact scans.

`recoverySnapshotJSON` is stage-owned next-action truth, not agent-level execution truth.
It may narrow the operator action after a watchdog failure or exhausted retry, but it must not override the settled `AgentExecution` truth described above.

### Recovery uses the narrowest valid next action

`StageRetryCoordinator` persists and rebuilds `RecoveryActionSnapshot` values that describe the narrowest valid next step:

- retry failed agent,
- retry failed stage,
- operator inspection first,
- clone run from frozen snapshot,
- clone run from current config.

`RunReportBuilder` and `RecoveryCoordinator` consume these snapshots directly when present and synthesize them from stage evidence only as a fallback.

## Resume and Approval Restore

### Resume is fail-closed

`ResumeManager` does not blindly restart work.
It classifies interrupted runs as:

- resumable,
- needing operator decision,
- or not resumable.

That classification already considers:

- compiler-version mismatches,
- frozen snapshot rebuild failure,
- workflow or catalog drift,
- side-effect-stage interruption,
- and frozen workspace-path validity.

### Approval restore preserves operator context

Approval-bound runs are allowed to restore visible pending approval context after relaunch.
The contract is:

- approval gates restore the same operator decision point when the persisted state still supports it,
- drift can be surfaced as context without silently discarding the approval state,
- recovery or report readers must not invent a new approval truth that was not persisted.

`Approval.lineageID` and `Approval.repairedAt` exist as persisted approval-truth fields for this slice; consumers should treat them as the canonical lineage metadata when present.

## Runtime Binding Truth

Execution truth is not only about success or failure.
Reports also need to say what provider/model actually ran.

The current read path combines:

- run-level frozen intent and trust metadata on `Run`,
- frozen provenance in `bindingProvenanceJSON`,
- and runtime provider/model evidence persisted per `AgentExecution`.

Rules:

1. frozen binding intent remains historical context, not reconstructed guesswork,
2. runtime provider/model evidence should be shown when present,
3. weak or contradictory runtime evidence should downgrade trust instead of manufacturing certainty.

The narrower binding contract is documented in [provider-binding-truth.md](provider-binding-truth.md).

## Recovery and Report Read Order

Current report/recovery readers should prefer:

1. `AgentExecution` execution-truth columns,
2. `StageExecution` failure and recovery payloads,
3. run-level trust / provenance metadata,
4. coarse legacy statuses only as compatibility fallback.

This keeps report timelines, failed-step summaries, retry hints, and resume guidance tied to persisted truth rather than heuristic rescans of historical artifacts.

## Verification and Proof Owners

This slice is currently proved primarily through current-head non-UI test suites rather than a dedicated standalone wrapper gate.

High-signal proof owners include:

- `RuntimeAgentExecutorTests` for transport-outcome classification and limit exhaustion,
- `OrchestratorTests` for persistence of canonical outcome, provider/model truth, and validation-after-output settlement,
- `ResumeManagerTests` for interrupted-run classification and approval restore behavior,
- `RecoveryCoordinatorTests` for narrow recovery action ownership,
- `Proposal013Tests` for failed-stage evidence and report/recovery fallback behavior.

For the consolidated proof story, use [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md).

## Adjacent References

Use:

- [runtime-contract.md](runtime-contract.md) for frozen snapshots and artifact boundaries,
- [workflow-execution-engine.md](workflow-execution-engine.md) for orchestrator topology,
- [run-control.md](run-control.md) for cancellation settlement and operator-visible cancel truth,
- [provider-binding-truth.md](provider-binding-truth.md) for historical binding provenance,
- [operator-experience.md](operator-experience.md) for shell/report/recovery presentation contracts.
