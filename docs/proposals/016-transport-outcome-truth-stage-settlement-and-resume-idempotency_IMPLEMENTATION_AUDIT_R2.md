# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `12036b7` |
| Working Tree | `dirty (59 modified, 17 untracked)` |
| Audited At | `2026-03-30T01:07:22+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `At Risk` |
| Audit Confidence | `High` |

## Executive Verdict

`P016` is materially stronger than `R1` and is no longer blocked by missing core runtime seams. The biggest proposal-owned defects from `R1` are now directly closed on the current tree:

- truthful cancellation now persists `cancelled_before_output` / `cancelled_after_output` instead of stopping at coarse `.cancelled`;
- deterministic legacy backfill and fail-closed `unverifiable` handling are live in `ResumeManager`;
- limit-exhaustion and policy-bound stops now default to operator inspection / clone-only recovery instead of ordinary same-run retry;
- Section `8.3` app-level proof is real and passed on the current tree.

Fresh evidence for this pass:

- local macOS build passed:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-build.jWE8ER/p016-r2-build.xcresult`
- focused current-head proposal slice passed `41/41`:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
- replay/backfill slice passed `10/10`:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- direct app-level proof passed:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`

`P016` still does not reach `Implemented` for three proposal-owned reasons:

1. create-path uniqueness is mostly enforced, but the proposal chose an explicit `ActiveExecutionUniquenessGuard` boundary and current prevention remains distributed across `WorkflowOrchestrator` helpers rather than a clearly isolated owner;
2. standard report/recovery surfaces still do not show frozen binding truth side-by-side with runtime truth as clearly as the proposal requires; that comparison is strongest today in the dedicated proof harness/direct surface, not in ordinary operator views;
3. Section `8.1` proof is strong but still split across multiple direct slices and supporting suites rather than one clean current-head proving lane that replays every listed proof owner in one place.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | proposal-owned gaps remain in create-path guard ownership, ordinary operator truth presentation, and proof-lane completeness | High |
| Architecture | At Risk | uniqueness prevention is real but distributed; future maintenance can drift because the guard is semantic, not isolated | High |
| Product | Acceptable | recovery policy now fails closed for exhaustion and policy-bound stops, and app-level behavior is materially truthful | High |
| UI | Acceptable | the dedicated proof surface demonstrates the new truth model, but ordinary report/recovery surfaces still compress the comparison | Medium |
| UX | At Risk | operators still get a trust badge more clearly than a frozen-vs-runtime comparison in the standard shell | Medium |
| Readiness | At Risk | direct proof is green, but the canonical `proposal-016` gate could not be replayed in this pass because `check_idle_environment` fail-closed on ambient processes | High |

## Proposal Contract

### Scope

- canonicalize agent terminal outcome truth, including timeout, cancellation, and limit exhaustion
- persist explicit outcome/storage columns on `AgentExecution`
- settle stages exactly once per lineage
- make aggregate settlement first-class and subordinate to the aggregate stage
- repair stale active stages/approvals before resume creates new work
- migrate runtime binding truth over existing frozen provenance
- align report/recovery readers to canonical settlement truth

### Primary User Flows

1. One agent attempt settles once with a canonical persisted terminal outcome even when output survives timeout, transport error, cancellation, or limit exhaustion.
2. Relaunch/resume repairs stale `running` / `waitingApproval` siblings before new work begins.
3. Aggregate review failure remains first-class aggregate settlement instead of fan-out artifact inference.
4. Recovery/report surfaces identify the actual failed step, the actual narrowest next action, and the actual runtime-truth confidence.

### Acceptance / Proof Commitments

- Section `8.1` focused unit/integration proof
- Section `8.2` motivating-run replay proof
- Section `8.3` app-level proof
- Acceptance criteria `1` through `14`

## Proposal Fidelity / Divergence

### Matches

- The canonical outcome taxonomy is live in persisted model code and includes truthful cancellation plus explicit limit exhaustion.
- `AgentExecution` now persists `canonicalOutcome`, `transportErrorKind`, `providerStopReason`, `outputPresence`, `settledAt`, `runtimeProvider`, `runtimeModel`, and `outcomeEnvelopeJSON`.
- `ResumeManager` performs deterministic legacy backfill and fail-closed runtime-truth downgrade to `unverifiable` when evidence is insufficient.
- `StageRetryCoordinator` now defaults limit-exhaustion and policy-bound stops to operator inspection instead of automatic same-run retry.
- `AggregateSettlementRecord` is a real persisted subordinate record keyed by `stageExecutionID` and `lineageID`.
- A real `proposal-016` proving lane now exists in `scripts/test-gate.sh`, and a dedicated app-level proof harness/direct surface exists on the current tree.

### Divergences

- create-path uniqueness is implemented as distributed orchestration helpers (`claimOrCreateStageExecution`, `existingOrRestoredApproval`, canonical stage/approval lookups), not as a clearly isolated `ActiveExecutionUniquenessGuard` owner
- ordinary report/recovery UI still exposes runtime provider/model summary plus trust badge more strongly than explicit frozen-vs-runtime comparison
- the current proving lane for Section `8.1` remains split between the canonical gate and additional direct targeted suites run outside that gate

### Ambiguities / Evidence Gaps

- the `proposal-016` gate itself is now real, but its replay in this pass fail-closed before execution because ambient `xcodebuild` / app processes tripped `check_idle_environment`
- executor/orchestrator proof categories for neutral finish, subordinate aggregate settlement, and completed-with-transport-error remain strongly supported by current code and test inventory, but were not rerun as their own dedicated current-pass bundle

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical terminal outcomes are explicitly persisted on `AgentExecution`, including truthful cancellation and limit exhaustion
- Proposal Source: `4.2`, `4.3`, acceptance criteria `1`, `2`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks ForgeTests/RunCancellationCoordinatorTests.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
- Gap / Note: `beginSettlement()` now persists canonical cancellation truth through `ExecutionTruthSupport.persistTerminalTruth(...)`, and focused tests passed for both `cancelled_after_output` and `cancelled_before_output`.

### REQ-002 Neutral finish markers, post-output transport errors, validation failure after output, and limit exhaustion settle without ambiguous success/error truth
- Proposal Source: `4.2`, `4.3.1`, `4.3.2`, acceptance criteria `3`, `4`, `5`
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
- Gap / Note: The live model/runtime path now fixes reader precedence and deterministic coarse-status mapping. Current tree also contains direct tests for neutral finish markers, limit exhaustion after output, validation-failure override, and completed-with-transport-error preservation.

### REQ-003 Legacy migration/backfill is deterministic and fail-closed, including `legacy_unverifiable` behavior and deterministic lineage derivation
- Proposal Source: `4.4`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- Gap / Note: `ResumeManager.backfillLegacyExecutionTruthIfNeeded(...)` now applies deterministic canonical backfill, derives lineage when stable, and downgrades to `unverifiable` / explicit decision when durable evidence is insufficient or conflicting.

### REQ-004 One logical stage lineage cannot have more than one active execution; startup repair must reconcile stale siblings before new work begins
- Proposal Source: `5.2`, `7.3`, `7.4`, acceptance criteria `7`, `9`, `10`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- Gap / Note: Startup repair is real and strong, and create-path helpers usually claim the canonical active stage instead of creating a sibling. The remaining gap is ownership sharpness: the proposal chose an explicit create-path guard boundary, while the current tree still enforces this through distributed helper logic rather than a clearly isolated `ActiveExecutionUniquenessGuard`.

### REQ-005 One logical approval lineage cannot have more than one active approval record at a time
- Proposal Source: `7.2`, `7.2.1`, acceptance criteria `8`, `10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
- Gap / Note: Approval lineage is persisted, duplicate requested siblings are expired deterministically, and waiting-approval restore now repairs duplicate approval siblings before restoring the gate.

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
- Gap / Note: The subordinate aggregate owner model is real in persistence and read paths. Current test inventory includes an explicit subordinate aggregate-settlement proof owner.

