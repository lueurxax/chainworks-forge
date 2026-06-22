# MVP Stabilization To First Green Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get one `full-mvp-live` run to `workflow_complete` on current head without stage crashes, stale approval dead-ends, or contract-driven review failures.

**Architecture:** Freeze one canonical proving lane, then remove failure classes in dependency order: approval truth, proposal-review contract truth, workflow transition authority, and only then read-model polish. The engine must become fail-closed at every boundary so we stop inferring truth from stale UI or permissive artifacts.

**Tech Stack:** Swift 6, SwiftData, Swift Testing, YAML workflow/catalog DSL, Goose transport bridge, existing `test-gate.sh` proof lanes.

---

## Scope Freeze

This plan is intentionally narrow. Until Task 6 is green:
- no new product features,
- no broad UI redesign,
- no proposal-expansion work beyond the minimum `017` conflict fallback needed to keep the workflow authoritative,
- no “fix while here” refactors outside touched files.

Success means exactly this:
1. A canonical current-head proof lane starts a `full-mvp-live` run.
2. The run reaches `state_6_workflow_complete`.
3. No approval gate requires stale-UI guessing.
4. `Proposal reviewed` accepts only contract-valid review artifacts.
5. Report and workflow map agree on run status and stage outcome after relaunch.

## File Map

**Execution truth / approvals**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ResumeManager.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Approval.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ApprovalGateView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/BlockedRunRecoveryView.swift`

**Proposal review contract enforcement**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/OutputContractResolverV2.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/FixtureGooseTransport.swift`

**Workflow authority / conflict fallback**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/TransitionEvaluator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/YAMLValidator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/examples/workflows/full-mvp-live.yaml`
- Modify: `/Users/user/Documents/Chainworks Forge/examples/workflows/workflow.yaml`

**Read-model / proof lane**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowMapProjectionService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/WorkflowMapView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh`

**Tests**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ResumeManagerTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal013Tests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/GooseSessionBridgeTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/OrchestratorTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/WorkflowMapProjectionTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/LiveProposalWorkflowTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Create: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`

## Canonical Proof Lane

All tasks below serve one proving lane:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests' \
  -only-testing:'Chainworks ForgeTests/Proposal013Tests' \
  -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' \
  -only-testing:'Chainworks ForgeTests/OrchestratorTests' \
  -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' \
  -only-testing:'Chainworks ForgeTests/LiveProposalWorkflowTests' \
  -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests'
```

Expected end state:
- all selected suites green,
- `./scripts/test-gate.sh mvp-golden-run` green,
- one app-launched `full-mvp-live` proof run ends in `workflow_complete`.

### Task 1: Freeze One Canonical Green Lane

**Files:**
- Create: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh`

- [ ] **Step 1: Write the failing end-to-end proof test**

```swift
import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("MVP Golden Run", .serialized, .tags(.live))
struct MVPGoldenRunTests {
    @Test("full-mvp-live reaches workflow_complete with fixture transport")
    func fullMVPLiveReachesWorkflowComplete() async throws {
        let (container, context) = try makeTestModelContainer()
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let run = makeTestRun(plan: plan, context: context)
        let workspace = try makeTestWorkspace(runID: run.id)
        let executor = FixtureAgentExecutor(mode: .fullMVPGreenPath)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        await executor.waitForIdle()

        #expect(run.status == .completed)
        #expect(run.currentStageID == "state_6_workflow_complete")
    }
}
```

- [ ] **Step 2: Run the new test and verify it fails**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests'
```

Expected: FAIL because the current tree still allows one of the known blockers to break the run before `workflow_complete`.

- [ ] **Step 3: Add a dedicated gate wrapper**

Add a `mvp-golden-run` case in `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh` that runs:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests' \
  -only-testing:'Chainworks ForgeTests/Proposal013Tests' \
  -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' \
  -only-testing:'Chainworks ForgeTests/OrchestratorTests' \
  -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' \
  -only-testing:'Chainworks ForgeTests/LiveProposalWorkflowTests' \
  -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests'
```

- [ ] **Step 4: Re-run the dedicated gate**

Run:

```bash
./scripts/test-gate.sh mvp-golden-run
```

Expected: FAIL on current head, but now with one stable reproduction lane.

- [ ] **Step 5: Commit**

```bash
git add \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift' \
  'Chainworks ForgeTests/FullMVPDeliveryTests.swift' \
  'scripts/test-gate.sh'
git commit -m 'test: add mvp golden run proof lane'
```

### Task 2: Kill Stale Approval Requests And Make Approval Truth Idempotent

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Approval.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ApprovalGateView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ResumeManagerTests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`

- [ ] **Step 1: Write failing approval truth tests**

Add these tests to `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ResumeManagerTests.swift`:

