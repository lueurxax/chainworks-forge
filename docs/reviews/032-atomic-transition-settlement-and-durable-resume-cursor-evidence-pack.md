# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/032-atomic-transition-settlement-and-durable-resume-cursor.md` | 2026-04-08 | High | Updated `P032` now explicitly covers live stalled-run reconciliation (`§5.5`, `§6.5`), honest interrupted-transition reporting (`§5.6`), shell/read-model rebinding (`§6.6`), pre-cursor fallback (`§6.8`), notification ordering (`§6.10`), rollout (`§7`), and acceptance (`§8`). | Review could judge against a stale draft and preserve already-closed findings. | Core proposal |
| DOC-02 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-08 | High | Current recovery/report readers must prefer persisted truth over heuristic reconstruction, but the reference still does not already provide a transition cursor. | Proposal must extend the current truth hierarchy without breaking it. | Core dependency |
| DOC-03 | `docs/reference/runtime-contract.md` | 2026-04-08 | High | Stable runtime status machines remain `RunStatus`, `StageStatus`, and related enums; resume/retry policy already exists as baseline truth. | Proposal could open a parallel progression state machine unless it maps cleanly to current runtime truth. | Core dependency |
| DOC-04 | `docs/reference/domain-model.md` | 2026-04-08 | High | `Run.currentStageID` is currently a computed property derived from `StageExecution` ordering, and that behavior is treated as baseline persistence-model truth. | Proposal must explicitly rebind or constrain this current derived owner. | Core dependency |
| DOC-05 | `docs/reference/current-system-baseline.md` | 2026-04-08 | High | Chainworks Forge already has a stable operator shell, workflow map, recovery surfaces, and persisted execution engine baseline. | Proposal must fit implemented shell/read-model reality, not an imagined greenfield shell. | Baseline orientation |
| DOC-06 | `docs/proposals/021-run-transition-notifications-and-attention-routing.md` | 2026-04-08 | Medium | `P021` already positions notifications as a shell-owned delivery layer on top of canonical run/stage truth. | Transition truth changes must keep notification delivery subordinate to settlement. | Adjacent dependency |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo shape, operator shell, execution model, reference-doc map | 2026-04-08 | High | Still valid as accelerator; execution/recovery/read-model slices required direct code refresh. | Review setup |
| BASE-02 | Proposal-specific integration context | Missing | none | 2026-04-08 | High | No existing `032-atomic-transition-settlement-and-durable-resume-cursor.review/integration-context.md` was present. | None blocking |
| BASE-03 | Prior `P032` review/evidence artifacts | Partially refreshed | prior red-pass findings | 2026-04-08 | High | Previous review/evidence were used as stale-basis comparators and superseded after the proposal changed. | Freshness check |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - durable run-owned continuation cursor
  - atomic transition settlement between completed state and next scheduled state
  - cursor-first resume targeting
  - startup normalization changes
  - cursor-first report/recovery read order
  - live stalled-run reconciliation changes
  - operator-facing transition-truth cleanup
  - notification ordering after settlement commit
- Out of scope:
  - YAML redesign
  - agent output contract changes
  - ACP transport changes
  - loop-budget policy changes
  - historical bulk backfill cleanup
- Deferred intentionally:
  - any later simplification that removes scheduled downstream `StageExecution` rows entirely
- Assumptions:
  - no hidden transition cursor already exists outside the checked-in repo
  - current shell/read-model surfaces in repo are the relevant implementation baseline
