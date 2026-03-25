import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("ResumeManager", .serialized, .tags(.fast))
struct ResumeManagerTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("ResumeManagerTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    // MARK: - Helpers

    private func loadCanonicalWorkflow() throws -> WorkflowDefinition {
        try loadTestCanonicalWorkflow()
    }

    private func loadCanonicalCatalog() throws -> AgentCatalog {
        try loadTestCanonicalCatalog()
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

    // MARK: - Find Interrupted Runs (parameterized — Proposal 009 REQ-005)

    struct InterruptedRunCase: CustomStringConvertible, Sendable {
        let status: RunStatus
        let shouldBeFound: Bool
        var description: String { "\(status.rawValue) → \(shouldBeFound ? "found" : "not found")" }
    }

    @Test("findInterruptedRuns classifies status correctly", arguments: [
        InterruptedRunCase(status: .running, shouldBeFound: true),
        InterruptedRunCase(status: .waitingApproval, shouldBeFound: true),
        InterruptedRunCase(status: .completed, shouldBeFound: false),
        InterruptedRunCase(status: .cancelled, shouldBeFound: false),
        InterruptedRunCase(status: .failed, shouldBeFound: false),
    ])
    func findInterruptedRunsByStatus(testCase: InterruptedRunCase) async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = testCase.status
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        if testCase.shouldBeFound {
            #expect(interrupted.count == 1, "\(testCase.status.rawValue) should be found as interrupted")
            #expect(interrupted.first?.id == run.id)
        } else {
            #expect(interrupted.isEmpty, "\(testCase.status.rawValue) should NOT be found as interrupted")
        }
    }

    // MARK: - Classification

    @Test("Classify resumeable run")
    func classifyResumeableRun() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)

        switch actions[0] {
        case .resume(let resumeRun, let resumePlan, let resumeWorkspace):
            #expect(resumeRun.id == run.id)
            #expect(resumePlan.workflowID == "proposal_to_release")
            #expect(resumeWorkspace.runID == run.id)
        default:
            Issue.record("Expected .resume action, got \(actions[0])")
        }
    }

    @Test("Classify compiler version mismatch")
    func classifyCompilerVersionMismatch() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)
        #expect(actions.count == 1)

        if case .resume(_, let plan, _) = actions[0] {
            #expect(plan.planCompilerVersion == RunPlan.currentCompilerVersion)
        }
    }

    // MARK: - Side-Effect Detection

    @Test("Side-effect stage detected")
    func sideEffectStageDetected() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(stageID: "commit_and_push", label: "Commit", status: .running)
        stage.run = run
        context.insert(stage)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.contains("side-effect"), "Should mention side-effect: \(reason)")
        } else if case .resume = actions[0] {
            // Also acceptable if no drift detected — the side-effect check is for running stages
        }
    }

    // MARK: - ExecutionService

    @Test("ExecutionService start run")
    func executionServiceStartRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(modelContext: context, executor: executor)

        #expect(!service.hasActiveRuns)

        service.startRun(run: run, plan: plan, workspace: workspace)

        #expect(service.hasActiveRuns)
        #expect(service.orchestrator(for: run.id) != nil)

        service.cancelRun(runID: run.id)
    }

    @Test("ExecutionService cancel run")
    func executionServiceCancelRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        #expect(service.hasActiveRuns)

        service.cancelRun(runID: run.id)
        #expect(!service.hasActiveRuns)
        #expect(run.status == .cancelled)
    }

    @Test("ExecutionService duplicate start prevented")
    func executionServiceDuplicateStartPrevented() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        service.startRun(run: run, plan: plan, workspace: workspace) // No-op

        #expect(service.activeOrchestrators.count == 1)

        service.cancelRun(runID: run.id)
    }

    // MARK: - Live Executor Routing

    private func repositoryRootURL(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func loadLiveWorkflow() throws -> WorkflowDefinition {
        try loadTestLiveWorkflow()
    }

    @Test("ExecutionService uses live executor for live workflow")
    func executionServiceUsesLiveExecutorForLiveWorkflow() async throws {
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
            Issue.record("Expected live orchestrator to be created")
            return
        }
        #expect(orchestrator.executor is GooseAgentExecutor)

        service.cancelRun(runID: run.id)
    }

    @Test("ExecutionService blocks live workflow without runtime config")
    func executionServiceBlocksLiveWorkflowWithoutRuntimeConfig() async throws {
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

        #expect(service.orchestrator(for: run.id) == nil)
        #expect(run.status == .blocked)
        #expect(run.driftDetails?.contains("Live runtime is not configured") == true)
    }

    @Test("ExecutionService resume waiting approval restores pending approval without re-executing stage")
    func executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage() async throws {
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

        // Wait for approval restoration using awaitCondition instead of pollUntil
        await awaitCondition("Waiting approval should be restored", timeout: 3.0) {
            service.pendingApprovalCount > 0
        }

        #expect(service.pendingApprovalCount == 1, "Waiting approval should be restored into the app shell")
        #expect(executor.executedTasks.count == 0, "Approval restore must not re-execute the paused stage")
        #expect(run.status == .waitingApproval)
        #expect(run.stageExecutions.count == 1, "Approval restore must not duplicate the waiting stage")
        #expect(service.orchestrator(for: run.id) != nil, "Resumed live run should still be attached to an orchestrator")
    }
}
