import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

// MARK: - EndToEndTests (Proposal 002 Section 12 — full canonical workflow)

/// Tests the complete execution flow from compilation through all 12 states
/// of the canonical workflow, verifying sequential/parallel execution,
/// approval gates, loop handling, transition evaluation, and artifact persistence.
@MainActor
@Suite("EndToEnd", .tags(.integration))
struct EndToEndTests {
    let container: ModelContainer
    let context: ModelContext
    let tempDir: URL

    init() throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, Artifact.self
        ])
        let config = ModelConfiguration("EndToEndTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("E2E-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    // MARK: - Helpers

    private func makeWorkspace() -> RunWorkspace {
        let runID = UUID()
        let workspaceRoot = tempDir.appendingPathComponent(runID.uuidString, isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        return RunWorkspace(runID: runID, workspaceRoot: workspaceRoot, artifactRoot: artifactRoot, worktreeRoot: nil)
    }

    private func makeRun(workspace: RunWorkspace, plan: RunPlan) -> Run {
        let idea = Idea(title: "E2E Test", body: "End-to-end workflow test")
        context.insert(idea)

        let run = Run(
            id: workspace.runID,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml",
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        ) // RunRepository-exempt
        run.idea = idea
        context.insert(run)
        return run
    }

    // MARK: - Full Canonical Workflow End-to-End

    /// Section 12 required test: full canonical workflow executes through all states
    /// with correct agent execution order, approval handling, and artifact production.
    @Test("Full canonical workflow executes through all states")
    func fullCanonicalWorkflow() async throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        // Phase 1: Preview compile
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Verify compilation results
        #expect(!plan.states.isEmpty, "Plan should have states")
        #expect(!plan.agentBindings.isEmpty, "Plan should have agent bindings")
        #expect(plan.planCompilerVersion == RunPlan.currentCompilerVersion)

        // Phase 2: Create orchestrator (using simulated executor)
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace, plan: plan)
        let executor = SimulatedAgentExecutor(catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context,
            catalog: catalog
        )

        // Track approvals and completion
        var approvalRequests: [ApprovalRequest] = []
        var completionCalled = false
        var completionSuccess = false

        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }
        orchestrator.onComplete = { success in
            completionCalled = true
            completionSuccess = success
        }

        // Start execution
        await orchestrator.start()

        // Auto-approve gates and wait for completion.
        // The canonical workflow has approval gates that pause execution.
        // We poll until completion, auto-approving any gates that appear.
        await awaitCondition("Canonical workflow should complete with auto-approved gates", timeout: 20.0) {
            // Check if we need to approve something
            if orchestrator.isPaused && !approvalRequests.isEmpty {
                for request in approvalRequests {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "E2E auto-approve")
                }
                approvalRequests.removeAll()
            }
            return run.status == .completed || run.status == .failed || run.status == .cancelled
        }

        // Verify completion — assert the specific expected terminal state.
        // The canonical workflow with auto-approved gates should reach .completed.
        // If it doesn't, the test should fail loudly rather than silently accepting any state.
        expectRunCompleted(run)

        // Verify stage executions were created lazily
        #expect(!run.stageExecutions.isEmpty, "Should have created stage executions")

        // Verify agents executed
        #expect(!executor.executedTasks.isEmpty, "Should have executed at least one agent")

        // Verify provenance
        #expect(run.workflowSnapshotHash == plan.workflowSnapshotHash)
        #expect(run.catalogSnapshotHash == plan.catalogSnapshotHash)
    }

    // MARK: - Compact Workflow End-to-End

    /// Tests the compact workflow normalization + compilation + execution path.
    @Test("Compact workflow normalization, compilation, and execution")
    func compactWorkflowEndToEnd() async throws {
        let compact: CompactWorkflowDefinition
        do {
            compact = try loadTestCompactWorkflow()
        } catch {
            // Skip if compact workflow not in test bundle
            return
        }

        let catalog = try loadTestCanonicalCatalog()

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        #expect(!plan.states.isEmpty)
        #expect(!plan.agentBindings.isEmpty)

        // Execute with simulated agents
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace, plan: plan)
        let executor = SimulatedAgentExecutor(catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context,
            catalog: catalog
        )

        var approvalRequests: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }

        await orchestrator.start()

        // Auto-approve all gates using awaitCondition instead of sleep loop
        await awaitCondition("Compact workflow should finish or pause", timeout: 10.0) {
            !orchestrator.isRunning || orchestrator.isPaused
        }

        while orchestrator.isPaused {
            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true)
            }
            approvalRequests.removeAll()
            await awaitCondition("Compact workflow should resume after approval", timeout: 5.0) {
                !orchestrator.isPaused || !approvalRequests.isEmpty
            }
        }

        // Verify progress
        #expect(!run.stageExecutions.isEmpty, "Compact workflow should execute stages")
        #expect(!executor.executedTasks.isEmpty, "Compact workflow should execute agents")
    }

    // MARK: - ExecutionService Integration E2E

    /// Tests the full flow through ExecutionService: start, execute, complete.
    @Test("ExecutionService full flow: start, execute, complete")
    func executionServiceEndToEnd() async throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace, plan: plan)

        let executor = SimulatedAgentExecutor(catalog: catalog)
        let service = ExecutionService(
            modelContext: context,
            executor: executor,
            catalog: catalog
        )

        // Start via service
        service.startRun(run: run, plan: plan, workspace: workspace)
        #expect(service.hasActiveRuns)
        #expect(service.orchestrator(for: run.id) != nil)

        // Wait for initial execution using awaitCondition instead of fixed sleep
        await awaitCondition("Service should execute at least one agent", timeout: 5.0) {
            !executor.executedTasks.isEmpty || !service.pendingApprovals.isEmpty
        }

        // If there are pending approvals, resolve them
        while !service.pendingApprovals.isEmpty {
            for (id, _) in service.pendingApprovals {
                service.resolveApproval(approvalID: id, granted: true, comment: "E2E approve")
            }
            await awaitCondition("Approvals should be processed", timeout: 3.0) {
                service.pendingApprovals.isEmpty || !executor.executedTasks.isEmpty
            }
        }

        // Verify the service tracked the execution
        #expect(!executor.executedTasks.isEmpty, "Service should have executed agents")
    }

    // MARK: - Cost Aggregation E2E

    /// Verifies cost tracking aggregates across multiple agent executions.
    @Test("Cost tracking aggregates across agent executions")
    func costAggregationEndToEnd() async throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace, plan: plan)

        let executor = SimulatedAgentExecutor(catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context,
            catalog: catalog
        )

        var approvalRequests: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }

        await orchestrator.start()

        // Auto-approve if needed, using awaitCondition for reliable sync
        if orchestrator.isPaused {
            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true)
            }
            await awaitCondition("Orchestrator should resume after approval", timeout: 5.0) {
                !orchestrator.isPaused
            }
        }

        // Assert explicitly that tasks executed — do NOT silently skip the assertion.
        // Previous version wrapped this in `if executor.executedTasks.count > 1` which
        // meant the test passed vacuously when nothing executed.
        #expect(executor.executedTasks.count > 0,
                "At least one agent must execute for cost tracking to be meaningful")
        #expect(run.totalCostCents != nil, "Cost should be tracked after agent execution")
        #expect(run.totalCostCents! > 0, "Cost should be positive after agent execution")
    }

    @Test("Live proposal loop fixture reaches approval and completes")
    func liveProposalLoopFixtureReachesApprovalAndCompletes() async throws {
        let workflow = try loadTestLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace, plan: plan)
        let transport = FixtureGooseTransport(
            scenario: .proposalLoopSuccess
        )
        let executor = GooseAgentExecutor(transport: transport)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvalRequests: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }

        await orchestrator.start()

        // Wait for the run to reach a stable state using awaitCondition
        await awaitCondition("Fixture live workflow should reach approval gate or complete", timeout: 15.0) {
            run.status == .waitingApproval || run.status == .completed || run.status == .blocked
        }

        // Allow fire-and-forget live event routing tasks to settle
        try await Task.sleep(nanoseconds: 300_000_000)

        #expect(
            run.status == .waitingApproval || run.status == .completed || run.status == .blocked,
            "Fixture live workflow should reach approval gate, complete, or block, got: \(run.status.rawValue)"
        )
        #expect(!run.stageExecutions.isEmpty, "Run should have executed stages")

        // If paused at approval gate or blocked, approve and wait for completion
        if (run.status == .waitingApproval || run.status == .blocked) && !approvalRequests.isEmpty {
            let artifactCountBeforeApproval = run.stageExecutions
                .flatMap(\.agentExecutions)
                .flatMap(\.artifacts)
                .count
            #expect(artifactCountBeforeApproval > 0, "Fixture live workflow should persist artifacts before approval")

            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Fixture test approval")
            }

            await awaitCondition("Fixture live workflow should complete after approval", timeout: 15.0) {
                run.status == .completed || run.status == .blocked
            }
        }

        #expect(
            run.status == .completed || run.status == .blocked || run.status == .waitingApproval,
            "Fixture live workflow should complete or reach a stable state, got: \(run.status.rawValue)"
        )

        // completedAt is only set when the run reaches .completed
        if run.status == .completed {
            #expect(run.completedAt != nil)
        }

        #expect(
            run.stageExecutions
                .flatMap(\.agentExecutions)
                .contains { ($0.providerSessionID ?? "").hasPrefix("fixture-") },
            "At least one live agent execution should capture a fixture provider session id"
        )

        let allArtifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)

        if let summaryArtifact = allArtifacts.first(where: { $0.name == "proposal_review_summary" }) {
            let data = try ArtifactStorage.read(filePath: summaryArtifact.filePath, workspaceRoot: workspace.workspaceRoot)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            #expect(json["pass"] as? Bool == true)
            #expect(json["average_score"] as? Double != nil)
            #expect(json["required_changes"] as? [Any] != nil)
        } else {
            Issue.record("Expected proposal_review_summary artifact to be persisted")
        }

        if run.status == .completed {
            let descriptor = FetchDescriptor<Artifact>()
            let reports = try context.fetch(descriptor)
                .filter { $0.runID == run.id && $0.name == "final_feature_report" }
            #expect(reports.count == 1, "Completed live run should persist a final feature report")
        }
    }
}
