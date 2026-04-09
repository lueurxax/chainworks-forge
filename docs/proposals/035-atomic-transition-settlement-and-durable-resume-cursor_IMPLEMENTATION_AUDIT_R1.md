# Proposal 035: Atomic Transition Settlement and Durable Resume Cursor Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/035-atomic-transition-settlement-and-durable-resume-cursor.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `8d79a35` |
| Working Tree | clean |
| Audited At | `2026-04-09T11:14:42+03:00` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 035 is partially implemented, not complete. The repository now has a real run-owned `TransitionCursor`, cursor-first resume/recovery behavior, and live reconciliation protection for scheduled-but-not-started boundaries, but the implementation still fails the proposal’s harder closure conditions: transition settlement is not fail-closed, major shell/read-model surfaces still project heuristic `run.currentStageID` truth, and the proposal-required focused same-tree interrupted-transition proof for the `EA93E855` class is still missing. Proposal 030 was inspected only as adjacent runtime context; it did not change this verdict.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Atomic settlement still advances after ignored save failure | High |
| Architecture | At Risk | Durable cursor exists, but settlement is not enforced as a fail-closed commit boundary | High |
| Product | At Risk | Resume/recovery truth improved, but operator shell can still show stale single-stage truth | High |
| UI | At Risk | Runs Home, Idea detail, and blocked recovery still read heuristic stage labels | High |
| UX | At Risk | Interrupted transition is not explained consistently across shell surfaces | Medium |
| Readiness | Not Ready | No focused `proposal-035` gate or `EA93E855`-class same-tree proof | High |

## Proposal Contract

### Scope

- Eliminate heuristic reconstruction of interrupted stage boundaries with one run-owned durable transition cursor.
- Make resume, recovery, reports, and operator surfaces consume one persisted continuation truth.
- Preserve the distinction between last completed stage, next scheduled stage, and actually started next-stage work.

### Locked Decisions

- The cursor is run-owned and stored on `Run`, not as a separate SwiftData model.
- `run_state` artifacts remain evidence only, not canonical continuation truth.
- Resume, recovery, reports, and shell projections must prefer the durable cursor over heuristic stage ordering.
- Live `.sessionClosed` is transport evidence only and must not demote a completed transition boundary by itself.

### Primary User Flows

1. A state completes, the workflow advances, and the next continuation point is durably recorded.
2. After app restart or operator resume, the run continues from the same scheduled state without re-inferring intent from stale rows or `run_state`.
3. Recovery/report/shell surfaces describe interrupted transition truth honestly: last completed, next scheduled, and whether next-stage work actually started.

### UI Commitments

- Recovery and shell surfaces must migrate away from heuristic `run.currentStageID` authority.
- Detail surfaces should be able to distinguish last completed, scheduled next, and started-now truth when they differ.

### UX Commitments

- Scheduled-but-not-started continuation must remain resumable, not rewritten into misleading failure truth.
- Live reconciliation after `.sessionClosed` must not falsely demote a completed-stage boundary.
- Reports must not invent phantom downstream failure truth.

### Acceptance Criteria

The proposal requires:

1. one durable run-owned cursor per successful state transition,
2. same resume target across manual resume and relaunch,
3. no false demotion of downstream `ready` stages,
4. no live false demotion from `.sessionClosed` alone,
5. report/recovery/shell distinction between completed, scheduled, and started truth,
6. green interrupted-transition proof tests for the canonical non-UI lane,
7. a focused `EA93E855`-class same-tree proof scenario,
8. non-reproducibility of the recurring “falls again at the same place” failure shape on that proof.

### Test / Evidence Requirements

- Focused proof coverage for interrupted transition after completion but before next-stage start.
- Proof coverage for stale `run_state`, live `.sessionClosed`, restart before scheduled stage executes, report generation, shell projection, and the `EA93E855` class scenario.

### Explicit Exclusions

- No workflow YAML redesign.
- No broader ACP transport redesign.
- No historical bulk migration of pre-cursor runs.

## Proposal Fidelity / Divergence

### Matches

- `TransitionCursor` exists as a run-owned persisted JSON contract on `Run`.
- Resume and recovery target the cursor first when it exists.
- Startup normalization preserves a scheduled-but-not-started stage as `.ready`.
- Live stalled-run reconciliation does not demote a completed transition boundary solely from `.sessionClosed`.
- Workflow map projection has a cursor-first current-stage derivation path.

### Divergences

- Atomic settlement is not fail-closed: `WorkflowOrchestrator` ignores `modelContext.save()` failure and still advances the state machine.
- Shell summary surfaces still read `run.currentStageID`, which is still stage-row heuristic truth.
- Report artifacts are still stamped with `run.currentStageID`, not explicit cursor-derived interrupted-transition truth.
- The proposal-required focused `EA93E855` proof scenario and dedicated `proposal-035` gate are absent.

