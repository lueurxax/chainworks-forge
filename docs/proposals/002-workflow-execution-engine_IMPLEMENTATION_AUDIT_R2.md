# Proposal 002: Workflow Execution Engine - RunPlan Compiler, Orchestrator, and Approval Flow Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/002-workflow-execution-engine.md |
| Repository Root | . |
| Git SHA | a454e53 |
| Working Tree | clean |
| Audited At | 2026-03-23T00:25:54+02:00 |
| Proposal State | Active (Draft, not superseded) |
| Overall Status | Not Implemented |

## Verdict

Proposal 002 is materially closer than in R1, but it still does not satisfy its own implementation gate. The app now wires an app-scoped `ExecutionService`, exposes Proposal 002 execution screens from the Ideas flow, and fixes the earlier approval-rejection and drift-timestamp gaps. The proposal remains not implemented overall because the fresh general acceptance gate is still red: `xcodebuild build` passed, but `xcodebuild test` failed in `Chainworks_ForgeUITests`, and the live product-checkpoint proof still stops short of the proposal's full 12-state, 3-approval, artifact-inspection flow.

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
| Implemented | 9 |
| Partially Implemented | 3 |
| Missing | 1 |
| Not Verifiable | 0 |

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
- Gap / Note: No new gap surfaced in this audit. Compiler behavior remains implemented as in R1.

### REQ-002 Compact workflow normalization and deterministic alias resolution exist
- Proposal Source: `## 4. Compact workflow path` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:335`, `docs/proposals/002-workflow-execution-engine.md:1509`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/DSL/CompactNormalizer.swift:3-197`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:61-67`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:229`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:239`
- Gap / Note: No new compact-path gap surfaced in code or tests during this pass.

### REQ-003 Phase-2 persistence creates run-scoped workspace and keeps execution records lazy
- Proposal Source: `## 3. RunPlan Compiler`, `## 8. Resume`, locked decisions `ARCH-021`, `ARCH-025`, `ARCH-027`, and acceptance criteria (`docs/proposals/002-workflow-execution-engine.md:217`, `docs/proposals/002-workflow-execution-engine.md:1018`, `docs/proposals/002-workflow-execution-engine.md:1601`, `docs/proposals/002-workflow-execution-engine.md:1548`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:72-129`
  - `Chainworks Forge/Models/RunRepository.swift:95-127`
  - `Chainworks Forge/Models/Run.swift:22-30`
  - `Chainworks Forge/Models/Run.swift:48-56`
  - `Chainworks ForgeTests/OrchestratorTests.swift:498-544`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:69-103`
- Gap / Note: Run-scoped workspace persistence and lazy stage creation now have direct supporting tests in the repo.

### REQ-004 Core orchestrator executes linear, sequential, parallel, failure, and artifact-driven flows
- Proposal Source: `## 6. Workflow Orchestrator` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:455`, `docs/proposals/002-workflow-execution-engine.md:1513`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:185-190`
  - `Chainworks ForgeTests/OrchestratorTests.swift:73`
  - `Chainworks ForgeTests/OrchestratorTests.swift:129`
  - `Chainworks ForgeTests/OrchestratorTests.swift:190`
  - `Chainworks ForgeTests/OrchestratorTests.swift:341-385`
  - `Chainworks ForgeTests/OrchestratorTests.swift:440-494`
  - `Chainworks ForgeTests/EndToEndTests.swift:83-154`
- Gap / Note: The engine core is implemented. The product-checkpoint gap is tracked separately under `REQ-012`, not here.

### REQ-005 Approval semantics must pause, resume, cancel on rejection, and support `run_after_approval`
- Proposal Source: `## 6. Workflow Orchestrator`, `## 11. Execution UI`, and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:952`, `docs/proposals/002-workflow-execution-engine.md:1153`, `docs/proposals/002-workflow-execution-engine.md:1518`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:112-143`
  - `Chainworks ForgeTests/OrchestratorTests.swift:246-337`
  - `Chainworks ForgeTests/OrchestratorTests.swift:601-662`
  - `Chainworks ForgeTests/OrchestratorTests.swift:667-728`
- Gap / Note: This requirement moved forward since R1. Rejection now cancels the run, and the proposal-named approval tests are present.

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
- Gap / Note: No new gap surfaced in this evaluator contract.

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
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:164-205`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:229-335`
- Gap / Note: Workspace isolation coverage is now materially stronger than in R1.

### REQ-008 Resume safety must detect interrupted runs, preserve compiler safety, and stamp drift events
- Proposal Source: `## 8. Resume Manager`, locked decisions `ARCH-029`, and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:1018`, `docs/proposals/002-workflow-execution-engine.md:1609`, `docs/proposals/002-workflow-execution-engine.md:1546`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:4-189`
  - `Chainworks Forge/Engine/ExecutionService.swift:84-129`
  - `Chainworks Forge/Models/Run.swift:27-30`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:75`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:123`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:142`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:165`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:320-340`
- Gap / Note: This requirement moved forward since R1. The live resume path now stamps `driftDetectedAt` for both `needsDecision` and `cannotResume`.

### REQ-009 `ExecutionService` must be app-scoped and wired into root app lifecycle
- Proposal Source: locked decision `ARCH-022`, `## 10. Resume on launch`, and `## 11. Execution UI` (`docs/proposals/002-workflow-execution-engine.md:1085`, `docs/proposals/002-workflow-execution-engine.md:1602`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:29-79`
  - `Chainworks Forge/ContentView.swift:4-64`
  - `Chainworks Forge/Engine/ExecutionService.swift:5-208`
- Gap / Note: This requirement moved forward since R1. The app now creates `ExecutionService` once in `AppBootstrapView`, injects it into the environment, and calls `resumeInterruptedRuns()` on launch.

