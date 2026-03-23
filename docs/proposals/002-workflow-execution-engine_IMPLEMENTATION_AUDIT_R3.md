# Proposal 002: Workflow Execution Engine — RunPlan Compiler, Orchestrator, and Approval Flow Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/002-workflow-execution-engine.md` |
| Repository Root | `.` |
| Git SHA | `c490e6b` |
| Working Tree | `dirty (16 entries: modified and untracked files present)` |
| Audited At | `2026-03-23T15:11:26+0200` |
| Proposal State | `Active Draft` |
| Overall Status | `Not Implemented` |

## Verdict

Proposal 002 is substantially implemented: the compiler, compact normalization, workspace-backed run creation, app-scoped execution service, simulated orchestration path, artifact storage boundary, and most of the execution UI are present and heavily tested. The overall status is still `Not Implemented` because the proposal explicitly requires a green fresh `xcodebuild build && xcodebuild test`, and this audit's full `xcodebuild test` surfaced real failures before completion, including multiple `ResumeManagerTests` regressions. In addition, the proposal's full execution product checkpoint remains not fully verifiable in this headless macOS environment because the key UI tests skip behind runtime guards.

## Proposal Contract

### Scope

- Build Layer C execution engine components: `RunPlanCompiler`, `CompactNormalizer`, `WorkflowOrchestrator`, `AgentExecutor` + simulated executor, `ArtifactManager`, `TransitionEvaluator`, cost tracking, and `ResumeManager`. Proposal source: `## 2. What we build` at `docs/proposals/002-workflow-execution-engine.md:49`.
- Build Layer D execution UI: Start Run flow, Run Progress, Approval Gate, Stage Detail, and Artifact Inspector. Proposal source: `## 2. What we build` at `docs/proposals/002-workflow-execution-engine.md:69`.

### Locked Decisions

- Two-phase compiler: `previewCompile()` is side-effect free; `createRun()` is the irreversible persistence step. Proposal source: `## 3. RunPlan Compiler` at `docs/proposals/002-workflow-execution-engine.md:217`.
- `RunPlan` is pure execution topology with no run-scoped identity; identity lives on `Run`. Proposal source: `## 3. RunPlan Compiler` at `docs/proposals/002-workflow-execution-engine.md:105`.
- `ExecutionService` is app-scoped and owns orchestrators independently of view lifetime. Proposal source: `## 5. Workflow Orchestrator` at `docs/proposals/002-workflow-execution-engine.md:411`.
- `ArtifactStorage` owns disk I/O; `ArtifactManager` only records SwiftData metadata on `@MainActor`. Proposal source: `## 7. Artifact Manager` at `docs/proposals/002-workflow-execution-engine.md:841`.
- Resume reconstructs from frozen snapshots and blocks on compiler drift or dangerous states. Proposal source: `## 10. Resume Manager` at `docs/proposals/002-workflow-execution-engine.md:1018`.

### Acceptance Criteria

- Canonical and compact workflow compilation must succeed with resolved agents and provenance hashes. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1505`.
- Orchestrator must execute sequential/parallel/approval/loop/failure-policy behavior and complete the canonical workflow with the simulated executor. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1513`.
- Artifacts must follow the workspace path convention and remain single-owner under `ArtifactManager`. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1532`.
- Resume must detect interrupted runs, block on compiler/version drift, auto-resume safe stages, and restore approval gates. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1546`.
- Execution UI must expose Start Run, Run Progress, Approval inbox/gate, Stage Detail, and Artifact Inspector. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1557`.
- General gate requires app launch/build success and green `xcodebuild build && xcodebuild test`. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1565`.
- Product checkpoint requires create idea -> start run -> observe full canonical execution -> approve 3 gates -> inspect artifacts -> complete in under 120 seconds. Proposal source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1571`.

### Test / Evidence Requirements

- Dedicated test suites for compiler, orchestrator, transition evaluator, artifact manager, resume manager, simulated executor, end-to-end integration, workspace isolation, and execution UI. Proposal source: `## 12. Testing` at `docs/proposals/002-workflow-execution-engine.md:1199`.

### Explicit Exclusions

- Goose REST/SSE adapter and real provider calls are out of Proposal 002 scope and targeted at Proposal 003. Proposal source: `## 15. What's NOT in scope` at `docs/proposals/002-workflow-execution-engine.md:1578`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
| Partially Implemented | 2 |
| Missing | 1 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Canonical RunPlan compilation and provenance hashing
- Proposal Source: `## 3. RunPlan Compiler` at `docs/proposals/002-workflow-execution-engine.md:83`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1505`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:38`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:50`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:87`
  - Fresh full test run logged passing `RunPlanCompilerTests.testCompileCanonicalWorkflow`, `testAllAgentsResolved`, and `testProvenanceHashes`
