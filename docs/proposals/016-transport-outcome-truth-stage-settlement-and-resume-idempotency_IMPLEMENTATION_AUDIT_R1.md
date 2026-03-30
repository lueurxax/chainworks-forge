# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `12036b7` |
| Working Tree | `dirty (26 modified, 2 untracked)` |
| Audited At | `2026-03-29T22:21:58+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P016` is materially landed in the model layer and several live readers, but the proposal is not fully implemented on the current tree. The strongest evidence is mixed:

- local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-audit-build.O9JYwX/p016-build.xcresult`
- direct targeted `xcodebuild` proof attempts were non-proving and executed `0` tests:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-audit-test.JJVOLJ/p016-tests.xcresult`
  - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-18-01-+0300.xcresult`
- a broader local unit-target run did execute real Swift Testing suites and exposed current-head instability:
  - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-20-48-+0300.xcresult`
  - that run reached non-zero execution, then hit unrelated red tests and a live `SwiftData/BackingData.swift:844` fatal crash

The proposal-owned blockers are explicit, not cosmetic:

1. stop-path cancellation still settles only the coarse `.cancelled` status and does not write `cancelled_before_output` / `cancelled_after_output` into the new canonical outcome columns;
2. the fail-closed legacy backfill path described in `4.4` is not present as live migration logic;
3. recovery policy still recommends ordinary same-run retry for limit-exhaustion / provider-policy terminal stops instead of defaulting those classes to non-auto-retryable;
4. Section `8.3` app-level proof has no canonical execution path on the current tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | proposal-owned stop-path, backfill, recovery-policy, and app-proof gaps remain open | High |
| Architecture | At Risk | cancellation truth and create-path uniqueness are still split across coarse status, startup repair, and implicit canonical selection | High |
| Product | At Risk | operator recovery still over-advertises retry for exhaustion/policy-bound terminal stops | High |
| UI | Acceptable | existing recovery/report surfaces already consume more canonical truth and aggregate evidence than before | Medium |
| UX | At Risk | ordinary shell surfaces still do not clearly separate frozen binding context from actual runtime binding facts | Medium |
| Readiness | Not Ready | current-head proof path is incomplete: non-proving `0`-test slices, blocked canonical gate, red wider unit run, and no app-level harness path | High |

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

1. An agent run settles once with a canonical persisted outcome even when output survives timeout, transport error, or limit exhaustion.
2. Relaunch/resume repairs stale `running` / `waitingApproval` siblings before new work begins.
3. Aggregate review failure is surfaced as first-class aggregate settlement rather than fan-out artifact inference.
4. Recovery/report surfaces identify the actual failed step, actual runtime truth, and actual narrowest next action.

### Acceptance / Proof Commitments

- Section `8.1` focused unit/integration proof
- Section `8.2` motivating-run replay proof
- Section `8.3` app-level proof
- Acceptance criteria `1` through `14`

## Proposal Fidelity / Divergence

### Matches

- The canonical outcome taxonomy exists in the live model layer, including cancellation and limit-exhaustion enum cases.
- `AgentExecution` now carries explicit outcome/storage columns, and `WorkflowOrchestrator` writes them for ordinary execution paths.
- `StageExecution` / `Approval` lineage fields and `AggregateSettlementRecord` are real persisted models.
- `ResumeManager`, `RecoveryCoordinator`, and `RunReportBuilder` already consume lineage, aggregate settlement, and runtime-truth seams rather than relying only on raw historical scans.

### Divergences

- stop-path cancellation still bypasses the new canonical outcome fields and only writes coarse `AgentStatus.cancelled`
- `4.4` fail-closed legacy backfill is still proposal text, not live migration logic
- recovery policy for limit exhaustion / provider-policy terminal stops is not yet provider-aware; same-run retry remains the default recommendation
- standard report/recovery/operator surfaces still show a trust badge more clearly than they show frozen-vs-runtime binding comparison
- no canonical `P016` app-proof entrypoint exists in gates, UI tests, or direct surfaces

