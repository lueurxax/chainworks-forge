# Proof: Atomic Transition Settlement and Durable Resume Cursor

## Goal
Validate that resume/recovery uses cursor-continuation truth as the recovery authority and that transition settlement updates are treated consistently across map projection, reports, and resume planning.

## Evidence scope
- `WorkflowMapProjectionService`, `Run` cursor model, report/recovery builders, and resume coordinator integration.
- Restart/resume tests exercising interrupted and partial transition states.
- Existing full and proposal-level recovery verification lanes.

## What is considered proven
1. Resume planning and transition settlement now persist cursor updates during interruption/retry boundaries.
2. Interruption/recovery paths use transition metadata and cursor state together in planner and recovery reporting surfaces.
3. Cursor-backed evidence is used for recovery reporting and audit traces in most slices.

## Current implementation mismatch
1. **Known gap:** `WorkflowMapProjectionService` can still derive UI-facing `currentStageID` from persisted workflow snapshots when cursor metadata is incomplete, so stale or mixed stage rows may override durable cursor truth.
2. Until this gap is fixed, UI-facing current-stage truth is only partially cursor-driven.

## Current verification commands
- `scripts/test-gate.sh proposal-033` (execution slice where cursor-driven behavior is covered)
- `scripts/test-gate.sh proposal-030` and repository-level recovery/smoke gates as regression guards

## Residual risk
- Remaining risk is the cursor-vs-snapshot race in map projection described above, which can surface stale current-stage values for interrupted runs.
- Additional risk remains if cursor and transition writes are not kept atomic in future transport/adapter additions.