### Ambiguities / Evidence Gaps

- I did not find runtime-validated shell screenshots for the interrupted-transition boundary.
- I did not find a focused report-generation proof that asserts artifact-stage metadata does not drift from cursor truth.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 3 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Run-owned durable continuation cursor exists

- Proposal Source: §5.1, §6.1, §8.1
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/TransitionCursor.swift:6-170`
  - `Chainworks Forge/Models/Run.swift:231-260`
- Gap / Note: The cursor contract is present, persisted on `Run.transitionCursorJSON`, and read as primary continuation truth for resume paths.

### REQ-002 Transition settlement is atomic and fail-closed before advancement

- Proposal Source: §5.2, §6.2, §6.10, §8.1
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:270-281`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2789-2804`
- Gap / Note: `settleTransition(...)` exists, but `modelContext.save()` is wrapped in `try?`, and callers still set `currentStateID` after settlement is attempted. This does not satisfy the proposal’s fail-closed atomicity contract.

### REQ-003 Scheduled-but-not-started continuation truth is preserved across startup normalization and live reconciliation

- Proposal Source: §5.3, §5.5, §6.3, §6.5, §8.3, §8.4
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:34-88`
  - `Chainworks Forge/Engine/ExecutionService.swift:689-702`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1316-1419`
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`
- Gap / Note: The live reconciliation guard and startup normalization behavior are both present and covered by same-tree tests.

### REQ-004 Resume and blocked-run recovery are cursor-first, with heuristic fallback only for pre-cursor runs

- Proposal Source: §5.4, §6.4, §6.8, §8.2
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:233-289`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:522-558`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:242-330`
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`
- Gap / Note: Recovery and resume now target the cursor first and only fall back when no cursor exists.

### REQ-005 Reports and recovery readers describe interrupted-transition truth without inventing phantom downstream state

- Proposal Source: §5.6, §6.4, §8.5
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:22-55`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:300-309`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:522-558`
- Gap / Note: The report payload now carries cursor fields, and recovery is cursor-first, but immutable/latest report artifacts are still stamped with `stageID: run.currentStageID ?? "unknown"`. That leaves report metadata attached to heuristic stage truth even when the payload knows the interrupted-transition boundary.

### REQ-006 Operator shell read models are rebound away from heuristic `run.currentStageID` authority

- Proposal Source: §6.6, §8.5
- Status: Partially Implemented
- Evidence Type: code, tests-run, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:41-42`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:120-145`
  - `Chainworks Forge/Views/RunsHomeView.swift:959-972`
  - `Chainworks Forge/Views/IdeaListView.swift:2644-2645`
  - `Chainworks Forge/Views/IdeaListView.swift:2745-2764`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift:588-590`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift:54-80`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:281-347`
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`
- Gap / Note: `WorkflowMapProjectionService` is cursor-first, but the core shell surfaces still display `run.currentStageID`, and the test suite still preserves that heuristic compatibility view as first-class truth. The read-model migration is incomplete.

### REQ-007 Canonical interrupted-transition proof exists on the non-UI lane

- Proposal Source: §7.10, §8.6
- Status: Partially Implemented
- Evidence Type: tests-run, tests-found
- Evidence:
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1316-1419`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:242-330`
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`
- Gap / Note: There is meaningful same-tree non-UI coverage for live reconciliation and recovery targeting, but I did not find a proposal-complete proof set covering report generation and shell projection for the interrupted-transition boundary.

### REQ-008 Focused `EA93E855`-class interrupted-transition proof scenario is green on the same tree

- Proposal Source: §7.10, §8.7, §8.8
- Status: Missing
- Evidence Type: tests-found, inference
- Evidence:
  - `docs/proposals/035-atomic-transition-settlement-and-durable-resume-cursor.md:451-468`
  - `scripts/test-gate.sh:1136-1144`
  - `scripts/test-gate.sh:1362-1371`
  - `rg -n "EA93E855-3BEA-4D86-B287-205A7A32AA1C|EA93E855" /Users/user/Documents/Chainworks Forge`