### REQ-007 Reports and operator surfaces show frozen binding truth separately from actual runtime truth and downgrade to `unverifiable` when runtime evidence is weak
- Proposal Source: `6.2`, `6.3`, acceptance criteria `12`, `13`
- Status: Partially Implemented
- Evidence Type: code, runtime, tests-run
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`
- Gap / Note: Runtime-truth downgrade is real and the proof surface explicitly shows `frozen=... runtime=...` plus `runtimeTrust=unverifiable`. The remaining gap is ordinary operator UX: `RunReportBuilder` still emits only the resolved provider/model pair plus trust level, and standard report/recovery views mostly surface the trust badge rather than a clear side-by-side frozen-vs-runtime comparison.

### REQ-008 Reports and recovery derive failed-step identity, retry path, resume path, and aggregate evidence from canonical settlement/recovery records
- Proposal Source: `3` Layer V, `5.4`, acceptance criterion `12`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- Gap / Note: Current tree derives recovery and report narratives from canonical stages, approvals, aggregate settlement, and persisted recovery snapshots rather than raw historical scans.

### REQ-009 Limit exhaustion and provider policy-bound terminal stops default to non-auto-retryable unless a narrower explicit override is persisted
- Proposal Source: `7.4`, `7.5`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`
- Gap / Note: Recovery policy now recommends operator inspection for limit exhaustion and policy-bound terminal stops, and the app-level proof verifies clone-only/manual recovery for those cases.

