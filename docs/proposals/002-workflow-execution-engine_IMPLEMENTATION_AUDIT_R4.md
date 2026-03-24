# Proposal 002: Workflow Execution Engine — RunPlan Compiler, Orchestrator, and Approval Flow Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/002-workflow-execution-engine.md |
| Repository Root | . |
| Git SHA | 59b28ea |
| Working Tree | dirty |
| Audited At | 2026-03-23T17:28:03+02:00 |
| Proposal State | Active Draft |
| Overall Status | Partial |

## Verdict

Overall status is `Partial`. Proposal 002's engine core is now implemented and backed by a fresh green macOS build plus a fresh green targeted `Chainworks ForgeTests` run, including the R3 problem areas in `ResumeManagerTests` and `EndToEndTests`. The remaining gap is verification, not core implementation: in this headless macOS `xcodebuild` environment, the proposal's execution UI/product-checkpoint proofs still skip on tab and toolbar discovery, and the full scheme-wide `xcodebuild test` gate was started but not closed cleanly within the audit window.

## Proposal Contract

### Scope

- Compile canonical and compact workflow YAML plus the agent catalog into an immutable executable `RunPlan`.
- Persist a run-scoped `Run` and `RunWorkspace`, then drive execution through sequential, parallel, looped, and approval-gated stages.
- Persist artifacts on disk with SwiftData metadata, aggregate execution cost, and support resume classification after interruption.
- Surface execution in the app through Start Run, Run Progress, Approval Inbox/Gate, Stage Detail, and Artifact Inspector UI.

### Locked Decisions

- Engine ownership stays in the SwiftUI app; provider runtimes are adapters, not the control plane.
- Compilation is two-phase: `previewCompile` is side-effect free, `createRun` is the irreversible persistence step.
- `RunPlan` is immutable and carries no run-scoped identity; identity lives on `Run` plus `RunWorkspace`.
- `ExecutionService` is app-scoped and owns orchestrators and pending approvals.
- `ArtifactStorage` handles disk I/O while `ArtifactManager` owns SwiftData metadata.
- `StageExecution` and `AgentExecution` are created lazily.
- Resume blocks on compiler-version mismatch and drift/side-effect decisions.

### Acceptance Criteria

- RunPlan compiler resolves workflows, agents, provenance hashes, and compact normalization correctly.
- Orchestrator executes canonical workflow behavior: sequential phases, parallel fan-out, transitions, approval gates, `run_after_approval`, loop handling, cancellation, and failure policy.
- Artifact paths and workspace isolation follow the proposal contract.
- Cost aggregation and resume rules behave per the model contract.
- Execution UI surfaces are present and the end-to-end product checkpoint can be completed in under 120 seconds.
- Fresh `xcodebuild build` and `xcodebuild test` are green.

### Test / Evidence Requirements

- Section 12 test inventory across compiler, orchestrator, transition evaluator, artifact manager, resume manager, simulated executor, end-to-end integration, workspace isolation, and UI.
- Fresh Apple-platform verification with `xcodebuild`.
- Runtime or UI evidence for the Start Run flow, Run Progress, Approval, Stage Detail, Artifact Inspector, and full checkpoint flow.

### Explicit Exclusions

- Goose REST/SSE adapter and real provider execution layers.
- Multi-provider routing.
- Worktree creation and management.
- Drift-review decision UI.
- Completed run report generation.
- Provider-layer permission enforcement and `.gooseignore`.
- Temporal/Rust backend migration.
- Full general-purpose expression language.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 2 |

## Requirement Audit

### REQ-001 Canonical RunPlan compilation and provenance hashing
- Proposal Source: `§3.2 RunPlan structure` (line 105), `§3.3 Two-phase compilation pipeline` (line 217), `§14 RunPlan Compiler` (line 1505)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:17`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:39`
  - `Chainworks Forge/Engine/RunPlan.swift:38`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:38`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:87`
- Gap / Note: None. Compiler preview path, provenance hashing, and compiler-version stamping are implemented and covered by the green targeted `Chainworks ForgeTests` run.