```swift
@Test("Approving an already granted gate is rejected and does not enqueue a new request")
func duplicateApproveRejected() async throws {
    let harness = try ApprovalHarness.makeResolvedGate()
    let request = try #require(harness.service.pendingApprovals.values.first)

    harness.service.resolveApproval(approvalID: request.id, granted: true)
    harness.service.resolveApproval(approvalID: request.id, granted: true)

    let approvals = harness.run.approvals.filter { $0.stageID == request.stageID }
    #expect(approvals.count == 1)
    #expect(approvals.first?.decision == .granted)
    #expect(harness.service.pendingApprovals.isEmpty)
}

@Test("Relaunch restores only requested approvals into pending inbox")
func relaunchRestoresOnlyRequestedApprovals() async throws {
    let harness = try ApprovalHarness.makeGrantedGateInStore()
    let service = ExecutionService(modelContext: harness.context, executor: SimulatedAgentExecutor())
    service.resumeInterruptedRuns(compiler: harness.compiler)
    #expect(service.pendingApprovals.isEmpty)
}
```

- [ ] **Step 2: Run the approval slice and verify it fails**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests'
```

Expected: FAIL until stale approval requests are filtered out by persisted approval truth.

- [ ] **Step 3: Implement fail-closed approval resolution**

Implement these rules:

```swift
// ExecutionService.resolveApproval(...)
guard let approval = run.approvals.first(where: { $0.id == approvalID }) ??
                     run.approvals.first(where: { $0.stageID == request.stageID && $0.decision == .requested }) else {
    pendingApprovals.removeValue(forKey: approvalID)
    return
}

guard approval.decision == .requested else {
    pendingApprovals.removeValue(forKey: approvalID)
    return
}
```

```swift
// WorkflowOrchestrator existingOrRestoredApproval(for:)
if let existing = run.approvals.first(where: { $0.stageID == state.id && $0.decision == .requested }) {
    return existing
}
```

```swift
// ApprovalGateView
.disabled(isResolving || request.isStaleResolvedGate)
```

Do not create a new approval row if a gate is already granted or rejected. Only `.requested` is pending truth.

- [ ] **Step 4: Re-run the approval slice**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests'
```

Expected: PASS for the new approval truth tests; the golden run may still fail later in the flow.

- [ ] **Step 5: Commit**

```bash
git add \
  'Chainworks Forge/Engine/ExecutionService.swift' \
  'Chainworks Forge/Engine/WorkflowOrchestrator.swift' \
  'Chainworks Forge/Models/Approval.swift' \
  'Chainworks Forge/Views/ApprovalGateView.swift' \
  'Chainworks ForgeTests/ResumeManagerTests.swift' \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift'
git commit -m 'fix: make approval resolution idempotent and stale-proof'
```

### Task 3: Make Proposal Review Contract Fail Closed Before Aggregation

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/OutputContractResolverV2.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/FixtureGooseTransport.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal013Tests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/GooseSessionBridgeTests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`

- [ ] **Step 1: Add failing review contract tests**

Add tests like this:

```swift
@Test("Proposal review markdown payload is rejected as contract violation")
func proposalReviewMarkdownRejected() {
    let catalog = makeTestCanonicalCatalog()
    let agent = makeProposalReviewAgent()
    let markdown = Data("# Proposal Review\\n\\nNot JSON".utf8)
    let results = OutputContractResolverV2.validateOutputs(
        ["proposal_review_architect": markdown],
        agent: agent,
        catalog: catalog
    )
    #expect(results["proposal_review_architect"]?.status == .failed)
}

@Test("Proposal review requires exact artifact basename without markdown extension")
func proposalReviewExactNameEnforced() {
    let packet = GooseSessionBridge.makePromptPacket(...)
    #expect(packet.contains(\"Write exactly one file named proposal_review_architect\"))
    #expect(packet.contains(\"Do not add .md, .txt, or .json to the filename\"))
}
```

- [ ] **Step 2: Run the review-contract slice and verify it fails**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/Proposal013Tests' \
  -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'
```

Expected: FAIL until markdown-only review outputs are no longer tolerated anywhere in the pipeline.

- [ ] **Step 3: Implement strict review quartet enforcement**

Required behavior:

```swift
// ProposalReviewContractAdapter
guard parsedTopLevelObject != nil else {
    throw OutputContractViolation.nonJSONProposalReview(outputName)
}
```

```swift
// OutputContractResolverV2
if contractID == "proposal_review_v1" {
    return .strictStructured
}
```

```swift
// GooseSessionBridge
instructions += """
Write exactly one top-level JSON object.
Write it to the exact artifact basename: \(outputName)
Do not add .md, .txt, .json, prose, headings, or tables.
Required fields: \(requiredFields.joined(separator: ", "))
"""
```