- Gap / Note: I found proposal text and adjacent test fixtures involving the state-9/state-10 loop, but no dedicated `proposal-035` gate, no named same-tree `EA93E855` proof artifact, and no proof test that explicitly closes the proposal’s acceptance criterion.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Settlement commit is not fail-closed

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-002`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:274-281`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2794-2804`
- Why It Matters: Proposal 035 is explicitly about eliminating heuristic boundary reconstruction with one durable atomic checkpoint. If the orchestrator can ignore save failure and still advance `currentStateID`, the system can still observe the very half-settled state the proposal was meant to remove.
- Recommended Action: Make settlement save explicit and fail-closed. If the save fails, do not advance the state machine; surface terminal recovery truth instead of continuing on an uncommitted transition.

### ARCH-002 Compatibility-stage heuristics still leak into canonical readers

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Models/Run.swift:104-115`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:281-347`
- Why It Matters: The proposal required the shell and operator read-models to stop treating `run.currentStageID` as authority. The codebase still preserves and tests the old heuristic compatibility view, so cursor truth has not fully displaced row-order truth.
- Recommended Action: Demote `run.currentStageID` to an explicitly labeled compatibility helper, and move shell/report readers onto cursor-derived summary fields or a dedicated read model.

## Product Review

**Summary:** At Risk

### PROD-001 Operator shell still presents a stale single-stage story

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:959-972`
  - `Chainworks Forge/Views/IdeaListView.swift:2644-2645`
  - `Chainworks Forge/Views/IdeaListView.swift:2745-2764`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift:588-590`
- Why It Matters: Product-wise, Proposal 035 only pays off if operators stop being told one misleading “current stage” when the real truth is “stage N completed, stage N+1 scheduled, stage N+1 maybe not started”. The recovery engine can be right while the shell is still telling the wrong story.
- Recommended Action: Introduce one operator-facing summary contract for interrupted transition truth and migrate list/detail/recovery surfaces to it.

### PROD-002 Report metadata still anchors immutable artifacts to heuristic stage truth

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-005`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:28-35`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:46-53`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:306-309`
- Why It Matters: The payload can now explain interrupted-transition truth correctly, but immutable artifact metadata still uses `run.currentStageID`. That creates a split between what the report says and how the report is cataloged and surfaced elsewhere.
- Recommended Action: Stamp report artifacts from cursor-derived summary truth, or persist explicit last-completed / next-scheduled identifiers on the report artifact metadata.

## UI Review

**Summary:** At Risk

### UI-001 Key macOS run surfaces have not been rebound to the new cursor model

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:959-972`
  - `Chainworks Forge/Views/IdeaListView.swift:2743-2764`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift:588-590`
- Why It Matters: These are the primary operator-facing surfaces on macOS. Leaving them on heuristic stage labels means the UI still flattens interrupted transition truth back into a one-label approximation.
- Recommended Action: Add cursor-aware labels such as “Last completed”, “Scheduled next”, and “Started now” where needed, or at minimum source the single primary label from one cursor-aware shell mapping rule.

## UX Review

**Summary:** At Risk

### UX-001 Interrupted-transition explanation is still inconsistent across shell and recovery surfaces

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:522-558`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift:588-590`
  - `Chainworks Forge/Views/RunsHomeView.swift:959-972`
- Why It Matters: Recovery logic now has enough truth to guide the operator correctly, but the surrounding shell still collapses that state into a generic “Current Stage”. That weakens trust and makes interrupted transitions harder to understand and act on.
- Recommended Action: Align the blocked recovery and shell summary copy with the same cursor-derived narrative, not a generic stage label.

## Readiness Review

**Summary:** Weak

### READY-001 Proposal 035 does not have a focused same-tree proof gate

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-007`, `REQ-008`
- Evidence Type: code, tests-found
- Evidence:
  - `scripts/test-gate.sh:1136-1144`
  - `scripts/test-gate.sh:1362-1371`
- Why It Matters: The proposal explicitly requires a focused interrupted-transition proof set and a same-tree `EA93E855`-class scenario. The canonical gate script has proposal gates through `proposal-030`, but no `proposal-035` lane at all.
- Recommended Action: Add a dedicated `proposal-035` gate with the non-UI interrupted-transition lane plus the focused `EA93E855` scenario.

### READY-002 Same-tree proof is real but still narrower than the proposal contract

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-007`
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`
  - Result bundle: `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.04.09_11-12-15-+0300.xcresult`
- Why It Matters: The existing tests prove meaningful progress, especially for resume/recovery/live reconciliation, but they do not close the proposal’s full report + shell + focused-scenario proof requirement.
- Recommended Action: Keep these suites in the proof lane, then add explicit tests for report truth and cursor-aware shell projection.

## Verification Log

- `rg -n "TransitionCursor|transitionCursor|resumeContinuationStateID|currentStageID|deriveCurrentStageID|EA93E855" /Users/user/Documents/Chainworks Forge`
- `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/RecoveryCoordinatorTests' -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` -> `44 tests`, `TEST SUCCEEDED`

## Final Roll-Up

- `Overall Conformance: Partial`
- `Overall Readiness: Not Ready`
- `Audit Confidence: High`

Proposal 035 has real implementation progress and meaningful same-tree proof for the cursor-first non-UI lane, but it is not complete. The remaining blockers are concrete implementation gaps, not audit noise: settlement still is not fail-closed, shell/read-model migration is incomplete, and the required focused same-tree proof scenario is still missing.