### Ambiguities / Evidence Gaps

- the repository documents `./scripts/test-gate.sh` as the canonical agent proving path, but there is no `proposal-016` gate
- raw `xcodebuild -testPlan FastGate test` remains diagnostic-only and yielded a green `0`-test run on the current toolchain
- the current tree has a live full-unit regression outside the explicit `P016` slice, which weakens same-head proof completeness even where the new runtime code is present

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 5 |
| Missing | 3 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical terminal outcomes are explicitly persisted on `AgentExecution`, including truthful cancellation and limit exhaustion
- Proposal Source: `4.2`, `4.3`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
- Gap / Note: The taxonomy and persisted columns are real, and ordinary execution paths write them. The stop-path does not: `RunCancellationCoordinator.beginSettlement()` still sets only `agentExec.status = .cancelled` plus coarse run-level settlement metadata. No live writer path classifies `cancelled_before_output` or `cancelled_after_output`.

### REQ-002 Neutral finish markers, post-output transport errors, validation failure after output, and limit exhaustion settle without ambiguous success/error truth
- Proposal Source: `4.2`, `4.3.1`, acceptance criteria `3`, `4`, `5`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
- Gap / Note: `GooseAgentExecutor` explicitly distinguishes neutral finish markers, timeouts, completed-with-transport-error, and limit exhaustion, and `WorkflowOrchestrator` persists the resolved canonical outcome instead of leaving contradictory success/error truth in readers.

### REQ-003 Legacy migration/backfill is deterministic and fail-closed, including `legacy_unverifiable` behavior and deterministic lineage derivation
- Proposal Source: `4.4`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - repository-wide search for `legacy_unverifiable`, backfill, and migration hooks in runtime truth owners
- Gap / Note: The only live legacy handling is “require explicit resume when runtime truth is unknown/unverifiable.” There is no explicit backfill/migration path that writes `legacy_unverifiable`, backfills canonical outcomes from durable evidence, or performs the deterministic old-row lineage derivation promised by `4.4`.

### REQ-004 One logical stage lineage cannot have more than one active execution; startup repair must reconcile stale siblings before new work begins
- Proposal Source: `5.2`, `7.3`, `7.4`, acceptance criteria `7`, `9`, `10`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
- Gap / Note: Startup repair is real and strong: stale active siblings are reconciled, lineage is assigned, and terminal agent truth can settle a stale `running` stage. The create-path prevention story is weaker than the proposal contract. `claimOrCreateStageExecution(...)` reuses the canonical active stage, but there is no explicit `ActiveExecutionUniquenessGuard` that fail-closes every stage-creation boundary before duplicate active siblings can exist.

### REQ-005 One logical approval lineage cannot have more than one active approval record at a time
- Proposal Source: `7.2`, `7.2.1`, acceptance criterion `8`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
- Gap / Note: `Approval.lineageID` and `Approval.repairedAt` are persisted, approval creation/restoration preserves lineage, and duplicate requested siblings are deterministically expired on repair.

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
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
- Gap / Note: Aggregate settlement is a real persisted subordinate owner keyed by `stageExecutionID` and `lineageID`, and both recovery and reporting traverse through stage truth first.

### REQ-007 Reports and operator surfaces show frozen binding truth separately from actual runtime truth and downgrade to `unverifiable` when runtime evidence is weak
- Proposal Source: `6.2`, `6.3`, acceptance criterion `13`
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - full unit run log shows `RunReportBuilder prefers runtime provider and model truth over frozen bindings` and `RunReportBuilder marks runtime truth verified when canonical receipt identity exists` executed before the broader crash: `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-20-48-+0300.xcresult`
- Gap / Note: The trust resolver and downgrade path are real. The ordinary report/recovery shell still mostly exposes a merged provider/model label plus `RuntimeProvenanceBadge`; it does not yet clearly present frozen configuration as comparison context beside runtime fact across the standard operator surfaces.