### REQ-010 Execution UI surfaces from Proposal 002 must exist and be reachable
- Proposal Source: `## 11. Execution UI` and `## 14. Acceptance criteria` (`docs/proposals/002-workflow-execution-engine.md:1085`, `docs/proposals/002-workflow-execution-engine.md:1557`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:189-277`
  - `Chainworks Forge/Views/IdeaListView.swift:314-508`
  - `Chainworks Forge/Views/IdeaListView.swift:512-870`
  - `Chainworks Forge/Views/ApprovalGateView.swift:8-124`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:163-280`
- Gap / Note: The Start Run sheet, Run Progress view, Approval inbox/detail, Stage Detail view, and Artifact Inspector now exist in code and are reachable from the Ideas flow. The remaining gap is the artifact presentation contract: `WorkflowArtifactInspectorView` pretty-prints JSON, but markdown is still displayed as plain monospaced text through `Text(renderedContent)` rather than rendered markdown.

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
  - `Chainworks ForgeTests/EndToEndTests.swift:5-290`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:5-336`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:156-280`
- Gap / Note: The big missing files from R1 are now present, which is a major improvement. The remaining gap is UI coverage depth: the audit found only start-run reachability UI automation, not dedicated proof for Run Progress, Approval Gate, Stage Detail, or Artifact Inspector behavior as independent surfaces.

### REQ-012 Product checkpoint flow must be executable from the app
- Proposal Source: `## 14. Acceptance criteria` product checkpoint (`docs/proposals/002-workflow-execution-engine.md:1571`)
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:215-276`
  - `Chainworks Forge/Views/IdeaListView.swift:546-685`
  - `Chainworks ForgeTests/EndToEndTests.swift:83-154`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:163-280`
- Gap / Note: The product flow is now reachable in the app up to the Start Run sheet, and there is non-UI end-to-end engine coverage. The live checkpoint proof still falls short of the proposal contract: the UI automation does not prove create idea -> start run -> observe the full 12-state canonical execution -> approve 3 gates -> inspect artifacts -> complete under 120 seconds.

### REQ-013 General build-and-test gate must be green in this audit
- Proposal Source: `## 14. Acceptance criteria` general section (`docs/proposals/002-workflow-execution-engine.md:1565`)
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-r2-build.toQW2n" build` (passed)
  - `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-r2-build.toQW2n" test` (failed)
  - `Chainworks_ForgeUITests.testProductCheckpointScaffoldFlowUnder60Seconds` failed at `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:57`
  - `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable` failed at `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:183`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-r2-build.toQW2n/Logs/Test/Test-Chainworks Forge-2026.03.23_00-27-02-+0200.xcresult`
- Gap / Note: This is the blocking requirement for the overall roll-up. Fresh build proof is green, but fresh `xcodebuild test` is not green in this audit.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/002-workflow-execution-engine.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/002-workflow-execution-engine.md docs docs/reviews docs/proposals`
- `sed -n '1,260p' /Users/user/.codex/skills/proposal-implementation-audit/SKILL.md`
- `sed -n '1560,1585p' docs/proposals/002-workflow-execution-engine.md`
- `nl -ba 'Chainworks Forge/Chainworks_ForgeApp.swift' | sed -n '1,120p'`
- `nl -ba 'Chainworks Forge/ContentView.swift' | sed -n '1,120p'`
- `nl -ba 'Chainworks Forge/Views/IdeaListView.swift' | sed -n '180,900p'`
- `nl -ba 'Chainworks Forge/Views/ApprovalGateView.swift' | sed -n '1,180p'`
- `nl -ba 'Chainworks Forge/Engine/WorkflowOrchestrator.swift' | sed -n '110,190p'`
- `nl -ba 'Chainworks Forge/Engine/ExecutionService.swift' | sed -n '80,150p'`
- `nl -ba 'Chainworks ForgeTests/EndToEndTests.swift' | sed -n '1,320p'`
- `nl -ba 'Chainworks ForgeTests/WorkspaceIsolationTests.swift' | sed -n '1,360p'`
- `nl -ba 'Chainworks ForgeTests/OrchestratorTests.swift' | sed -n '230,760p'`
- `nl -ba 'Chainworks ForgeTests/Chainworks_ForgeTests.swift' | sed -n '300,340p'`
- `nl -ba 'Chainworks ForgeUITests/Chainworks_ForgeUITests.swift' | sed -n '1,320p'`
- `rg -n "testStartRunSheet|testRunProgress|testApprovalGate|testStageDetail|testArtifactInspector" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `rg -n "RunProgressView|ApprovalGateView|StageDetailView|ArtifactInspectorView|Start New Run|start-new-run-button|approval-inbox-view|approval-gate-view" 'Chainworks ForgeUITests' 'Chainworks ForgeTests'`
- `rg -n "driftDetectedAt|testCompilerVersionMismatchBlocked|testDriftDetected|resumeInterruptedRuns|testResume" 'Chainworks ForgeTests/ResumeManagerTests.swift' 'Chainworks ForgeTests/Chainworks_ForgeTests.swift' 'Chainworks ForgeTests/LiveProposalWorkflowTests.swift' 'Chainworks ForgeTests'`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-r2-build.toQW2n" build`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination 'platform=macOS' -derivedDataPath "/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-audit-r2-build.toQW2n" test`

## Recommended Next Actions

- Fix the failing UI bootstrap/tab-detection path so fresh `xcodebuild test` is green again.
- Upgrade `WorkflowArtifactInspectorView` to render markdown as formatted markdown, not plain monospaced text.
- Extend the Proposal 002 UI automation from Start Run reachability to the real product checkpoint: full canonical execution, 3 approvals, artifact inspection, and completion under 120 seconds.