- Open questions:
  - none blocking; the updated draft now specifies the owner chain and shell-mapping requirement tightly enough for proposal readiness
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `BlockedRunRecoveryView` | Targeted refresh | 2026-04-08 | High | The blocked-run recovery surface still derives its displayed current stage from `run.currentStageID`, and the updated proposal now explicitly folds that path under cursor-first recovery/read-model migration. | Implementation could still miss this surface if it only updates data-layer readers. | Recovery UI |
| NAV-02 | `WorkflowMapProjectionService` / workflow map | Targeted refresh | 2026-04-08 | High | The workflow map currently projects `currentStageID/currentStageLabel` from `Run.currentStageID`, and `§6.6` now explicitly requires that projection to migrate. | Shell could regress if implementation ignores the proposal’s explicit remapping rule. | Operator shell |
| NAV-03 | `RunsHomeView` and `IdeaListView` run summaries | Targeted refresh | 2026-04-08 | High | Current summary surfaces display `Current Stage` directly from `run.currentStageID`, and `§6.6` now explicitly names those surfaces as migration targets. | Summary surfaces could remain heuristic if implementation only fixes recovery/report readers. | Operator shell |
| NAV-04 | `RunReportBuilder` and recovery/report paths | Targeted refresh | 2026-04-08 | High | Current report/recovery paths already prefer persisted stage truth, and `§6.4`/`§8.5` now explicitly place the cursor first in that reader order. | Reader precedence must be implemented exactly as specified. | Report/recovery |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/Run.swift` / `currentStageID`, `resumeContinuationStateID`, `interruptedTransitionResumeStateID` | Persistence/read model | Current continuation heuristics on the run aggregate | 2026-04-08 | High | `currentStageID` remains a derived single-stage heuristic, while `resumeContinuationStateID` already contains a ready-stage heuristic that outranks stale `run_state`. The updated proposal now explicitly demotes heuristic ownership and requires cursor-derived or non-canonical treatment. | Implementation must not preserve heuristic authority by accident. | Core migration |
| MAP-02 | `Chainworks Forge/Engine/ResumeManager.swift` / `normalizeInterruptedRunsForManualResume()` | Resume | Current startup normalization owner | 2026-04-08 | High | Current startup normalization blocks all `.pending/.ready/.running` runs and converts their stages/agents into blocked/failed truth. `§6.3` now explicitly preserves scheduled-but-not-started continuation truth. | Restart-side false demotion should now be fully specified. | Core dependency |
| MAP-03 | `Chainworks Forge/Engine/ExecutionService.swift` / stalled-run reconciliation (`stalledStage`, `shouldReconcileStalledRun`) | Live execution | Current in-process `sessionClosed` interruption owner | 2026-04-08 | High | Current live reconciliation can still mark runs blocked and agents failed based on `currentStageID` plus latest `.sessionClosed`. `§5.5` and `§6.5` now explicitly reassign this path to cursor-first truth. | Implementation must land the now-explicit migration, but the proposal-text gap is closed. | Former blocker, now specified |
| MAP-04 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` / `executeStateMachine()` | Execution engine | Current transition progression owner | 2026-04-08 | High | Current orchestrator mutates in-memory `currentStateID` after transition evaluation and lazily creates downstream `StageExecution` rows on entry. `§6.2` still correctly targets this seam for atomic settlement. | Settlement must remain the single writer of progression truth. | Core dependency |
| MAP-05 | `Chainworks Forge/Engine/RunReportBuilder.swift` | Reports | Current persisted report read path | 2026-04-08 | High | Reports already prefer persisted stage/agent truth but still rely on stage lineage and `run.currentStageID` metadata rather than a run-owned cursor. `§5.6` and `§6.4` now explicitly tell reports how to describe interrupted-transition boundaries honestly. | Report wording and reader order are now specified. | Report owner |
| MAP-06 | `Chainworks Forge/Engine/WorkflowMapProjectionService.swift` | Shell projection | Current workflow-map current-stage owner | 2026-04-08 | High | Workflow map uses `run.currentStageID` as the single “current” truth today, and `§6.6` now explicitly requires cursor-based projection migration for that surface. | Shell projection remains an implementation task, not a proposal-text gap. | Shell migration |
| MAP-07 | `Chainworks Forge/Engine/RecoveryCoordinator.swift` | Recovery | Current action selection and failure snapshot reader | 2026-04-08 | High | Recovery is stage-first and snapshot-first today; `§6.4` now explicitly puts the run-owned cursor first in the reader precedence chain. | Recovery readers must preserve the new precedence order. | Recovery owner |
| MAP-08 | `Chainworks Forge/Views/BlockedRunRecoveryView.swift` | UI | Current blocked-run stage label owner | 2026-04-08 | High | UI still derives the displayed current stage from `run.currentStageID`, and `§6.6` now explicitly treats that class of shell/read-model surface as part of the migration. | UI implementation must avoid stale single-stage labels. | UI migration |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Run-owned mutable execution truth | `Run.swift`, `domain-model.md` | Persisted state | 2026-04-08 | High | `Run` currently owns mutable execution metadata and frozen snapshots, but not yet a persisted transition cursor. The proposal now unambiguously assigns that new owner role to `Run`. | Cursor must be added without conflicting with current run truth. | Core change |
| DATA-02 | Stage truth and recovery evidence | `StageExecution.swift`, `execution-truth-and-recovery.md` | Persisted stage truth | 2026-04-08 | High | Stage records own stage settlement, evidence packets, and recovery snapshots. The proposal keeps stage truth stage-owned while making continuation cursor run-owned. | Ownership split is now explicit and coherent. | Ownership boundary |
| DATA-03 | Restart normalization and live interruption handling | `ResumeManager.swift`, `ExecutionService.swift` | Runtime mutation | 2026-04-08 | High | Current restart and live `sessionClosed` flows both mutate runs/stages/agents into blocked/failed truth. The updated proposal now explicitly covers both paths. | Implementation must keep both paths on one owner chain. | Former blocker, now specified |
| DATA-04 | Workflow-authored `run_state` artifacts | `GooseSessionBridge.swift`, tests, proposal text | Artifact evidence | 2026-04-08 | High | `run_state` is still present as workflow evidence and current tests already guard stale-override behavior. `§6.7` now explicitly keeps it secondary. | Evidence-only boundary must stay intact. | Evidence-only boundary |
| DATA-05 | Pre-cursor historical runs | `P032 §6.8` | Fallback behavior | 2026-04-08 | High | The updated proposal now explicitly specifies degraded-trust fallback for runs that lack the new cursor field. | Historical readability/resumability is no longer undefined. | Migration completeness |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Derived run-stage owner | Current repo | 2026-04-08 | High | `Run.currentStageID` is still the broad operator-shell current-stage read model. The updated proposal now explicitly constrains it to cursor-derived compatibility or demoted authority. | No live text contradiction remains. | Shell/read-model migration |
| INT-02 | Startup normalization | Current repo | 2026-04-08 | High | Restart normalization aggressively blocks stale in-flight work today. `§6.3` now explicitly retargets this seam. | Proposal is aligned with the current seam. | Proposal-aligned |
| INT-03 | Live `sessionClosed` reconciliation | Current repo | 2026-04-08 | High | `ExecutionService` already reacts to `sessionClosed` before relaunch and can flatten truth immediately. `§5.5` and `§6.5` now explicitly cover that path. | Proposal is aligned with the current seam. | Proposal-aligned |
| INT-04 | Recovery/report readers | Current repo | 2026-04-08 | High | Current report/recovery readers are already persisted-truth-first but stage-centered. `§6.4` now explicitly inserts the cursor as top precedence. | Proposal is aligned with the current seam. | Proposal-aligned |
| INT-05 | Workflow map and summary shell surfaces | Current repo | 2026-04-08 | High | Current shell surfaces still expose a single heuristic current stage. `§6.6` now explicitly assigns those surfaces to the cursor/read-model migration. | Proposal is aligned with the current seam. | Proposal-aligned |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-04 | orchestrator transition path | Core entry seam is clear. |
| Happy path | Specified | DOC-01, MAP-04 | completed state -> next scheduled state | Atomic settlement intent is clear. |
| Loading | Specified | DOC-01, NAV-02, NAV-03, MAP-06 | shell projections | Shell/read-model mapping is now explicitly required. |
| Empty | Specified | DOC-01 | no next transition / terminal cases | Terminal cases are covered. |
| Validation error | Specified | DOC-01, MAP-02, MAP-03 | normalization and interruption handling | Restart and live interruption paths are both explicitly covered. |
| Backend error | Specified | DOC-01, MAP-03 | live session closure before next-stage start | Live `.sessionClosed` demotion path is now explicitly reassigned. |
| Offline / degraded | Specified | DOC-01, MAP-03, DATA-05 | stalled run after session close; pre-cursor fallback | Degraded fallback is explicit rather than undefined. |
| Retry / recovery | Specified | DOC-01, MAP-07 | recovery/report readers | Direction is strong and explicit. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | none | Out of scope. |
| Rollback / cancellation | Specified | DOC-01 | cancellation during settlement | `§6.9` now explicitly covers cancellation/settlement interaction. |