- Gap / Note: None for canonical compile/provenance path in this audit.

### REQ-002 Compact workflow normalization and alias resolution
- Proposal Source: `## 4. Compact Normalizer` at `docs/proposals/002-workflow-execution-engine.md:335`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1509`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/DSL/CompactNormalizer.swift`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:229`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:239`
  - `Chainworks ForgeTests/EndToEndTests.swift:167`
  - Fresh full test run logged passing `RunPlanCompilerTests.testCompactNormalization`, `testCompactAliasResolution`, and `EndToEndTests.testCompactWorkflowEndToEnd`
- Gap / Note: None for the compact compile path proved here.

### REQ-003 Phase-2 run creation persists workspace context and keeps execution records lazy
- Proposal Source: `## 3. RunPlan Compiler` at `docs/proposals/002-workflow-execution-engine.md:240`, `docs/proposals/002-workflow-execution-engine.md:279`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1548`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:253`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:277`
  - `Chainworks ForgeTests/OrchestratorTests.swift:598`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:391`
  - Fresh full test run logged passing `RunPlanCompilerTests.testCreateRunPersistsCorrectly`, `testPreviewCompileDoesNotPersist`, `OrchestratorTests.testStageExecutionsCreatedLazily`, and `RunTests.noDirectRunConstruction`
- Gap / Note: None for the persistence boundary itself.

### REQ-004 App-scoped execution ownership and startup resume wiring
- Proposal Source: `## 5. Workflow Orchestrator` at `docs/proposals/002-workflow-execution-engine.md:411`, `## 13. File structure` at `docs/proposals/002-workflow-execution-engine.md:1483`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift:16`
  - `Chainworks Forge/Engine/ExecutionService.swift:75`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks ForgeTests/EndToEndTests.swift:219`
- Gap / Note: `ExecutionService` is correctly bootstrapped at app launch and injected through the environment. The resume classification logic beneath it is covered separately in REQ-007.

### REQ-005 Orchestrator drives simulated execution, approvals, loops, cancellation, and failure policy
- Proposal Source: `## 5. Workflow Orchestrator` at `docs/proposals/002-workflow-execution-engine.md:455`, `docs/proposals/002-workflow-execution-engine.md:525`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1513`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/TransitionEvaluator.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift:219`
  - `Chainworks ForgeTests/OrchestratorTests.swift:346`
  - `Chainworks ForgeTests/OrchestratorTests.swift:393`
  - `Chainworks ForgeTests/OrchestratorTests.swift:648`
  - `Chainworks ForgeTests/OrchestratorTests.swift:701`
  - `Chainworks ForgeTests/OrchestratorTests.swift:767`
  - `Chainworks ForgeTests/EndToEndTests.swift:91`
  - Fresh full test run logged passing `OrchestratorTests.testParallelExecution`, `testApprovalGatePausesExecution`, `testApprovalGrantedResumesExecution`, `testApprovalRejectedCancels`, `testRunAfterApproval`, `testCostAggregation`, and `EndToEndTests.testFullCanonicalWorkflow`
- Gap / Note: The simulated Proposal 002 execution path is solid in this audit.

### REQ-006 Artifact storage boundary and workspace isolation
- Proposal Source: `## 7. Artifact Manager` at `docs/proposals/002-workflow-execution-engine.md:804`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1532`, `docs/proposals/002-workflow-execution-engine.md:1540`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:69`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:164`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:231`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:70`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:85`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:108`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:165`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:230`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:277`
  - Fresh full test run logged passing the artifact manager and workspace isolation suites cited above
- Gap / Note: None for the Proposal 002 artifact/workspace contract.

### REQ-007 Resume detection, safe classification, and drift/compiler blocking
- Proposal Source: `## 10. Resume Manager` at `docs/proposals/002-workflow-execution-engine.md:1018`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1546`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:33`
  - `Chainworks Forge/Engine/ResumeManager.swift:46`
  - `Chainworks Forge/Engine/ResumeManager.swift:65`
  - `Chainworks Forge/Engine/ResumeManager.swift:115`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:75`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:96`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:106`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:118`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:137`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:186`
  - Fresh full `xcodebuild test` emitted failures in `ResumeManagerTests.testCompletedRunsNotFound`, `testCancelledRunsNotFound`, `testClassifyResumeableRun`, `testClassifyCompilerVersionMismatch`, and `testExecutionServiceCancelRun`
- Gap / Note: The resume subsystem exists, but its core filtering/classification contract is currently regressed under fresh test evidence. That blocks a clean claim that interrupted/completed/cancelled runs are handled according to the proposal's safety rules.

