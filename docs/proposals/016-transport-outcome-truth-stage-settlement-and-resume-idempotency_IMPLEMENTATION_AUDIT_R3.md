# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `12036b7` |
| Working Tree | `dirty (84 modified, 21 untracked)` |
| Audited At | `2026-03-30T09:11:17+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P016` moved forward again after `R2`. The two biggest proposal-owned partials from `R2` are now materially addressed on the current tree:

- create-path uniqueness is no longer only implicit helper behavior; there is now a real `ActiveExecutionUniquenessGuard` wired into live stage/approval creation paths;
- frozen-vs-runtime binding truth is no longer confined to the proof harness; the standard report and recovery surfaces now render `RuntimeBindingTruthSummaryBuilder` output directly.

Fresh same-head evidence in this pass:

- local macOS build passed:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r3-build-bundle.URIbCZ/p016-r3-build.xcresult`
- fresh app-level proof passed:
  - `/tmp/p016-r3-app-proof.json`

Why the verdict still does not reach `Implemented`: the current non-UI proof lane is red before tests execute. The expanded same-head `P016` test slice now includes the right owners, but the `Chainworks ForgeTests` target fails to compile because current actor-isolation changes broke test-support files such as `RuntimeBindingTruthSummaryTests.swift`, `SharedMocks.swift`, and `SimulatedAgentExecutorTests.swift`. That leaves Section `8.1` incomplete on the current tree and prevents same-head replay proof from closing cleanly.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | proposal-owned proof lane is still red on the current tree | High |
| Architecture | Acceptable | prior create-path uniqueness gap is materially closed by the new guard owner | High |
| Product | Acceptable | app-level proof still demonstrates truthful cancellation, repair, exhaustion, and policy-stop handling | High |
| UI | Acceptable | standard report/recovery views now surface frozen-vs-runtime binding summaries | Medium |
| UX | Acceptable | operator-facing truth explanation is materially clearer than in `R2` | Medium |
| Readiness | Not Ready | same-head `Chainworks ForgeTests` target compile failure blocks canonical non-UI proof | High |

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

- `ActiveExecutionUniquenessGuard` now exists as a real runtime owner for create-path uniqueness.
- standard report and recovery surfaces now render explicit frozen-vs-runtime binding summary text through `RuntimeBindingTruthSummaryBuilder`.
- same-head app-level proof still passes and demonstrates truthful repair, exhaustion handling, policy-stop handling, and `unverifiable` runtime truth.
- the canonical `proposal-016` gate inventory in `scripts/test-gate.sh` now includes the previously missing guard/runtime-summary/replay proof owners.

### Divergences

- current same-head non-UI proposal slice is red because the `Chainworks ForgeTests` target fails to compile before the expanded proof owners execute.

### Ambiguities / Evidence Gaps

- replay and legacy-backfill proof owners now exist in the canonical lane, but current-head execution cannot verify them because compile failures abort the test target first.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical terminal outcomes are explicitly persisted on `AgentExecution`, including truthful cancellation and limit exhaustion
- Proposal Source: `4.2`, `4.3`, acceptance criteria `1`, `2`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Current tree still persists truthful cancellation and limit-exhaustion outcomes, and the fresh app proof passed on the same tree.

### REQ-002 Neutral finish markers, post-output transport errors, validation failure after output, and limit exhaustion settle without ambiguous success/error truth
- Proposal Source: `4.2`, `4.3.1`, `4.3.2`, acceptance criteria `3`, `4`, `5`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
- Gap / Note: Current code still uses explicit canonical classification and deterministic coarse-status mapping. Test owners for neutral finish, validation failure, completed-with-transport-error, and limit exhaustion remain present on the current tree.

### REQ-003 Legacy migration/backfill is deterministic and fail-closed, including `legacy_unverifiable` behavior and deterministic lineage derivation
- Proposal Source: `4.4`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Same-head app proof still shows `legacy-action=needsDecision` and `legacy-runtime-trust=unverifiable`, and live backfill/lineage derivation logic remains present in `ResumeManager`.

### REQ-004 One logical stage lineage cannot have more than one active execution; startup repair must reconcile stale siblings before new work begins
- Proposal Source: `5.2`, `7.3`, `7.4`, acceptance criteria `7`, `9`, `10`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks ForgeTests/ActiveExecutionUniquenessGuardTests.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: The old `R2` partial is materially closed. There is now a real guard owner, it is wired into orchestrator/retry create paths, and the fresh app proof still reports `active-stage-siblings=1` after repair.

### REQ-005 One logical approval lineage cannot have more than one active approval record at a time
- Proposal Source: `7.2`, `7.2.1`, acceptance criteria `8`, `10`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Approval create-paths now route through the same uniqueness owner, and the fresh app proof still reports `requested-approvals=1 expired-approvals=1`.

### REQ-006 Aggregate steps use a first-class persisted settlement record subordinate to the aggregate stage’s `StageExecution`
- Proposal Source: `5.3`, `5.4`, acceptance criterion `11`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Models/AggregateSettlementRecord.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
- Gap / Note: The subordinate aggregate owner model remains explicit and unchanged on the current tree.

### REQ-007 Reports and operator surfaces show frozen binding truth separately from actual runtime truth and downgrade to `unverifiable` when runtime evidence is weak
- Proposal Source: `6.2`, `6.3`, acceptance criteria `12`, `13`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks ForgeTests/RuntimeBindingTruthSummaryTests.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: The old `R2` partial is materially closed. Standard report and recovery surfaces now render explicit `frozen=... runtime=... [trust]` summaries, and the fresh app proof still passes with step `"[5/5] Report/recovery surfaces label unverifiable binding truth honestly..."`.

### REQ-008 Reports and recovery derive failed-step identity, retry path, resume path, and aggregate evidence from canonical settlement/recovery records
- Proposal Source: `3` Layer V, `5.4`, acceptance criterion `12`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Current app proof still demonstrates truthful blocked reason plus clone-only/manual recovery, and canonical readers remain stage/recovery-snapshot first.

### REQ-009 Limit exhaustion and provider policy-bound terminal stops default to non-auto-retryable unless a narrower explicit override is persisted
- Proposal Source: `7.4`, `7.5`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Same-head app proof still reports `suggested=none` with clone-only allowed actions for both limit exhaustion and policy-bound stops.

### REQ-010 Section 8.1 unit and integration proof is complete
- Proposal Source: `8.1`
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
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
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r3-test.CqBuEG/p016-r3-tests.xcresult`
- Gap / Note: This requirement is closer than in `R2`: the canonical lane now includes the right proof owners. It is still not complete on the current tree because `xcodebuild test` fails before execution with actor-isolation compile errors in `RuntimeBindingTruthSummaryTests.swift`, `SharedMocks.swift`, and `SimulatedAgentExecutorTests.swift`.