### REQ-002 Compact workflow normalization and deterministic alias resolution
- Proposal Source: `§4.1 Alias resolution` (line 337), `§4.2 Normalization rules` (line 353), `§4.3 Agent alias resolution` (line 366), `§14 RunPlan Compiler` (line 1505)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/DSL/CompactNormalizer.swift:1`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:62`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:229`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:239`
- Gap / Note: None. Compact YAML normalizes through the dedicated normalizer and passes targeted compiler tests.

### REQ-003 Phase-2 run creation persists workspace and keeps stage records lazy
- Proposal Source: `§3.3 Two-phase compilation pipeline` (line 217), `§3.5 Required model changes` (line 279), `§14 RunPlan Compiler` (line 1505), `§14 Workspace Isolation` (line 1540)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:74`
  - `Chainworks Forge/Models/RunRepository.swift:98`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:294`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:253`
  - `Chainworks ForgeTests/OrchestratorTests.swift:598`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:70`
- Gap / Note: None. `createRun` persists workspace paths and the orchestrator creates stage/agent executions on entry rather than pre-seeding them.

### REQ-004 App-scoped execution ownership and startup resume wiring
- Proposal Source: `§5.1 App-scoped execution service` (line 411), `§10.2 Resume rules` (line 1061), `§14 General` (line 1565)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:41`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:79`
  - `Chainworks Forge/Engine/ExecutionService.swift:42`
  - `Chainworks Forge/Engine/ExecutionService.swift:138`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:181`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:241`
- Gap / Note: None. `ExecutionService` is bootstrapped once, injected via environment, and asked to resume interrupted runs on app startup.

### REQ-005 Orchestrator drives sequential, parallel, approval, loop, transition, and cancellation behavior
- Proposal Source: `§5.2 Orchestrator contract` (line 455), `§5.3 State machine execution loop` (line 498), `§5.4 Run block execution` (line 525), `§5.5 Transition evaluation` (line 614), `§5.6 Loop management` (line 667), `§5.7 Failure handling` (line 679), `§8.1-§8.4` (lines 954-997), `§14 Orchestrator` (line 1513)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:112`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:139`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:375`
  - `Chainworks Forge/Engine/TransitionEvaluator.swift:29`
  - `Chainworks ForgeTests/OrchestratorTests.swift:219`
  - `Chainworks ForgeTests/OrchestratorTests.swift:346`
  - `Chainworks ForgeTests/OrchestratorTests.swift:393`
  - `Chainworks ForgeTests/OrchestratorTests.swift:701`
  - `Chainworks ForgeTests/OrchestratorTests.swift:767`
- Gap / Note: None. The targeted non-UI test run exercised the approval, cancellation, run-after-approval, cost, and lazy-creation paths successfully.

### REQ-006 Artifact storage, path contract, and workspace isolation
- Proposal Source: `§7.1 Storage layout` (line 806), `§7.2 Artifact Manager contract` (line 841), `§7.4 Input binding` (line 942), `§14 Artifact Manager` (line 1532), `§14 Workspace Isolation` (line 1540)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ArtifactManager.swift:16`
  - `Chainworks Forge/Engine/ArtifactManager.swift:85`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:460`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:69`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:164`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:108`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:165`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:277`
- Gap / Note: None. Artifact writes are routed through `ArtifactManager`/`ArtifactStorage`, and workspace boundary tests passed.