### REQ-008 Cost aggregation and Proposal 001 run invariants remain intact
- Proposal Source: `## 9. Cost Tracking` at `docs/proposals/002-workflow-execution-engine.md:1006`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1546`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/EndToEndTests.swift:261`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:278`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:334`
  - Fresh full test run logged passing `EndToEndTests.testCostAggregationEndToEnd`, `RunTests.currentStageIDDerived`, and `RunTests.driftDecisionPersists`
- Gap / Note: No new gap surfaced here.

### REQ-009 Execution UI surfaces exist in the application
- Proposal Source: `## 11. Execution UI` at `docs/proposals/002-workflow-execution-engine.md:1085`, `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1557`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:234`
  - `Chainworks Forge/Views/IdeaListView.swift:330`
  - `Chainworks Forge/Views/IdeaListView.swift:670`
  - `Chainworks Forge/Views/IdeaListView.swift:910`
  - `Chainworks Forge/Views/IdeaListView.swift:964`
  - `Chainworks Forge/Views/IdeaListView.swift:1036`
  - `Chainworks Forge/Views/IdeaListView.swift:1063`
  - `Chainworks Forge/Views/ApprovalGateView.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:392`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:454`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:486`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:536`
- Gap / Note: The screens and accessibility hooks exist, including markdown rendering in the artifact inspector. Fresh UI-test evidence is still partial because several execution-surface tests skip behind environment guards when tabs or toolbar flows are not discoverable in headless macOS.

### REQ-010 Proposal 002 test inventory is materially implemented in-repo
- Proposal Source: `## 12. Testing` at `docs/proposals/002-workflow-execution-engine.md:1199`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/EndToEndTests.swift`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - Fresh full test run executed tests across all of these suites
- Gap / Note: Inventory presence is not the problem; green execution of that inventory is covered separately in REQ-013.

### REQ-011 Product checkpoint full execution proof under 120 seconds
- Proposal Source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1571`
- Status: `Not Verifiable`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:212`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:262`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:589`
  - Fresh full `xcodebuild test` run passed `testProductCheckpointScaffoldFlowUnder60Seconds` in `45.728s`
  - Fresh full `xcodebuild test` run skipped `testProductCheckpointExecutionFlowReachable` and `testFullProductCheckpointCanonicalExecution` due headless macOS discoverability guards
- Gap / Note: The scaffold checkpoint is proven, but the proposal's stronger execution checkpoint is not closed in this environment. That is an evidence gap, not proof of absence.

### REQ-012 Fresh macOS build succeeds
- Proposal Source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1565`
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - Fresh command: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r3-build.C1nyRG build`
  - Result: `** BUILD SUCCEEDED **`
- Gap / Note: None for the build gate.

### REQ-013 Fresh `xcodebuild test` gate is green
- Proposal Source: `## 14. Acceptance criteria` at `docs/proposals/002-workflow-execution-engine.md:1569`, `docs/proposals/002-workflow-execution-engine.md:1573`
- Status: `Missing`
- Evidence Type: `tests-run`
- Evidence:
  - Fresh command: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r3-test.FdZNMC test`
  - Result during this audit: real failures surfaced before the audit terminated the long-running suite after failure state was established
  - Observed failing tests included:
    - `EndToEndTests.testLiveProposalLoopFixtureReachesApprovalAndCompletes()`
    - `ResumeManagerTests.testCompletedRunsNotFound()`
    - `ResumeManagerTests.testCancelledRunsNotFound()`
    - `ResumeManagerTests.testClassifyResumeableRun()`
    - `ResumeManagerTests.testClassifyCompilerVersionMismatch()`
    - `ResumeManagerTests.testExecutionServiceCancelRun()`
  - Partial result bundle path: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r3-test.FdZNMC/Logs/Test/Test-Chainworks Forge-2026.03.23_14-59-39-+0200.xcresult`
- Gap / Note: This requirement is explicitly binary in the proposal. Even if some failures come from slices beyond strict Proposal 002 scope, the proposal still demands a green global suite, and that gate did not close in this audit.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/002-workflow-execution-engine.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "..."` and `sed`/`nl -ba` reads over:
  - `docs/proposals/002-workflow-execution-engine.md`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/ApprovalGateView.swift`
  - `Chainworks ForgeTests/*.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r3-build.C1nyRG build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r3-test.FdZNMC test`

## Recommended Next Actions

- Fix the `ResumeManager` regression cluster until `ResumeManagerTests` is green again, starting with interrupted-run filtering for `.completed`/`.cancelled` and resume classification expectations.
- Re-close the proposal's explicit green-suite gate by getting a full fresh `xcodebuild test` to complete without failures.
- Stabilize a macOS UI proof path for the full execution checkpoint so `testProductCheckpointExecutionFlowReachable` and `testFullProductCheckpointCanonicalExecution` stop skipping in audit conditions.