### REQ-011 Section 8.2 motivating-run replay proof exists and proves the full failure class
- Proposal Source: `8.2`
- Status: Partially Implemented
- Evidence Type: tests-found
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r3-test.CqBuEG/p016-r3-tests.xcresult`
- Gap / Note: The replay/backfill proof owners are now present in the canonical `proposal-016` lane, but the same test-target compile failure aborts execution before the current-head replay suite can run.

### REQ-012 Section 8.3 app-level proof exists and is executable on the current tree
- Proposal Source: `8.3`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `/tmp/p016-r3-app-proof.json`
- Gap / Note: Fresh same-head app proof passed on the current tree.

### REQ-013 Ownership boundaries are explicit across `WorkflowOrchestrator`, `ResumeManager`, `RecoveryCoordinator`, and approval persistence
- Proposal Source: `7.1`, acceptance criterion `14`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/ActiveExecutionUniquenessGuard.swift`
- Gap / Note: Owner boundaries are now sharper than in `R2`, because create-path uniqueness has its own concrete owner instead of living only as helper convention.

## Architecture Review

**Summary:** Acceptable

No new architecture blocker remains after the introduction of `ActiveExecutionUniquenessGuard`. The current architecture delta is positive: the explicit guard closes the main `R2` ownership weakness.