### REQ-007 Resume manager rebuilds plans safely and classifies interrupted runs
- Proposal Source: `§10.1 RunPlan reconstruction on resume` (line 1049), `§10.2 Resume rules` (line 1061), `§10.3 Drift detection` (line 1073), `§14 Cost & Resume` (line 1546)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:103`
  - `Chainworks Forge/Engine/ResumeManager.swift:33`
  - `Chainworks Forge/Engine/ResumeManager.swift:67`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:99`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:123`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:143`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:197`
- Gap / Note: None. The exact R3 failing resume/cancel tests now pass in the green targeted test run.

### REQ-008 Cost tracking and derived current-stage model behavior
- Proposal Source: `§5.6 Loop management` (line 667), `§6 Agent Execution Protocol` (line 699), `§9 Cost Tracking` (line 1006), `§14 Cost & Resume` (line 1546)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:10`
  - `Chainworks Forge/Models/Run.swift:49`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:498`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:628`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:278`
  - `Chainworks ForgeTests/OrchestratorTests.swift:648`
  - `Chainworks ForgeTests/EndToEndTests.swift:261`
- Gap / Note: None. Aggregation and derived stage-state behavior are both covered by passing tests.

### REQ-009 Execution UI surfaces exist in the app shell
- Proposal Source: `§11.1 Enhanced IdeaDetailView` (line 1087), `§11.2 Start Run Sheet` (line 1111), `§11.3 Run Progress View` (line 1129), `§11.4 Approval Gate View` (line 1153), `§11.5 Artifact Inspector` (line 1183), `§14 Execution UI` (line 1557)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/ContentView.swift:8`
  - `Chainworks Forge/Views/IdeaListView.swift:205`
  - `Chainworks Forge/Views/IdeaListView.swift:337`
  - `Chainworks Forge/Views/IdeaListView.swift:700`
  - `Chainworks Forge/Views/ApprovalGateView.swift:8`
  - `Chainworks Forge/Views/ApprovalGateView.swift:97`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:365`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:392`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:454`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:486`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:536`
- Gap / Note: The UI surfaces are implemented in source and have dedicated UI tests, but those UI tests skip in this headless audit environment. That affects runtime proof, not code existence.

### REQ-010 Proposal 002 test inventory exists across engine, integration, workspace, and UI layers
- Proposal Source: `§12 Testing` (line 1199)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:1`
  - `Chainworks ForgeTests/OrchestratorTests.swift:1`
  - `Chainworks ForgeTests/ArtifactManagerTests.swift:1`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1`
  - `Chainworks ForgeTests/EndToEndTests.swift:1`
  - `Chainworks ForgeTests/WorkspaceIsolationTests.swift:1`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:212`
- Gap / Note: None. The proposal's requested test categories are present in the repository.

### REQ-011 Product checkpoint flow completes in under 120 seconds with live UI proof
- Proposal Source: `§14 Product checkpoint (PROD-PA-002)` (line 1571)
- Status: Not Verifiable
- Evidence Type: tests-found, runtime
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:589`
  - Full-scheme UI run started via `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-test.vNv7K3 -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-test.xcresult test`
- Gap / Note: Current headless macOS XCUITest could not close this checkpoint proof. `testFullProductCheckpointCanonicalExecution()` skipped on toolbar reachability, and other tab-dependent UI tests skipped on `waitForTabs(...)`.

### REQ-012 Fresh macOS build succeeds
- Proposal Source: `§14 General` (line 1565)
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-build.7pvNLQ build`
- Gap / Note: None. Fresh build completed successfully in this audit.

### REQ-013 Fresh scheme-wide `xcodebuild test` gate is green
- Proposal Source: `§14 General` (line 1565)
- Status: Not Verifiable
- Evidence Type: tests-run
- Evidence:
  - Targeted non-UI test run passed: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-unit.1ebaen -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-unit.xcresult -only-testing:'Chainworks ForgeTests' test`
  - Full-scheme run started but was interrupted after establishing that headless UI tests were skipping repeatedly: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-test.xcresult`
- Gap / Note: I do not have a clean completed full-scheme `xcodebuild test` proof from this audit. The non-UI target is green, but the scheme-level gate remains open until the full run is completed in a stable UI environment.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/002-workflow-execution-engine.md docs/reviews`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-build.7pvNLQ build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-test.vNv7K3 -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-test.xcresult test`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-unit.1ebaen -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-002-audit-r4-unit.xcresult -only-testing:'Chainworks ForgeTests' test`
- `rg -n "func previewCompile\\(|func createRun\\(|createRunFromPlan\\(" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "class WorkflowOrchestrator|func start\\(|func resolveApproval\\(|ArtifactManager|TransitionEvaluator|currentStageID|totalCostCents" 'Chainworks Forge'`

## Recommended Next Actions

- Complete one clean full-scheme `xcodebuild test` run in an environment where SwiftUI tabs and toolbar actions are discoverable by XCUITest.
- Capture a passing runtime proof for `testProductCheckpointExecutionFlowReachable()` and `testFullProductCheckpointCanonicalExecution()`.
- Triage the Swift 6 actor-isolation warnings in `Chainworks ForgeTests` before they turn into hard failures under stricter toolchains.
