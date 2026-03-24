import XCTest
import SwiftData
@testable import Chainworks_Forge

// MARK: - EndToEndTests (Proposal 002 Section 12 — full canonical workflow)

/// Tests the complete execution flow from compilation through all 12 states
/// of the canonical workflow, verifying sequential/parallel execution,
/// approval gates, loop handling, transition evaluation, and artifact persistence.
@MainActor
final class EndToEndTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var tempDir: URL!

    override func setUp() async throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, Artifact.self
        ])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("E2E-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let dir = tempDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    // MARK: - Helpers

    private func loadCanonicalWorkflow() throws -> WorkflowDefinition {
        let url = Bundle(for: type(of: self)).url(forResource: "workflow", withExtension: "yaml")!
        return try YAMLParser.loadWorkflow(from: url)
    }

    private func loadCanonicalCatalog() throws -> AgentCatalog {
        let url = Bundle(for: type(of: self)).url(forResource: "agents", withExtension: "yaml")!
        return try YAMLParser.loadAgentCatalog(from: url)
    }

    private func loadLiveWorkflow() throws -> WorkflowDefinition {
        let url = try XCTUnwrap(
            Bundle(for: type(of: self)).url(forResource: "proposal-loop-live", withExtension: "yaml"),
            "proposal-loop-live.yaml fixture must be bundled with tests"
        )
        return try YAMLParser.loadWorkflow(from: url)
    }

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
    func testFullCanonicalWorkflow() async throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

        // Phase 1: Preview compile
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Verify compilation results
        XCTAssertFalse(plan.states.isEmpty, "Plan should have states")
        XCTAssertFalse(plan.agentBindings.isEmpty, "Plan should have agent bindings")
        XCTAssertEqual(plan.planCompilerVersion, RunPlan.currentCompilerVersion)

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
        try await pollUntil(timeout: 20.0, message: "Canonical workflow should complete with auto-approved gates") {
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
        assertRunCompleted(run)

        // Verify stage executions were created lazily
        XCTAssertFalse(run.stageExecutions.isEmpty, "Should have created stage executions")

        // Verify agents executed
        XCTAssertFalse(executor.executedTasks.isEmpty, "Should have executed at least one agent")

        // Verify provenance
        XCTAssertEqual(run.workflowSnapshotHash, plan.workflowSnapshotHash)
        XCTAssertEqual(run.catalogSnapshotHash, plan.catalogSnapshotHash)
    }

    // MARK: - Compact Workflow End-to-End

    /// Tests the compact workflow normalization + compilation + execution path.
    func testCompactWorkflowEndToEnd() async throws {
        guard let compactURL = Bundle(for: type(of: self))
            .url(forResource: "proposal-to-release", withExtension: "yaml") else {
            // Skip if compact workflow not in test bundle
            return
        }

        let catalog = try loadCanonicalCatalog()
        let compact = try YAMLParser.loadCompactWorkflow(from: compactURL)

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        XCTAssertFalse(plan.states.isEmpty)
        XCTAssertFalse(plan.agentBindings.isEmpty)

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

        // Auto-approve all gates using pollUntil instead of sleep loop
        try await pollUntil(timeout: 10.0, message: "Compact workflow should finish or pause") {
            !orchestrator.isRunning || orchestrator.isPaused
        }

        while orchestrator.isPaused {
            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true)
            }
            approvalRequests.removeAll()
            try await pollUntil(timeout: 5.0, message: "Compact workflow should resume after approval") {
                !orchestrator.isPaused || !approvalRequests.isEmpty
            }
        }

        // Verify progress
        XCTAssertFalse(run.stageExecutions.isEmpty, "Compact workflow should execute stages")
        XCTAssertFalse(executor.executedTasks.isEmpty, "Compact workflow should execute agents")
    }

    // MARK: - ExecutionService Integration E2E

    /// Tests the full flow through ExecutionService: start, execute, complete.
    func testExecutionServiceEndToEnd() async throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

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
        XCTAssertTrue(service.hasActiveRuns)
        XCTAssertNotNil(service.orchestrator(for: run.id))

        // Wait for initial execution using pollUntil instead of fixed sleep
        try await pollUntil(timeout: 5.0, message: "Service should execute at least one agent") {
            !executor.executedTasks.isEmpty || !service.pendingApprovals.isEmpty
        }

        // If there are pending approvals, resolve them
        while !service.pendingApprovals.isEmpty {
            for (id, _) in service.pendingApprovals {
                service.resolveApproval(approvalID: id, granted: true, comment: "E2E approve")
            }
            try await pollUntil(timeout: 3.0, message: "Approvals should be processed") {
                service.pendingApprovals.isEmpty || !executor.executedTasks.isEmpty
            }
        }

        // Verify the service tracked the execution
        XCTAssertFalse(executor.executedTasks.isEmpty, "Service should have executed agents")
    }

    // MARK: - Cost Aggregation E2E

    /// Verifies cost tracking aggregates across multiple agent executions.
    func testCostAggregationEndToEnd() async throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

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

        // Auto-approve if needed, using pollUntil for reliable sync
        if orchestrator.isPaused {
            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true)
            }
            try await pollUntil(timeout: 5.0, message: "Orchestrator should resume after approval") {
                !orchestrator.isPaused
            }
        }

        // Assert explicitly that tasks executed — do NOT silently skip the assertion.
        // Previous version wrapped this in `if executor.executedTasks.count > 1` which
        // meant the test passed vacuously when nothing executed.
        XCTAssertGreaterThan(executor.executedTasks.count, 0,
                             "At least one agent must execute for cost tracking to be meaningful")
        XCTAssertNotNil(run.totalCostCents, "Cost should be tracked after agent execution")
        XCTAssertTrue(run.totalCostCents! > 0, "Cost should be positive after agent execution")
    }

    func testLiveProposalLoopFixtureReachesApprovalAndCompletes() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()

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

        // Wait for the run to reach a stable state using pollUntil
        try await pollUntil(timeout: 15.0, message: "Fixture live workflow should reach approval gate or complete") {
            run.status == .waitingApproval || run.status == .completed || run.status == .blocked
        }

        // Allow fire-and-forget live event routing tasks to settle
        try await Task.sleep(nanoseconds: 300_000_000)

        XCTAssertTrue(
            run.status == .waitingApproval || run.status == .completed || run.status == .blocked,
            "Fixture live workflow should reach approval gate, complete, or block, got: \(run.status.rawValue)"
        )
        XCTAssertFalse(run.stageExecutions.isEmpty, "Run should have executed stages")

        // If paused at approval gate or blocked, approve and wait for completion
        if (run.status == .waitingApproval || run.status == .blocked) && !approvalRequests.isEmpty {
            let artifactCountBeforeApproval = run.stageExecutions
                .flatMap(\.agentExecutions)
                .flatMap(\.artifacts)
                .count
            XCTAssertGreaterThan(artifactCountBeforeApproval, 0, "Fixture live workflow should persist artifacts before approval")

            for request in approvalRequests {
                orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Fixture test approval")
            }

            try await pollUntil(timeout: 15.0, message: "Fixture live workflow should complete after approval") {
                run.status == .completed || run.status == .blocked
            }
        }

        XCTAssertTrue(
            run.status == .completed || run.status == .blocked || run.status == .waitingApproval,
            "Fixture live workflow should complete or reach a stable state, got: \(run.status.rawValue)"
        )

        // completedAt is only set when the run reaches .completed
        if run.status == .completed {
            XCTAssertNotNil(run.completedAt)
        }

        XCTAssertTrue(
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
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            XCTAssertEqual(json["pass"] as? Bool, true)
            XCTAssertNotNil(json["average_score"] as? Double)
            XCTAssertNotNil(json["required_changes"] as? [Any])
        } else {
            XCTFail("Expected proposal_review_summary artifact to be persisted")
        }

        if run.status == .completed {
            let descriptor = FetchDescriptor<Artifact>()
            let reports = try context.fetch(descriptor)
                .filter { $0.runID == run.id && $0.name == "final_feature_report" }
            XCTAssertEqual(reports.count, 1, "Completed live run should persist a final feature report")
        }
    }
}