## Product Review

**Summary:** Acceptable

The fresh app proof still demonstrates the intended product behavior: truthful blocked reason, truthful runtime trust downgrade, truthful startup repair, and no auto-retry for exhaustion/policy-bound stops.

## UI Review

**Summary:** Acceptable

The important UI delta from `R2` is real: `RunReportView`, `BlockedRunRecoveryView`, and `RecoverySheet` now show the frozen-vs-runtime binding summary directly instead of only a generic trust badge.

## UX Review

**Summary:** Acceptable

The explanatory trust model is materially better than in `R2`. Operators now get the concrete `frozen=... runtime=...` mismatch line in standard report and recovery surfaces.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Same-head `P016` proof is blocked by test-target compile regression
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-010`, `REQ-011`
- Evidence Type: tests-run
- Evidence:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r3-test.CqBuEG/p016-r3-tests.xcresult`
  - representative failures from the current run:
    - `Main actor-isolated property 'executedTasks' cannot be accessed from outside of the actor`
    - `Call to main actor-isolated static method 'generate(...)' in a synchronous nonisolated context`
    - `Main actor-isolated initializer 'init(simulatedDelay:catalog:)' cannot be called from outside of the actor`
- Why It Matters: Proposal 016 now has the right proof inventory, but current-head delivery confidence is blocked because the test target does not compile. That prevents the canonical non-UI lane from proving the expanded requirement set.
- Recommended Action: Fix the actor-isolation compile regressions in `Chainworks ForgeTests/RuntimeBindingTruthSummaryTests.swift`, `Chainworks ForgeTests/SharedMocks.swift`, and `Chainworks ForgeTests/SimulatedAgentExecutorTests.swift`, then rerun the expanded `proposal-016` slice.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r3-build-bundle.URIbCZ/p016-r3-build.xcresult` |
| Core user flow runtime-validated | Pass | `/tmp/p016-r3-app-proof.json` |
| Empty/loading/error states covered | Partial | app proof covers blocked/report/recovery narrative, but red test target prevents fuller same-head non-UI proof |
| Accessibility risk acceptable | Not Checked | not proposal-critical in this pass |
| Localization risk acceptable | Not Checked | not proposal-critical in this pass |
| Critical tests executed | Partial | app proof executed; non-UI test slice failed at compile time |
| Privacy/permissions/entitlements reviewed | Not Checked | outside proposal-critical scope for this pass |

## Verification Log

- `rg -n "ActiveExecutionUniquenessGuard|frozen=.*runtime|proposal-016|RuntimeBindingTruthSummaryTests" ...`
- `xcodebuild build -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$(mktemp -d ...)" -resultBundlePath "$(mktemp -d ...)/p016-r3-build.xcresult"`
- `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$(mktemp -d ...)" -resultBundlePath "$DERIVED_DATA/p016-r3-tests.xcresult" -only-testing:...`
- `CHAINWORKS_P016_PROOF_AUTORUN=1 CHAINWORKS_IN_MEMORY_STORE=1 CHAINWORKS_P016_RESULT_PATH=/tmp/p016-r3-app-proof.json <fresh app binary>`

## Recommended Next Actions

1. Fix the actor-isolation compile regressions in the `Chainworks ForgeTests` target, especially `RuntimeBindingTruthSummaryTests.swift`, `SharedMocks.swift`, and `SimulatedAgentExecutorTests.swift`.
2. Rerun the expanded non-UI `proposal-016` slice and confirm the replay/backfill suites execute instead of failing at compile time.
3. If that bundle goes green, rerun the implementation audit once more; the old `R2` behavior gaps are already materially closed on the current tree.
