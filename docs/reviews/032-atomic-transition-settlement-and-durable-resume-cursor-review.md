# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/032-atomic-transition-settlement-and-durable-resume-cursor.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/021-run-transition-notifications-and-attention-routing.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/032-atomic-transition-settlement-and-durable-resume-cursor-review.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/032-atomic-transition-settlement-and-durable-resume-cursor-evidence-pack.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
- Baseline reused:
  - repo-level baseline shape
  - stable reference docs for execution truth, runtime contract, and persistence model
  - prior `P032` red-pass review/evidence as a stale basis check
- Baseline refreshed:
  - targeted reread of the updated proposal text
  - targeted code refresh for `Run`, `ResumeManager`, `ExecutionService`, `WorkflowOrchestrator`, `RunReportBuilder`, `RecoveryCoordinator`, and workflow-map/read-model surfaces
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - none present
- Targeted context refresh performed:
  - yes, repo-local only
- External research used: `None`
- Research pack:
  - none
- Sources reused:
  - stable reference docs and current baseline
  - prior `P032` review/evidence artifacts as stale-basis comparators
- Sources refreshed:
  - current proposal text and focused current code paths
- Time-sensitive external guidance:
  - none
- Code areas inspected:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
- Current repo contradictions found:
  - current continuation truth is still split in implementation across `run.currentStageID`, `resumeContinuationStateID`, stale stage rows, `run_state`, startup normalization, and live `sessionClosed` reconciliation
  - no live `proposal-text` contradiction remains after the updated draft explicitly rebinds live stalled-run reconciliation (`§5.5`, `§6.5`), shell/read-model surfaces (`§6.6`), pre-cursor fallback (`§6.8`), notification ordering (`§6.10`), rollout (`§7`), and acceptance (`§8`)
- Runtime evidence used: `None`
- Provenance of key evidence:
  - local proposal/docs + current code inspection + reusable baseline + stale-artifact freshness check
- Remaining assumptions:
  - no hidden transition cursor already exists outside checked-in repo code
- Remaining blockers:
  - none

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong and implementation-ready`
- Top residual implementation risks:
  1. Implementation must actually land the live `ExecutionService` cursor-first reconciliation path, not only the restart-side resume changes.
  2. `run.currentStageID` must stay non-canonical or cursor-derived during migration so shell/read-model surfaces do not regress to heuristic stage authority.
  3. Pre-cursor fallback must remain visibly degraded so historical runs do not silently masquerade as cursor-backed truth.
- Top opportunities:
  1. Keep the focused `proposal-032` proof lane aligned to both restart and live `.sessionClosed` interruption paths.
  2. Make the shell/read-model migration explicit in implementation exactly as the draft now does in `§6.6`.
  3. Preserve `Proposal 021` notification subordination by firing only after settlement commit.

## 2. Proposal Scope and Completeness
- In scope:
  - run-owned durable continuation cursor
  - atomic transition settlement
  - resume targeting and startup normalization changes
  - live stalled-run reconciliation changes
  - report/recovery cursor-first read order
  - operator-facing transition truth cleanup
  - notification ordering relative to settlement commit
- Out of scope:
  - YAML redesign
  - agent output contract changes
  - ACP transport changes
  - loop-budget changes
  - historical bulk backfill cleanup
- Deferred intentionally:
  - possible later simplification that removes pre-start scheduled downstream stage rows
- Most important baseline refreshes performed:
  - current `Run.currentStageID` and `resumeContinuationStateID` heuristics
  - current startup normalization behavior
  - current `sessionClosed` stalled-run reconciliation
  - current report/recovery readers
  - current workflow-map and summary projections
- Most important contradictions with current repo:
  - none live at the proposal-text level; the updated draft now explicitly covers the owner paths that were previously left implicit
- Most important missing or partial states:
  - none blocking; interrupted-transition, live reconciliation, shell projection, and pre-cursor fallback states are now explicitly specified

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` findings remain. The updated draft now explicitly migrates shell/read-model surfaces in `§6.6` and includes shell-projection proof in `§7` and acceptance criterion `5`.

### 5.2 UX Findings
- No live UX `proposal-text` findings remain. The draft now states honest interrupted-transition reporting in `§5.6` and keeps operator-facing truth aligned across recovery, reports, and shell projections.

### 5.3 iOS Architecture Findings
- No live architecture `proposal-text` findings remain. The updated draft explicitly rebinds live stalled-run reconciliation in `§5.5` and `§6.5`, adds explicit pre-cursor fallback in `§6.8`, and ties rollout plus acceptance to both restart and live interruption paths.

## 6. Cross-Discipline Conflicts and Decisions
- Previously live conflict: the earlier draft fixed restart/manual-resume truth but left live `ExecutionService` reconciliation and broader shell projections implicit.
  Resolution: the updated draft now assigns both paths to the same cursor-first owner chain and explicitly demotes heuristic `currentStageID` authority.
- Decision: `P032` is now implementation-ready for proposal-readiness purposes.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Keep `run.currentStageID` cursor-derived or explicitly non-canonical throughout implementation | iOS Architecture | Implementation owner | During implementation | cursor persistence + shell migration | no shell surface regresses to heuristic single-stage authority | `—` |
| P2 | Keep the focused proof lane covering both restart and live `.sessionClosed` interruption paths | Cross-discipline | Implementation owner | During implementation | `proposal-032` test gate | same-tree proof stays green for restart, live reconciliation, report, and shell projection cases | `—` |
| P2 | Preserve degraded-trust fallback for pre-cursor runs | UX | Implementation owner | During implementation | migration / fallback path | historical runs stay readable without being misrepresented as cursor-backed truth | `—` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Interrupted transition truth | cursor-first settlement, resume, live reconciliation, report/recovery, and shell projection consistency | focused proof for `sessionClosed before next-stage start`; restart proof; shell/read-model projection proof | no blocked/failed rewrite when cursor says next state is only scheduled; no shell surface claims a scheduled-not-started stage has already started | proposal text is ready; next checkpoint is implementation audit | hold implementation sign-off if any path still derives continuation primarily from stale stage rows, `run_state`, or heuristic `currentStageID` |