### REQ-010 Section 8.1 unit and integration proof is complete
- Proposal Source: `8.1`
- Status: Partially Implemented
- Evidence Type: tests-run, tests-found
- Evidence:
  - `Chainworks ForgeTests/Proposal016Tests.swift`
  - `Chainworks ForgeTests/RunCancellationCoordinatorTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- Gap / Note: The current tree now has the needed proof owners, and the two focused slices run green. The remaining gap is proof-lane completeness: not every executor/orchestrator proof owner from Section `8.1` was rerun in one current-pass bundle, and the canonical `proposal-016` gate replay itself was blocked by the idle-environment guard before execution.

### REQ-011 Section 8.2 motivating-run replay proof exists and proves the full failure class
- Proposal Source: `8.2`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/HistoricalRunReplayTests.swift`
  - `Chainworks ForgeTests/LegacyExecutionTruthBackfillTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
- Gap / Note: Current-head replay/backfill proof passed `10/10` and now covers canonical interrupted truth, duplicate-active-lineage repair, aggregate-missing retry recommendation, legacy-unverifiable behavior, and non-success treatment of partial-output exhaustion.

### REQ-012 Section 8.3 app-level proof exists and is executable on the current tree
- Proposal Source: `8.3`
- Status: Implemented
- Evidence Type: code, runtime, tests-found
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`
- Gap / Note: The app-level proof path is now real: the repo has a dedicated harness, a direct surface, a UI smoke owner, and a passing current-head app-launched proof payload.

### REQ-013 Ownership boundaries are explicit across `WorkflowOrchestrator`, `ResumeManager`, `RecoveryCoordinator`, and approval persistence
- Proposal Source: `7.1`, acceptance criterion `14`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Models/Approval.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
- Gap / Note: The owner split is now explicit and matches the proposal materially: orchestration owns live execution, resume owns startup repair/classification, recovery owns valid next actions, and approval persistence owns durable gate identity/decision history.

## Expert Findings

### ARCH-001 Distributed uniqueness prevention is still easier to drift than the proposal’s explicit guard boundary
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-004`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
- Why It Matters: The runtime now mostly prevents duplicate active siblings, but the protection is spread across several canonical-lookup helpers plus startup repair. That is materially better than `R1`, yet more fragile than the proposal’s explicit guard-owner model because future create-paths can bypass these conventions more easily.
- Recommended Action: Either land a clearly named guard owner or document/test every create-path boundary as proposal-owned invariants instead of relying on scattered helper behavior.

### UX-001 Standard operator surfaces still under-explain frozen-vs-runtime binding truth
- Severity: Major
- Confidence: Medium
- Related Proposal Items: `REQ-007`
- Evidence Type: code, runtime
- Evidence References:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`
- Why It Matters: The proof harness demonstrates the exact comparison the proposal wants, but ordinary shell/report surfaces still communicate a trust badge more clearly than the frozen-vs-runtime discrepancy itself. Operators can still miss why truth is `unverifiable`.
- Recommended Action: Promote the harness’ explicit `frozen=... runtime=... trust=...` narrative into standard report and blocked-run surfaces.

### READY-001 Canonical gate reproducibility is still environment-sensitive
- Severity: Minor
- Confidence: High
- Related Proposal Items: `REQ-010`, `REQ-012`
- Evidence Type: runtime, code
- Evidence References:
  - `scripts/test-gate.sh`
  - local `proposal-016` replay attempt in this pass fail-closed before execution because `check_idle_environment` detected ambient processes
- Why It Matters: Current-head direct proof is strong and green, but operator handoff is weaker when the canonical gate cannot always be replayed without manual environment cleanup.
- Recommended Action: Keep the strict guard, but document the exact cleanup path or add a safer diagnostic mode so proposal audits can reproduce the canonical lane more predictably.

## Evidence Inventory

### Tests Run

- build: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-build.jWE8ER/p016-r2-build.xcresult`
- focused proposal slice: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-test.ji0hYT/p016-r2-tests.xcresult`
  - result: `41/41` passed
- replay/backfill slice: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-extra-bundle.vUUeBv/p016-r2-extra.xcresult`
  - result: `10/10` passed
- app proof: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-r2-app-proof.4T5dOqJ5kP.json`
  - result: `PASS — Proposal 016 app-level proof verified`

### High-Signal Code Surfaces

- `Chainworks Forge/Models/ExecutionTruth.swift`
- `Chainworks Forge/Models/AggregateSettlementRecord.swift`
- `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
- `Chainworks Forge/Engine/ResumeManager.swift`
- `Chainworks Forge/Engine/StageRetryCoordinator.swift`
- `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- `Chainworks Forge/Engine/RecoveryCoordinator.swift`
- `Chainworks Forge/Engine/RunReportBuilder.swift`
- `Chainworks Forge/Engine/Proposal016ExecutionTruthHarness.swift`
- `Chainworks Forge/Views/UITestDirectSurfaces.swift`

## Conclusion

`R2` closes the substantive `R1` blockers and establishes that `P016` is live in the current runtime, recovery policy, replay/backfill path, and app-level proof harness. The remaining deltas are narrower and proposal-shaped rather than foundational.

Current status is therefore `Partial`, not `Not Implemented`:

- no core requirement is still `Missing`;
- the remaining work is concentrated in explicit create-path guard ownership, ordinary operator truth presentation, and proof-lane completeness.
