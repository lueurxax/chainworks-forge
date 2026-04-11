# Atomic Transition Settlement and Durable Resume Cursor (former P035)

## Status
- **Implemented and stabilized**: 2026-04-11
- **Primary contract owners**: execution persistence, recovery coordination, report projection
- **Evidence source**: execution truth/recovery implementation and canonical run projection paths

## Purpose
This document replaces the proposal-level text for resume truth. It defines the production contract for atomic transition settlement and cursor-driven recovery in the current codebase.

## Core model
1. **Cursor is continuation authority**
   - `Run`-level transition cursor is the authoritative continuation signal used for resume and recovery derivation.
   - `currentStageID` resolution is cursor-first. If cursor metadata is present, projection and UI-facing stage state follow it; `stageExecutions` order is a compatibility fallback only.
   - This prevents stale mixed stage rows from overriding the true continuation path.

2. **Atomic transition settlement**
   - Transition completion and cursor update are treated as a single settlement unit in the execution persistence flow.
   - Recovery does not infer continuation from partial stage snapshots alone.

3. **Resume semantics**
   - Resume/restart chooses the cursor-derived continuation state where available.
   - Interrupted transition paths keep intermediate marker state so the system can surface exact interruption and continue deterministically.
   - For mixed or incomplete historical data, cursor-first selection is the tie-breaker, with stage-execution priority ordering only as a fallback.

4. **Projection and reports**
   - Workflow-map projection derives UI and orchestration current stage from cursor data.
   - Report/recovery builders use cursor provenance for truthful continuation labels before stage aggregate views.

## Proof obligations
- **Truth over convenience**: no UI-facing stage can claim resumable state without matching cursor continuity.
- **Determinism under mixed state**: repeated projection/recovery cycles converge to the same cursor-derived stage.
- **Atomicity under interruption**: resumed runs never lose transition intent when partial transition state is present.

## Implementation surface
- `Run` cursor metadata and derived-stage helpers.
- `WorkflowMapProjectionService` cursor-first map projection.
- `RunReportBuilder` and `RecoveryCoordinator` continuation derivation paths.
- Resume pathways in run orchestration and execution event handlers.

## Evidence
- Canonical engine/source of truth in `Chainworks Forge/Models/Run.swift`, `Engine/WorkflowMapProjectionService.swift`, `Engine/RecoveryCoordinator.swift`, `Engine/RunReportBuilder.swift`.
- Resume and recovery tests in run orchestration/test suites.

## Related stable docs
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md)
- [domain-model.md](domain-model.md)