### REQ-008 Reports and recovery derive failed-step identity, retry path, resume path, and aggregate evidence from canonical settlement/recovery records
- Proposal Source: `3` Layer V, `5.4`, acceptance criterion `12`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - full unit run log shows:
    - `RunReportBuilder collapses retries into canonical stage lineage`
    - `RunReportBuilder preserves distinct lineages even when stage identifiers match`
    - `RunReportBuilder synthesizes failure evidence and canonical retry path when packet is missing`
    - `RunReportBuilder collapses duplicate approvals by lineage`
    all executing successfully before the wider crash in `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-20-48-+0300.xcresult`
- Gap / Note: The narrative/report path now clearly resolves through canonical stages, approvals, recovery snapshots, and aggregate settlement rather than raw historical scans.

### REQ-009 Limit exhaustion and provider policy-bound terminal stops default to non-auto-retryable unless a narrower explicit override is persisted
- Proposal Source: `7.4`, `7.5`, acceptance criterion `6`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
- Gap / Note: `StageRetryCoordinator.narrowestRecoveryAction(...)` only special-cases output-contract mismatch. It does not inspect `limitExhausted*`, provider policy-bound stop reasons, or any persisted override flag. The default recommendation remains ordinary same-run retry.

### REQ-010 Section 8.1 unit and integration proof is complete
- Proposal Source: `8.1`
- Status: Partially Implemented
- Evidence Type: tests-found, tests-run
- Evidence:
  - relevant suites and tests exist in:
    - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
    - `Chainworks ForgeTests/OrchestratorTests.swift`
    - `Chainworks ForgeTests/ResumeManagerTests.swift`
    - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift`
  - direct focused proof attempts were non-proving `0`-test runs:
    - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-audit-test.JJVOLJ/p016-tests.xcresult`
    - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-18-01-+0300.xcresult`
  - broader unit-target run executed real tests but ended red and unstable:
    - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-20-48-+0300.xcresult`
- Gap / Note: The suite inventory is substantial, but the proof requirement is not fully closed on the current tree. Raw focused plan execution still yields `0` tests, the canonical `fast` gate could not be replayed in this environment because the repo guard detected other ambient tool processes, the wider unit run exposed unrelated red tests plus a SwiftData crash, and no dedicated cancellation-bridge proof for `cancelled_before_output` / `cancelled_after_output` was found.

### REQ-011 Section 8.2 motivating-run replay proof exists and proves the full failure class
- Proposal Source: `8.2`
- Status: Partially Implemented
- Evidence Type: tests-found
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - motivating regression found: `Motivating run regression keeps aggregate absence and limit exhaustion truthful`
- Gap / Note: A nearby regression fixture exists and clearly encodes the same incident class, but it lives under Proposal 013 ownership, was not replayed cleanly to completion in this audit, and does not yet stand as an explicit `P016`-owned proving path for all `8.2` bullets.

### REQ-012 Section 8.3 app-level proof exists and is executable on the current tree
- Proposal Source: `8.3`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `scripts/test-gate.sh`
  - `docs/reference/test-gates.md`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
- Gap / Note: There is no `proposal-016` gate, no `UITestProposal016...` direct surface, no dedicated harness, and no app-launched proof route comparable to the explicit proposal-level paths used by `P007`, `P008`, `P012`, or `P013`.

### REQ-013 Ownership boundaries are explicit across `WorkflowOrchestrator`, `ResumeManager`, `RecoveryCoordinator`, and approval persistence
- Proposal Source: `7.1`, acceptance criterion `14`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Models/Approval.swift`
- Gap / Note: The owner split described by the proposal is already visible in the live code structure even though some guard/policy responsibilities remain incomplete.

## Expert Findings

### ARCH-001 Stop-path cancellation truth is still split across two authorities
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-001`, `REQ-010`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
- Why It Matters: `P016` exists specifically to stop contradictory runtime history. As long as the operator stop path writes only coarse `.cancelled` status and run-level settlement log entries, report/recovery readers still have to reason around an execution class that the canonical outcome columns were supposed to own.
- Recommended Action: make cancellation settlement write `canonicalOutcome`, `outputPresence`, and `settledAt` for affected `AgentExecution` rows using the same explicit before-output / after-output distinction the proposal requires.