```swift
// WorkflowOrchestrator
// Reject before aggregation if any reviewer output fails contract validation.
```

- [ ] **Step 4: Re-run the contract slice**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/Proposal013Tests' \
  -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests'
```

Expected: PASS on the new contract tests; the golden run should now move past the quartet or fail with a single explicit contract violation reason.

- [ ] **Step 5: Commit**

```bash
git add \
  'Chainworks Forge/Engine/GooseSessionBridge.swift' \
  'Chainworks Forge/Engine/ProposalReviewContractAdapter.swift' \
  'Chainworks Forge/Engine/OutputContractResolverV2.swift' \
  'Chainworks Forge/Engine/WorkflowOrchestrator.swift' \
  'Chainworks Forge/Engine/FixtureGooseTransport.swift' \
  'Chainworks ForgeTests/Proposal013Tests.swift' \
  'Chainworks ForgeTests/GooseSessionBridgeTests.swift' \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift'
git commit -m 'fix: enforce strict proposal review contract before aggregation'
```

### Task 4: Make Declarative Workflow The Only Transition Authority

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/TransitionEvaluator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/OrchestratorTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/LiveProposalWorkflowTests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`

- [ ] **Step 1: Add failing transition-authority tests**

```swift
@Test("Agent-authored bogus next_stage is ignored when declarative transition exists")
func bogusNextStageIgnored() async throws {
    let harness = try WorkflowConflictHarness.makeConditionalApproveCase()
    harness.injectRunState(nextStage: "state_3_proposal_drafted")
    let transition = try #require(harness.orchestrator.resolveNextStateFromWorkflow())
    #expect(transition == "state_5_proposal_refined")
}

@Test("No declarative match produces explicit workflow conflict block")
func noDeclarativeMatchBlocksWithConflict() async throws {
    let harness = try WorkflowConflictHarness.makeNoMatchCase()
    await harness.orchestrator.advance()
    #expect(harness.run.status == .blocked)
    #expect(harness.run.driftDetails?.contains("workflow conflict") == true)
}
```

- [ ] **Step 2: Run the transition slice and verify it fails**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/OrchestratorTests' \
  -only-testing:'Chainworks ForgeTests/LiveProposalWorkflowTests'
```

Expected: FAIL until the engine ignores advisory `next_stage` and derives transitions only from the workflow graph.

- [ ] **Step 3: Implement declarative-only transition resolution**

Required behavior:

```swift
// WorkflowOrchestrator
let transition = TransitionEvaluator.evaluateFirst(
    transitions: state.transitions,
    context: context
)

guard let next = transition?.to else {
    run.status = .blocked
    run.driftDetails = "Workflow conflict: no declarative transition matched state \(state.id)"
    return
}
currentStateID = next
```

Never trust agent-written `next_stage` as authority. Treat it as advisory diagnostics only.

- [ ] **Step 4: Re-run the transition slice**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/OrchestratorTests' \
  -only-testing:'Chainworks ForgeTests/LiveProposalWorkflowTests' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests'
```

Expected: PASS on workflow-authority tests; the golden run should now fail only on whatever unresolved blocker remains downstream.

- [ ] **Step 5: Commit**

```bash
git add \
  'Chainworks Forge/Engine/WorkflowOrchestrator.swift' \
  'Chainworks Forge/Engine/TransitionEvaluator.swift' \
  'Chainworks ForgeTests/OrchestratorTests.swift' \
  'Chainworks ForgeTests/LiveProposalWorkflowTests.swift' \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift'
git commit -m 'fix: make declarative workflow transitions authoritative'
```

### Task 5: Align Persisted Read Models With Execution Truth

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowMapProjectionService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/WorkflowMapView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/WorkflowMapProjectionTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ResumeManagerTests.swift`

- [ ] **Step 1: Add failing read-model consistency tests**

```swift
@Test("Persisted blocked run never renders as running after relaunch")
func blockedRunDoesNotRenderAsRunning() async throws {
    let projection = try makeBlockedRunProjectionAfterRelaunch()
    #expect(projection.runStatusLabel == "blocked")
}

@Test("Persisted timeline fallback surfaces stage checkpoints when live stream is absent")
func persistedTimelineFallbackVisible() async throws {
    let projection = try makeProjectionWithoutLiveOrchestrator()
    #expect(projection.liveTimeline.isEmpty)
    #expect(!projection.persistedTimeline.isEmpty)
}
```

- [ ] **Step 2: Run the read-model slice and verify it fails**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests'
```

Expected: FAIL until report/workflow-map status derive from persisted terminal truth instead of stale in-memory attachment.

- [ ] **Step 3: Implement persisted-first read behavior**

Required behavior:

