# Proposal 002: Workflow Execution Engine — RunPlan Compiler, Orchestrator, and Approval Flow Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | docs/proposals/002-workflow-execution-engine.md |
| Repository Root | . |
| Git SHA | 11bea23 |
| Working Tree | clean |
| Audited At | 2026-03-22T23:03:26+02:00 |
| Proposal State | Active (Draft, not superseded) |
| Overall Status | Not Implemented |

## Verdict

Proposal 002 is not implemented end-to-end in the current repository. The engine core is substantially present: compilation, compact normalization, orchestration, transition evaluation, artifact persistence, and much of resume handling already exist in code with meaningful unit-test coverage. The proposal still fails its own implementation gate because the execution UI layer is absent, app-scoped execution is not wired into the app shell, approval rejection semantics diverge from the proposal contract, and the product-checkpoint flow cannot currently be exercised from the running app.

## Proposal Contract

### Scope
- Build the execution engine layer: `RunPlanCompiler`, `CompactNormalizer`, `WorkflowOrchestrator`, `ExecutionService`, `AgentExecutor`, `SimulatedAgentExecutor`, `ArtifactManager`, `TransitionEvaluator`, and `ResumeManager`.
- Extend persistence so runs are created from a compiled plan with frozen provenance and run-scoped workspace paths.
- Add execution UI surfaces: enhanced `IdeaDetailView`, Start Run sheet, Run Progress, Approval Gate, Stage Detail, and Artifact Inspector.
- Verify the canonical `workflow.yaml` / `agents.yaml` flow and the compact `proposal-to-release.yaml` path.

### Locked Decisions
- `previewCompile()` is side-effect-free and `createRun()` is the irreversible persistence boundary.
- `ExecutionService` is app-scoped, injected via the root SwiftUI environment, and resumes interrupted runs on app startup.
- `ArtifactStorage` owns disk I/O and `ArtifactManager` owns SwiftData metadata updates.
- `RunPlan` carries no run-scoped identity; `RunWorkspace` is frozen at run creation.
- `StageExecution` and `AgentExecution` are created lazily.

### Acceptance Criteria
- Canonical and compact workflows compile into a valid `RunPlan` with correct agent resolution and provenance.
- The orchestrator supports sequential, parallel, approval-gated, and full canonical workflow execution.
- Artifacts are stored under the canonical path convention and retrievable without boundary violations.
- Cost aggregation, interrupted-run detection, compiler-version safety, safe resume, and drift detection work.
- The execution UI exposes Start Run, run progress, approval inbox/detail, stage detail, and artifact inspection.
- `xcodebuild build && xcodebuild test` is green and the product checkpoint flow completes the 12-state simulated workflow under 120 seconds.

### Test / Evidence Requirements
- Section 12 requires dedicated compiler, orchestrator, transition, artifact manager, resume, simulated executor, end-to-end, workspace isolation, and execution UI tests.
- Product checkpoint requires live proof that an engineer can create an idea, start a run, approve three gates, observe all 12 states, and inspect artifacts.

### Explicit Exclusions
- Real provider adapters, real LLM calls, worktree management, drift-review UI, and completed run report generation remain out of scope for Proposal 002.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 4 |
| Missing | 2 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 RunPlan compiler compiles canonical workflow and records provenance
- Proposal Source: `## 3. RunPlan Compiler` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:83`, `docs/proposals/002-workflow-execution-engine.md:1505`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:17-58`
  - `Chainworks Forge/Engine/RunPlan.swift:7-39`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:38`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:50`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:87`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:100`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:109`
- Gap / Note: The core compile path, provenance hashing, variables, and scoring preservation are implemented. The broader section-12 negative-test inventory is audited separately in `REQ-011`.

### REQ-002 Compact workflow normalization and deterministic alias resolution exist
- Proposal Source: `## 4. Compact workflow path` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:335`, `docs/proposals/002-workflow-execution-engine.md:1509`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/DSL/CompactNormalizer.swift:3-197`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:61-67`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:229`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:239`
- Gap / Note: The normalization path and explicit alias strategy are present. Missing proposal-listed negative compact tests are tracked under `REQ-011`.

