# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `12036b7` |
| Working Tree | `dirty (92 modified, 22 untracked)` |
| Audited At | `2026-03-30T10:00:40+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Implemented` |
| Overall Readiness | `Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P016` is now implemented on the current tree.

The only live blocker in `R3` was proof-level, not behavior-level: the expanded same-head non-UI proposal slice failed before test execution because the `Chainworks ForgeTests` target did not compile. That blocker is now closed. On the same dirty tree audited here:

- local macOS build passed:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-build-bundle.Zf0E0i/p016-r4-build.xcresult`
- the expanded non-UI `P016` proof lane compiled and passed:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `116` tests passed across `10` suites
- fresh app-level proof passed:
  - `/tmp/p016-r4-app-proof.json`

The proposal text itself did not change since `R3`; the delta is that the current-head implementation and proof lane finally line up with the proposal-owned verification contract. No in-scope `REQ-*` item remains partial, missing, or non-verifiable on this tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | no live proposal-owned gap remains on the audited tree | High |
| Architecture | Acceptable | dirty-tree reproducibility remains snapshot-specific, but ownership boundaries are explicit | High |
| Product | Ready | app proof shows truthful stop, repair, exhaustion, and downgrade behavior | High |
| UI | Acceptable | report and recovery surfaces now expose binding truth clearly enough for the proposal contract | Medium |
| UX | Acceptable | recovery defaults and blocked-run explanation now match proposal-owned operator truth | Medium |
| Readiness | Ready | same-head build, non-UI proof, replay proof, and app proof are all green | High |

## Proposal Contract

### Scope

- canonicalize agent terminal outcome truth, including timeout, cancellation, and limit exhaustion
- persist explicit outcome/storage columns on `AgentExecution`
- settle stages exactly once per lineage
- make aggregate settlement first-class and subordinate to the aggregate stage
- repair stale active stages/approvals before resume creates new work
- migrate runtime binding truth over existing frozen provenance
- align report/recovery readers to canonical settlement truth

### Locked Decisions

- flattened persisted outcome columns are canonical; raw receipts/envelopes are supporting evidence only
- aggregate settlement is subordinate to the aggregate stage `StageExecution`
- create-path prevention is primary; startup repair is secondary
- runtime truth must be shown separately from frozen binding context when evidence is weak or contradictory
- proof obligations in Sections `8.1`, `8.2`, and `8.3` are proposal-owned, not optional side evidence

### Primary User Flows

1. A single agent attempt settles exactly once with truthful persisted terminal outcome even when output survives cancellation, timeout, or limit exhaustion.
2. Relaunch/resume repairs stale active stage and approval siblings before any new work begins.
3. Aggregate review failure remains stage-owned and explainable through subordinate aggregate settlement evidence.
4. Operators can see truthful runtime-vs-frozen binding context and the narrowest valid recovery path.

### UI Commitments

- report and recovery surfaces show runtime truth separately from frozen binding context
- operator surfaces present truthful blocked reason and narrowest valid next action

### UX Commitments

- limit exhaustion and policy-bound stops must not advertise automatic same-run retry by default
- legacy or weak runtime evidence must downgrade cleanly to `unverifiable`
- startup repair must leave one canonical active owner per lineage

### Acceptance Criteria

- acceptance criteria `1` through `14` in Section `9`

### Test / Evidence Requirements

- Section `8.1` unit and integration proof
- Section `8.2` motivating-run replay proof
- Section `8.3` app-level proof

### Explicit Exclusions

- Proposal 016 does not move cancellation ownership away from `RunCancellationCoordinator`
- Proposal 016 does not introduce a second provider-truth stack beyond current frozen/runtime evidence

## Proposal Fidelity / Divergence

### Matches

- `ActiveExecutionUniquenessGuard` exists as a concrete runtime owner for create-path uniqueness.
- standard report and recovery surfaces render explicit frozen-vs-runtime binding summaries through `RuntimeBindingTruthSummaryBuilder`.
- same-head app proof passes and demonstrates truthful repair, exhaustion handling, policy-stop handling, and `unverifiable` runtime truth.
- the expanded same-head non-UI `P016` slice now compiles and passes, including proposal-owned guard, replay, backfill, recovery, cancellation, resume, and orchestrator proof owners.
- the motivating regression now executes as a real current-head replay proof instead of remaining only a found test owner.

### Divergences

- none proposal-owned on the audited tree

### Ambiguities / Evidence Gaps

- none material for in-scope proposal conformance

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 13 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical terminal outcomes are explicitly persisted on `AgentExecution`, including truthful cancellation and limit exhaustion
- Proposal Source: `4.2`, `4.3`, acceptance criteria `1`, `2`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Current tree persists truthful cancellation and limit-exhaustion outcomes, and the fresh app proof passed on the same tree.

### REQ-002 Neutral finish markers, post-output transport errors, validation failure after output, and limit exhaustion settle without ambiguous success/error truth
- Proposal Source: `4.2`, `4.3.1`, `4.3.2`, acceptance criteria `3`, `4`, `5`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
- Gap / Note: The current same-head slice executed the neutral-finish, transport-error, validation-failure, and limit-exhaustion proof owners successfully.

### REQ-003 Legacy migration/backfill is deterministic and fail-closed, including `legacy_unverifiable` behavior and deterministic lineage derivation
- Proposal Source: `4.4`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Fresh current-head proof still shows `legacy-action=needsDecision` and `legacy-runtime-trust=unverifiable`, matching the proposal’s fail-closed migration contract.

### REQ-004 One logical stage lineage cannot have more than one active execution; startup repair must reconcile stale siblings before new work begins
- Proposal Source: `5.2`, `7.3`, `7.4`, acceptance criteria `7`, `9`, `10`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks ForgeTests/ActiveExecutionUniquenessGuardTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: The explicit guard owner is live, the same-head suite passed, and the fresh app proof still reports a single repaired active-stage sibling.

### REQ-005 One logical approval lineage cannot have more than one active approval record at a time
- Proposal Source: `7.2`, `7.2.1`, acceptance criteria `8`, `10`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Approval create-paths use the same uniqueness owner, and the fresh app proof still reports one requested approval plus one repaired expired approval.

### REQ-006 Aggregate steps use a first-class persisted settlement record subordinate to the aggregate stage’s `StageExecution`
- Proposal Source: `5.3`, `5.4`, acceptance criterion `11`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/AggregateSettlementRecord.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
- Gap / Note: The current same-head orchestrator slice passed aggregate-settlement and subordinate-record coverage.

### REQ-007 Reports and operator surfaces show frozen binding truth separately from actual runtime truth and downgrade to `unverifiable` when runtime evidence is weak
- Proposal Source: `6.2`, `6.3`, acceptance criteria `12`, `13`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks ForgeTests/RuntimeBindingTruthSummaryTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Standard report and recovery surfaces now render explicit `frozen=... runtime=...` summaries, and the fresh app proof passes with truthful `unverifiable` downgrade messaging.

### REQ-008 Reports and recovery derive failed-step identity, retry path, resume path, and aggregate evidence from canonical settlement/recovery records
- Proposal Source: `3` Layer V, `5.4`, acceptance criterion `12`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Same-head recovery and report proof now execute cleanly and continue to show truthful blocked reason plus the narrowest valid next action.

### REQ-009 Limit exhaustion and provider policy-bound terminal stops default to non-auto-retryable unless a narrower explicit override is persisted
- Proposal Source: `7.4`, `7.5`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Fresh same-head proof still reports `suggested=none` with clone-only allowed actions for both limit exhaustion and policy-bound stops.

### REQ-010 Section 8.1 unit and integration proof is complete
- Proposal Source: `8.1`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks ForgeTests/ActiveExecutionUniquenessGuardTests.swift`
  - `Chainworks ForgeTests/RuntimeBindingTruthSummaryTests.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `Chainworks ForgeTests/RunCancellationCoordinatorTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