## I. Feature Flags / Rollout / Rollback
No new feature-flag owner is proposed. Rollout is handled through the explicit migration sequence in `§7`.

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | transition-derived notifications and interruption evidence | operator/troubleshooting evidence | settlement commit, stalled-run reconciliation, report generation | 2026-04-08 | Medium | The proposal now explicitly subordinates transition-derived notifications to settlement commit. No live proposal-text instrumentation blocker remains. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Model/read-model | current derived continuation heuristics | `Chainworks_ForgeTests` already lock `currentStageID` and `resumeContinuationStateID` behavior, including materialized `ready` stage vs stale `run_state` | 2026-04-08 | High | Implementation must replace or rebind those tests/surfaces under the cursor-first contract. |
| TEST-02 | Resume | restart normalization | `ResumeManagerTests` already prove that stale running runs are blocked and agents are failed on startup normalization | 2026-04-08 | High | `§6.3` now explicitly retargets this proof surface. |
| TEST-03 | Focused proposal gate | interrupted transition settlement | no current `proposal-032` gate exists | focused non-UI proof for restart and live `sessionClosed` interruption paths, report generation, and shell projection boundaries | 2026-04-08 | High | The updated rollout and acceptance language now explicitly names both restart and live variants. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Live interruption owner | `sessionClosed` should not decide continuation truth | current `ExecutionService` stalled-run path still uses `sessionClosed` plus `currentStageID` to block/fail work | 2026-04-08 | High | The updated proposal now explicitly covers this owner path. |
| REAL-02 | Operator-surface truth | operator surfaces should read one persisted continuation truth | current workflow map and run summaries still key off single `run.currentStageID` heuristics | 2026-04-08 | High | The updated proposal now explicitly maps those surfaces into the cursor/read-model migration. |
| REAL-03 | Restart normalization | startup normalization over-flattens interrupted work | current `ResumeManager` does exactly that today | 2026-04-08 | High | Proposal remains correctly aimed at this seam. |
| REAL-04 | `run_state` authority | `run_state` is evidence, not canonical continuation owner | current tests already contain stale `run_state` override scenarios | 2026-04-08 | High | Proposal direction remains consistent with current repo learning. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | The failure pattern is clearly articulated. |
| Scope boundaries | Specified | DOC-01 | Scope is bounded and coherent. |
| Reusable baseline coverage | Specified | BASE-01, DOC-02, DOC-03, DOC-04, DOC-05 | Baseline coverage is strong with targeted refresh. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03, NAV-04 | Recovery, report, and broader shell surfaces are now explicitly assigned. |
| Navigation / entry points | Specified | NAV-02, NAV-03 | Current operator entry points are now explicitly covered by the migration rule. |
| State handling | Specified | H table, REAL-01, REAL-02 | Restart, live `sessionClosed`, shell projection, and pre-cursor fallback states are now covered. |
| Data / API contract | Specified | DATA-01, DATA-02, DATA-04, DATA-05 | Cursor ownership and fallback boundaries are explicit. |
| Persistence / caching | Specified | DATA-01, DATA-02, DATA-05 | Run/stage/fallback ownership boundaries are coherent. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Not in scope. |
| Feature flags / rollout / rollback | Specified | DOC-01 | Rollout sequence is explicit even without feature flags. |
| Analytics / instrumentation | Specified | METRIC-01 | Notification ordering and operator evidence boundary are now explicit enough for this proposal. |
| Testing strategy | Specified | TEST-01, TEST-02, TEST-03 | Focused proof language now covers the right restart/live/report/shell cases. |
| Dependencies / integration points | Specified | INT-03, INT-04, INT-05 | Previously implicit owner chains are now explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The review assumes current checked-in execution/recovery/shell code is the authoritative implementation baseline for this slice.
- ASSUMP-02: The review assumes no hidden transition cursor or shell projection rewrite already exists outside the inspected code paths.
- QUESTION-01: No blocking open question remains for proposal-readiness.
- BLOCKER-01: None.

## O. Research Triggers / External Questions
No external research triggers were needed for this local readiness pass.