```swift
// RunReportBuilder / WorkflowMapProjectionService
// Preferred order:
// 1. persisted run.status / stage statuses
// 2. canonical outcome fields
// 3. live orchestrator attachment (only if present)
```

```swift
// WorkflowMapView / IdeaListView
// If live stream absent, show persisted checkpoint timeline and persisted status badge.
```

- [ ] **Step 4: Re-run the read-model slice**

Run:

```bash
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests' \
  -only-testing:'Chainworks ForgeTests/ResumeManagerTests' \
  -only-testing:'Chainworks ForgeTests/MVPGoldenRunTests'
```

Expected: PASS, with no stale `running`/empty-timeline contradictions after relaunch.

- [ ] **Step 5: Commit**

```bash
git add \
  'Chainworks Forge/Engine/RunReportBuilder.swift' \
  'Chainworks Forge/Engine/WorkflowMapProjectionService.swift' \
  'Chainworks Forge/Views/WorkflowMapView.swift' \
  'Chainworks Forge/Views/IdeaListView.swift' \
  'Chainworks ForgeTests/WorkflowMapProjectionTests.swift' \
  'Chainworks ForgeTests/ResumeManagerTests.swift' \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift'
git commit -m 'fix: align workflow map and reports to persisted execution truth'
```

### Task 6: Close The Loop With One Green Current-Head Run

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/MVPGoldenRunTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/FullMVPDeliveryTests.swift`

- [ ] **Step 1: Run the full proof lane**

Run:

```bash
./scripts/test-gate.sh mvp-golden-run
```

Expected: PASS.

- [ ] **Step 2: Add one app-level proof command for the exact lane**

Extend `test-gate.sh` with a second phase:

```bash
CHAINWORKS_MVP_GOLDEN_AUTORUN=1 \
CHAINWORKS_REPO_ROOT="$PWD" \
xcodebuild test \
  -project 'Chainworks Forge.xcodeproj' \
  -scheme 'Chainworks Forge' \
  -destination 'platform=macOS' \
  -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests/fullMVPGoldenAutorunProof'
```

- [ ] **Step 3: Run the app-level proof**

Run:

```bash
./scripts/test-gate.sh mvp-golden-run
```

Expected: PASS for both the non-UI slice and the app-level autorun proof.

- [ ] **Step 4: Verify end-state manually from artifacts only**

Check:

```bash
python3 - <<'PY'
import json, pathlib
latest = max(pathlib.Path('/Users/user/Library/Application Support/Chainworks Forge/runs').glob('*/artifacts/reports/run_report_v*.json'), key=lambda p: p.stat().st_mtime)
obj = json.loads(latest.read_text())
print(obj['runStatus'])
print(obj['currentStageId'])
print(obj.get('blockedReason'))
PY
```

Expected output:

```text
completed
state_6_workflow_complete
None
```

- [ ] **Step 5: Commit**

```bash
git add \
  'scripts/test-gate.sh' \
  'Chainworks ForgeTests/MVPGoldenRunTests.swift' \
  'Chainworks ForgeTests/FullMVPDeliveryTests.swift'
git commit -m 'test: close mvp golden run proof lane'
```

## Acceptance Gates

Do not declare the MVP stabilized until all of these are true on current head:

1. `./scripts/test-gate.sh mvp-golden-run` is green.
2. `MVPGoldenRunTests` reaches `state_6_workflow_complete`.
3. Repeating `Approve` on the same gate cannot create duplicate effective approvals.
4. `Proposal reviewed` cannot consume markdown quartet outputs.
5. Agent-authored `next_stage` cannot override workflow transitions.
6. Relaunch cannot show `running` while persisted truth is `blocked` or `completed`.

## What Not To Do

- Do not debug the next failing live run from screenshots first.
- Do not patch UI rendering before writing a failing persisted-truth test.
- Do not reopen proposal-scale work from `017` until this lane is green.
- Do not add new recovery actions while stale approval / stale status bugs remain.

## Self-Review

**Spec coverage:** This plan covers the actual failure classes already seen on current head: duplicate approval truth, markdown review outputs, bogus `next_stage`, stale read-model contradictions, and the missing single proving lane. It intentionally excludes broader lead-mediated workflow conflict design except where needed to fail closed.

**Placeholder scan:** No `TODO`, `TBD`, or “fix later” steps are used. Every task lists concrete files, exact commands, and the behavior being proved.

**Type consistency:** The plan consistently uses `Run.status`, `Approval.decision`, `pendingApprovals`, `proposal_review_v1`, `state_6_workflow_complete`, and the existing test suite names already present in the tree.

## Execution Handoff

Plan complete and saved to `/Users/user/Documents/Chainworks Forge/docs/superpowers/plans/2026-03-30-mvp-stabilization-to-first-green-run.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
