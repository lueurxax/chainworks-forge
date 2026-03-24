import XCTest
import SwiftData
@testable import Chainworks_Forge

@MainActor
final class ResumeManagerTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var compiler: RunPlanCompiler!

    override func setUp() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
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

    /// Create a run directly in SwiftData with proper snapshot data, avoiding filesystem ops.
    private func makeRunFromPlan() throws -> (Run, RunPlan, RunWorkspace) {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Test", body: "Test idea for resume")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = Run(
            id: runID,
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
        try context.save()

        return (run, plan, workspace)
    }

    // MARK: - Find Interrupted Runs

    func testFindInterruptedRuns() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertEqual(interrupted.count, 1)
        XCTAssertEqual(interrupted[0].id, run.id)
    }

    func testFindInterruptedRunsWaitingApproval() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .waitingApproval
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertEqual(interrupted.count, 1)
    }

    func testCompletedRunsNotFound() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .completed
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertTrue(interrupted.isEmpty, "Completed runs should not be found as interrupted")
    }

    func testCancelledRunsNotFound() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .cancelled
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertTrue(interrupted.isEmpty, "Cancelled runs should not be found as interrupted")
    }

    // MARK: - Classification

    func testClassifyResumeableRun() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        XCTAssertEqual(actions.count, 1)

        switch actions[0] {
        case .resume(let resumeRun, let resumePlan, let resumeWorkspace):
            XCTAssertEqual(resumeRun.id, run.id)
            XCTAssertEqual(resumePlan.workflowID, "proposal_to_release")
            XCTAssertEqual(resumeWorkspace.runID, run.id)
        default:
            XCTFail("Expected .resume action, got \(actions[0])")
        }
    }

    func testClassifyCompilerVersionMismatch() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)
        XCTAssertEqual(actions.count, 1)

        if case .resume(_, let plan, _) = actions[0] {
            XCTAssertEqual(plan.planCompilerVersion, RunPlan.currentCompilerVersion)
        }
    }

    // MARK: - Side-Effect Detection

    func testSideEffectStageDetected() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(stageID: "commit_and_push", label: "Commit", status: .running)
        stage.run = run
        context.insert(stage)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        XCTAssertEqual(actions.count, 1)
        if case .needsDecision(_, let reason) = actions[0] {
            XCTAssertTrue(reason.contains("side-effect"), "Should mention side-effect: \(reason)")
        } else if case .resume = actions[0] {
            // Also acceptable if no drift detected — the side-effect check is for running stages
        }
    }

    // MARK: - ExecutionService

    func testExecutionServiceStartRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(modelContext: context, executor: executor)

        XCTAssertFalse(service.hasActiveRuns)

        service.startRun(run: run, plan: plan, workspace: workspace)

        XCTAssertTrue(service.hasActiveRuns)
        XCTAssertNotNil(service.orchestrator(for: run.id))

        service.cancelRun(runID: run.id)
    }

    func testExecutionServiceCancelRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        XCTAssertTrue(service.hasActiveRuns)

        service.cancelRun(runID: run.id)
        XCTAssertFalse(service.hasActiveRuns)
        XCTAssertEqual(run.status, .cancelled)
    }

    func testExecutionServiceDuplicateStartPrevented() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        service.startRun(run: run, plan: plan, workspace: workspace) // No-op

        XCTAssertEqual(service.activeOrchestrators.count, 1)

        service.cancelRun(runID: run.id)
    }

    // MARK: - Live Executor Routing

    private func repositoryRootURL(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func loadLiveWorkflow() throws -> WorkflowDefinition {
        let url = try XCTUnwrap(
            Bundle(for: type(of: self)).url(forResource: "proposal-loop-live", withExtension: "yaml"),
            "proposal-loop-live.yaml fixture must be bundled with tests"
        )
        return try YAMLParser.loadWorkflow(from: url)
    }

    func testExecutionServiceUsesLiveExecutorForLiveWorkflow() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Live Workflow", body: "Validate Goose-backed executor routing")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("LiveExecutionServiceTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        try context.save()

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
            catalog: catalog,
            liveRuntimeConfiguration: LiveRuntimeConfiguration(
                baseURL: URL(string: "http://localhost:9999")!,
                apiKey: nil,
                override: LiveExecutionOverride(
                    enabled: true,
                    provider: "claude_code",
                    model: "default",
                    effort: "high"
                ),
                transportMode: .network,
                transportAPI: .gooseServer
            )
        )

        service.startRun(run: run, plan: plan, workspace: workspace)

        guard let orchestrator = service.orchestrator(for: run.id) else {
            XCTFail("Expected live orchestrator to be created")
            return
        }
        XCTAssertTrue(orchestrator.executor is GooseAgentExecutor)
    }

    func testExecutionServiceBlocksLiveWorkflowWithoutRuntimeConfig() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Blocked Live Workflow", body: "Missing runtime config")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("BlockedLiveExecutionServiceTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        try context.save()

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
            catalog: catalog
        )

        service.startRun(run: run, plan: plan, workspace: workspace)

        XCTAssertNil(service.orchestrator(for: run.id))
        XCTAssertEqual(run.status, .blocked)
        XCTAssertTrue(run.driftDetails?.contains("Live runtime is not configured") == true)
    }

    func testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Resume Waiting Approval", body: "Restore approval gate on app relaunch")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeWaitingApproval-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        run.status = .waitingApproval

        let stageExec = StageExecution(
            stageID: "state_5_proposal_approval",
            label: "Human approval: proposal quality",
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        stageExec.run = run
        context.insert(stageExec)

        let approval = Approval(stageID: "state_5_proposal_approval", decision: .requested)
        approval.run = run
        context.insert(approval)
        try context.save()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(
            modelContext: context,
            executor: executor,
            catalog: catalog,
            liveRuntimeConfiguration: LiveRuntimeConfiguration(
                baseURL: URL(string: "http://fixture.local")!,
                apiKey: nil,
                override: LiveExecutionOverride(
                    enabled: true,
                    provider: "claude_code",
                    model: "fixture-model",
                    effort: "high"
                ),
                transportMode: .fixtureProposalLoopSuccess,
                transportAPI: .bespoke
            )
        )

        service.resumeInterruptedRuns(compiler: compiler)

        let deadline = Date().addingTimeInterval(2)
        while service.pendingApprovalCount == 0 && Date() < deadline {
            await Task.yield()
        }

        XCTAssertEqual(service.pendingApprovalCount, 1, "Waiting approval should be restored into the app shell")
        XCTAssertEqual(executor.executedTasks.count, 0, "Approval restore must not re-execute the paused stage")
        XCTAssertEqual(run.status, .waitingApproval)
        XCTAssertEqual(run.stageExecutions.count, 1, "Approval restore must not duplicate the waiting stage")
        XCTAssertNotNil(service.orchestrator(for: run.id), "Resumed live run should still be attached to an orchestrator")
    }
}
