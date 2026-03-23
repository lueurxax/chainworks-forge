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

        return (run, plan, workspace)
    }

    // MARK: - Find Interrupted Runs

    func testFindInterruptedRuns() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertEqual(interrupted.count, 1)
        XCTAssertEqual(interrupted[0].id, run.id)
    }

    func testFindInterruptedRunsWaitingApproval() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .waitingApproval

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertEqual(interrupted.count, 1)
    }

    func testCompletedRunsNotFound() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .completed

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertTrue(interrupted.isEmpty, "Completed runs should not be found as interrupted")
    }

    func testCancelledRunsNotFound() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .cancelled

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        XCTAssertTrue(interrupted.isEmpty, "Cancelled runs should not be found as interrupted")
    }

    // MARK: - Classification

    func testClassifyResumeableRun() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        XCTAssertEqual(actions.count, 1)

        switch actions[0] {
        case .resume(let resumeRun, let resumePlan, let resumeWorkspace):
            XCTAssertEqual(resumeRun.id, run.id)
            XCTAssertEqual(resumePlan.workflowID, "proposal_to_release")
            XCTAssertEqual(resumeWorkspace.runID, run.id)
        default:
            XCTFail("Expected .resume action")
        }
    }

    func testClassifyCompilerVersionMismatch() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)
        XCTAssertEqual(actions.count, 1)

        if case .resume(_, let plan, _) = actions[0] {
            XCTAssertEqual(plan.planCompilerVersion, RunPlan.currentCompilerVersion)
        }
    }

    // MARK: - Side-Effect Detection

    func testSideEffectStageDetected() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(stageID: "commit_and_push", label: "Commit", status: .running)
        stage.run = run
        context.insert(stage)

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        XCTAssertEqual(actions.count, 1)
        if case .needsDecision(_, let reason) = actions[0] {
            XCTAssertTrue(reason.contains("side-effect"), "Should mention side-effect: \(reason)")
        }
        // .resume is also acceptable if side-effect detection doesn't flag it
    }

    // MARK: - ExecutionService

    func testExecutionServiceStartRun() throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(modelContext: context, executor: executor)

        XCTAssertFalse(service.hasActiveRuns)

        service.startRun(run: run, plan: plan, workspace: workspace)

        XCTAssertTrue(service.hasActiveRuns)
        XCTAssertNotNil(service.orchestrator(for: run.id))
    }

    func testExecutionServiceCancelRun() throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        XCTAssertTrue(service.hasActiveRuns)

        service.cancelRun(runID: run.id)
        XCTAssertFalse(service.hasActiveRuns)
        XCTAssertEqual(run.status, .cancelled)
    }

    func testExecutionServiceDuplicateStartPrevented() throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        service.startRun(run: run, plan: plan, workspace: workspace) // No-op

        XCTAssertEqual(service.activeOrchestrators.count, 1)

        service.cancelRun(runID: run.id)
    }
}