- Gap / Note: The current tree now executes the full proposal-owned unit/integration lane successfully. The `R3` compile blocker is closed.

### REQ-011 Section 8.2 motivating-run replay proof exists and proves the full failure class
- Proposal Source: `8.2`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
- Gap / Note: The same-head replay/backfill owners now execute instead of aborting at compile time, and the canonical motivating regression passes as part of the current proposal slice.

### REQ-012 Section 8.3 app-level proof exists and is executable on the current tree
- Proposal Source: `8.3`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `/tmp/p016-r4-app-proof.json`
- Gap / Note: Fresh same-head app proof passed on the current tree.

### REQ-013 Ownership boundaries are explicit across `WorkflowOrchestrator`, `ResumeManager`, `RecoveryCoordinator`, and approval persistence
- Proposal Source: `7.1`, acceptance criterion `14`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult`
- Gap / Note: Owner boundaries are explicit in code and now validated by the green same-head proposal lane.

## Architecture Review

**Summary:** Acceptable

No live architecture blocker remains on the current tree. The explicit guard owner, subordinate aggregate settlement model, and flattened canonical outcome fields now line up with the proposal’s ownership model and same-head proof.

## Product Review

**Summary:** Ready

The fresh app proof demonstrates the product behavior the proposal owns: truthful stop-path settlement, startup repair, honest runtime-truth downgrade, and clone-only recovery defaults for exhaustion/policy-bound stops.

## UI Review

**Summary:** Acceptable

`RunReportView`, `BlockedRunRecoveryView`, and `RecoverySheet` now expose runtime-versus-frozen binding truth directly enough for the operator-facing contract in Section `6`.

## UX Review

**Summary:** Acceptable

The recovery story now matches the proposal’s intent: blocked reason, actual failed step, and allowed next actions are derived from canonical recovery truth instead of loose historical inference.

## Delivery / Readiness Review

**Summary:** Ready

The proposal-owned proof lane is now complete on the same audited tree: build passed, the expanded non-UI slice passed `116` tests, the motivating regression executed, and the app proof passed.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-build-bundle.Zf0E0i/p016-r4-build.xcresult` |
| Core user flow runtime-validated | Pass | `/tmp/p016-r4-app-proof.json` |
| Empty/loading/error states covered | Pass | recovery/report surfaces covered by current same-head proof plus app harness |
| Accessibility risk acceptable | Not Checked | not proposal-critical in this pass |
| Localization risk acceptable | Not Checked | not proposal-critical in this pass |
| Critical tests executed | Pass | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r4-test.OO77Ku/p016-r4-tests.xcresult` |
| Privacy/permissions/entitlements reviewed | Not Checked | outside proposal-critical scope for this pass |

## Verification Log

- `rg -n "ActiveExecutionUniquenessGuard|frozen=.*runtime|proposal-016|RuntimeBindingTruthSummaryTests" ...`
- `xcodebuild build -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$(mktemp -d ...)" -resultBundlePath "$(mktemp -d ...)/p016-r4-build.xcresult"`
- `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$(mktemp -d ...)" -resultBundlePath "$DERIVED_DATA/p016-r4-tests.xcresult" -only-testing:... -skip-testing:'Chainworks ForgeUITests'`
- `CHAINWORKS_P016_PROOF_AUTORUN=1 CHAINWORKS_IN_MEMORY_STORE=1 CHAINWORKS_P016_RESULT_PATH=/tmp/p016-r4-app-proof.json <fresh app binary>`

## Recommended Next Actions

1. No proposal-owned implementation blocker remains on the audited tree.
2. If the team wants a cleaner handoff artifact later, rerun the same proof on a less-dirty tree snapshot; that is a reproducibility convenience, not a Proposal 016 conformance gap.