### REQ-003 Phase-2 persistence creates run-scoped workspace and keeps execution records lazy
- Proposal Source: `## 3. RunPlan Compiler`, `## 8. Resume`, locked decisions `ARCH-021`, `ARCH-025`, `ARCH-027`, and acceptance criteria (`docs/proposals/002-workflow-execution-engine.md:217`, `docs/proposals/002-workflow-execution-engine.md:1018`, `docs/proposals/002-workflow-execution-engine.md:1601`, `docs/proposals/002-workflow-execution-engine.md:1548`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:72-129`
  - `Chainworks Forge/Models/RunRepository.swift:95-127`
  - `Chainworks Forge/Models/Run.swift:22-30`
  - `Chainworks Forge/Models/Run.swift:48-56`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:253`
  - `Chainworks ForgeTests/OrchestratorTests.swift:498`
- Gap / Note: The persisted `Run` now carries `workspaceRoot`, `artifactRoot`, and `planCompilerVersion`, and stage executions are created lazily rather than at run creation.

### REQ-004 Core orchestrator executes linear, sequential, parallel, failure, and artifact-driven flows
- Proposal Source: `## 6. Workflow Orchestrator` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:455`, `docs/proposals/002-workflow-execution-engine.md:1513`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:101-110`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:160-180`
  - `Chainworks ForgeTests/OrchestratorTests.swift:73`
  - `Chainworks ForgeTests/OrchestratorTests.swift:129`
  - `Chainworks ForgeTests/OrchestratorTests.swift:190`
  - `Chainworks ForgeTests/OrchestratorTests.swift:341`
  - `Chainworks ForgeTests/OrchestratorTests.swift:389`
  - `Chainworks ForgeTests/OrchestratorTests.swift:440`
  - `Chainworks ForgeTests/OrchestratorTests.swift:548`
- Gap / Note: The engine core exists and has broad unit coverage for linear, parallel, failure, cancellation, artifact-exists transitions, and cost aggregation. The proposal’s full-canonical-flow proof is still missing and is tracked separately.

### REQ-005 Approval semantics must pause, resume, cancel on rejection, and support `run_after_approval`
- Proposal Source: `## 6. Workflow Orchestrator`, `## 11. Execution UI`, and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:952`, `docs/proposals/002-workflow-execution-engine.md:1153`, `docs/proposals/002-workflow-execution-engine.md:1518`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:112-158`
  - `Chainworks ForgeTests/OrchestratorTests.swift:246`
  - `Chainworks ForgeTests/OrchestratorTests.swift:293`
- Gap / Note: Approval pause and approval-granted resume exist, and `run_after_approval` code is present. The rejection path does not follow the proposal: it sets `Run.status = .failed` instead of cancelling the run, and the proposal-listed tests `testApprovalRejectedCancels()` and `testRunAfterApproval()` are not present by name.

### REQ-006 Transition evaluation supports the canonical condition set
- Proposal Source: `## 9. Transition Evaluator` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:1062`, `docs/proposals/002-workflow-execution-engine.md:1526`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/TransitionEvaluator.swift:3-255`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift:24`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift:96`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift:118`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift:134`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift:180`
- Gap / Note: The evaluator supports `always`, `exists(...)`, `approval.granted`, numeric comparisons, variable substitution, and `and` / `or` composition.

### REQ-007 Artifact storage and metadata persistence follow the single-owner contract
- Proposal Source: `## 7. Artifact Manager`, locked decisions `ARCH-023`, `ARCH-026`, and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:806`, `docs/proposals/002-workflow-execution-engine.md:1603`, `docs/proposals/002-workflow-execution-engine.md:1532`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift:4-93`
  - `Chainworks Forge/Engine/ArtifactManager.swift:4-155`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:69`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:137`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:164`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:207`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:310`
- Gap / Note: The path convention, boundary checks, checksuming, metadata persistence, and single-owner write path are implemented in code and covered by targeted tests.

### REQ-008 Resume safety must detect interrupted runs, preserve compiler safety, and stamp drift events
- Proposal Source: `## 8. Resume Manager`, locked decisions `ARCH-029`, and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:1018`, `docs/proposals/002-workflow-execution-engine.md:1609`, `docs/proposals/002-workflow-execution-engine.md:1546`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:4-189`
  - `Chainworks Forge/Engine/ExecutionService.swift:84-132`
  - `Chainworks Forge/Models/Run.swift:27-30`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:75`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:123`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:142`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:165`
- Gap / Note: Interrupted-run discovery, compiler-version blocking, and drift classification exist. The proposal explicitly requires drift detection to set `driftDetectedAt`, but `ExecutionService.resumeInterruptedRuns()` currently sets only `driftDetails` for `needsDecision` / `cannotResume` cases.

### REQ-009 `ExecutionService` must be app-scoped and wired into root app lifecycle
- Proposal Source: locked decision `ARCH-022`, `## 10. Resume on launch`, and `## 11. Execution UI` (`docs/proposals/002-workflow-execution-engine.md:1085`, `docs/proposals/002-workflow-execution-engine.md:1602`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:5-208`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:29-34`
  - `Chainworks Forge/ContentView.swift:29-52`
- Gap / Note: `ExecutionService` exists and is explicitly modeled as app-scoped, but the app does not instantiate it, inject it into the environment, or call `resumeInterruptedRuns()` on launch. The current shell still boots straight into `ContentView()` with only Proposal 001 scaffold tabs.

### REQ-010 Execution UI surfaces from Proposal 002 must exist and be reachable
- Proposal Source: `## 11. Execution UI` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:1085`, `docs/proposals/002-workflow-execution-engine.md:1557`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:155-185`
  - `Chainworks Forge/ContentView.swift:29-52`
  - `rg -n "StartRunSheet|RunProgressView|ApprovalGateView|StageDetailView|ArtifactInspectorView" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'` (no matches)
- Gap / Note: `IdeaDetailView` still shows only idea metadata and the runs list. There is no `[Start New Run]` action, no Start Run sheet, no run progress screen, no approval inbox/detail surface, no stage detail screen, and no artifact inspector implementation in the codebase.

### REQ-011 Section-12 test inventory must exist for engine, E2E, workspace isolation, and execution UI
- Proposal Source: `## 12. Testing` (`docs/proposals/002-workflow-execution-engine.md:1199`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `Chainworks ForgeTests/TransitionEvaluatorTests.swift`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `Chainworks ForgeTests/SimulatedAgentExecutorTests.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:36`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITestsLaunchTests.swift`
  - `rg -n "testStartRunSheet|testRunProgress|testApprovalGate|testStageDetail|testArtifactInspector|EndToEndTests|WorkspaceIsolationTests|testFullCanonicalWorkflow" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'` (no matches)
- Gap / Note: The repository has meaningful unit coverage for compiler, orchestrator, transition, artifact, resume, and simulated executor behavior. It is still missing the proposal-promised end-to-end canonical workflow test, workspace isolation test file, and execution UI-specific UI tests.

### REQ-012 Product checkpoint flow must be executable from the app
- Proposal Source: `## 14. Acceptance criteria` product checkpoint (`docs/proposals/002-workflow-execution-engine.md:1571`)
- Status: Missing
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:155-185`
  - `Chainworks Forge/ContentView.swift:29-52`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:36`
- Gap / Note: The only live UI checkpoint still exercises the Proposal 001 scaffold flow. The app has no reachable path for `idea -> start run -> approve three gates -> observe all 12 states -> inspect artifacts`, so the proposal’s go/no-go product checkpoint is not currently satisfiable.

### REQ-013 General build-and-test gate must be green in this audit
- Proposal Source: `## 14. Acceptance criteria` general section (`docs/proposals/002-workflow-execution-engine.md:1565`)
- Status: Not Verifiable
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-build.Xtpjms" build` (passed)
  - `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-derived-data.X3mojp" test` (launched but did not close within audit window)
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-derived-data.X3mojp/Logs/Test/Test-Chainworks Forge-2026.03.22_22-54-04-+0200.xcresult`
  - `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-build.Xtpjms" test -only-testing:'Chainworks ForgeTests/TransitionEvaluatorTests'` (started, but scheme still rebuilt unrelated UI targets and did not produce a clean pass/fail result within the audit window)
- Gap / Note: Fresh build proof exists. Fresh `xcodebuild test` proof did not close cleanly in this audit, so the full green test gate cannot be claimed here.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/002-workflow-execution-engine.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/002-workflow-execution-engine.md docs docs/reviews`
- `sed -n '1,40p' docs/proposals/002-workflow-execution-engine.md`
- `nl -ba docs/proposals/002-workflow-execution-engine.md | sed -n '80,120p'`
- `nl -ba docs/proposals/002-workflow-execution-engine.md | sed -n '1085,1198p'`
- `nl -ba docs/proposals/002-workflow-execution-engine.md | sed -n '1199,1315p'`
- `nl -ba docs/proposals/002-workflow-execution-engine.md | sed -n '1500,1608p'`
- `find 'Chainworks ForgeTests' -maxdepth 1 -type f | sort && printf '\n-- UI TESTS --\n' && find 'Chainworks ForgeUITests' -maxdepth 1 -type f | sort`
- `rg -n "StartRunSheet|RunProgressView|ApprovalGateView|StageDetailView|ArtifactInspectorView" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `rg -n "testStartRunSheet|testRunProgress|testApprovalGate|testStageDetail|testArtifactInspector|EndToEndTests|WorkspaceIsolationTests|testFullCanonicalWorkflow|testProductCheckpointScaffoldFlowUnder60Seconds" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `rg -n "func test" 'Chainworks ForgeTests/RunPlanCompilerTests.swift' 'Chainworks ForgeTests/OrchestratorTests.swift' 'Chainworks ForgeTests/ArtifactManagerTests.swift' 'Chainworks ForgeTests/ResumeManagerTests.swift' 'Chainworks ForgeTests/SimulatedAgentExecutorTests.swift' 'Chainworks ForgeTests/TransitionEvaluatorTests.swift'`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-build.Xtpjms" build`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-derived-data.X3mojp" test`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-build.Xtpjms" test -only-testing:'Chainworks ForgeTests/TransitionEvaluatorTests'`

## Recommended Next Actions

- Wire `ExecutionService` into `Chainworks_ForgeApp`, inject it into the SwiftUI environment, and call `resumeInterruptedRuns()` at app startup.
- Implement the Proposal 002 UI surfaces: Start Run flow, Run Progress, approval inbox/detail, stage detail, and artifact inspector.
- Align approval rejection behavior with the proposal by cancelling rejected runs instead of failing them, then add the missing approval-specific tests.
- Set `run.driftDetectedAt` in the live resume path when drift is detected.
- Add the missing section-12 tests: full canonical workflow end-to-end, workspace isolation, and execution UI coverage.