### ARCH-002 Stage uniqueness prevention still leans on canonical selection plus startup repair
- Severity: Major
- Confidence: Medium
- Related Proposal Items: `REQ-004`, `REQ-013`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
- Why It Matters: startup repair is strong, but the create-path contract in `7.3` is stricter. The system should prevent duplicate active siblings before they exist, not just choose the newest active stage and repair leftovers later.
- Recommended Action: add an explicit active-lineage guard on stage-creation / waiting-approval creation boundaries that fail-closes or repairs immediately rather than relying on “pick canonical active stage” behavior.

### PROD-001 Recovery still over-recommends retry for exhaustion and policy-bound terminal stops
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-009`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
- Why It Matters: the proposal explicitly tightens operator safety here. Exhaustion and provider policy-bound stops are not ordinary transient transport blips. Offering same-run retry by default creates exactly the kind of misleading recovery path the proposal is trying to remove.
- Recommended Action: extend `RecoveryActionSnapshot` construction to inspect canonical outcome plus provider stop reason, emit `operatorInspection` or clone-only defaults for non-auto-retryable classes, and require an explicit persisted override before recommending retry.

### UI-001 Runtime-binding migration is only partially visible in standard operator surfaces
- Severity: Minor
- Confidence: Medium
- Related Proposal Items: `REQ-007`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Why It Matters: a trust badge is not the same as an explicit comparison between frozen configuration and actual runtime evidence. When runtime truth is downgraded to `unverifiable`, operators still need to see what was configured versus what the runtime could really prove.
- Recommended Action: add a compact provenance panel on report/recovery surfaces that shows frozen provider/model + provenance source next to runtime provider/model + trust outcome instead of only showing the badge.

### READY-001 Proposal 016 still has no canonical proving path
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-010`, `REQ-011`, `REQ-012`
- Evidence Type: tests-run, tests-found
- Evidence References:
  - `scripts/test-gate.sh`
  - `docs/reference/test-gates.md`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-audit-test.JJVOLJ/p016-tests.xcresult`
  - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-18-01-+0300.xcresult`
  - `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.29_22-20-48-+0300.xcresult`
- Why It Matters: the implementation may continue to improve, but sign-off remains weak until `P016` has an owned proof path. Right now the current toolchain can still produce green `0`-test direct runs, there is no `proposal-016` gate, there is no app-level harness, and the broader unit target is not stable enough to serve as a clean same-head proof bundle.
- Recommended Action: add a canonical `proposal-016` gate or equivalent owned proof slice, include at least one motivating-run replay and one app-level proof path, and keep that path independent from the unrelated red suites currently polluting whole-target execution.

## Key Evidence Notes

- `build` is green on the current tree:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p016-audit-build.O9JYwX/p016-build.xcresult`
- raw focused proof is still unsafe to overclaim:
  - direct selected tests returned green with `0` executed tests
  - direct `FastGate.xctestplan` execution also returned green with `0` executed tests
- wider unit proof is non-zero but red:
  - full unit-target run executed many Swift Testing suites
  - unrelated red tests observed in the current tree included:
    - `Explicit output contract only applies to matching output`
    - `GooseServerTransport stores initialization parameters correctly`
  - run then hit a live `SwiftData/BackingData.swift:844` fatal crash

## Final Assessment

`P016` is no longer just an idea. The current repository already contains most of the intended storage model and much of the intended reader logic. But the proposal is still not complete because the remaining gaps are exactly the gaps the draft made non-optional: canonical cancellation outcome writing, fail-closed legacy migration/backfill, provider-aware non-auto-retryable recovery defaults, and a real proposal-owned proof path.
