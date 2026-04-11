# Proof: Atomic Transition Settlement and Durable Resume Cursor

## Goal
Validate that resume/recovery uses cursor-continuation truth as the recovery authority and that transition settlement updates are treated consistently across map projection, reports, and resume planning.

## Evidence scope
- `WorkflowMapProjectionService`, `Run` cursor model, report/recovery builders, and resume coordinator integration.
- Restart/resume tests exercising interrupted and partial transition states.
- Existing full and proposal-level recovery verification lanes.

## What is considered proven
1. `currentStageID`/current-stage derivation is cursor-first where cursor metadata exists.
2. Interruption and resume paths derive continuation from cursor + immutable transition metadata, not from mixed snapshots alone.
3. Map projection for UI-facing current stage follows durable cursor truth.
4. Recovery reports and stage labels are aligned with cursor-authoritative continuation.

## Current verification commands
- `scripts/test-gate.sh proposal-033` (execution slice where cursor-driven behavior is covered)
- `scripts/test-gate.sh proposal-030` and repository-level recovery/smoke gates as regression guards

## Residual risk
- No unresolved critical risk in current code path; watch for regressions where writes to cursor and transition state become non-atomic in new transport/adapter additions.
