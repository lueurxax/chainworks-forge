import XCTest
import SwiftData
@testable import Chainworks_Forge

@MainActor
final class OrchestratorTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var tempDir: URL!

    override func setUp() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("OrchestratorTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let dir = tempDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    // MARK: - Helpers

    private func makeWorkspace() -> RunWorkspace {
        let runID = UUID()
        let workspaceRoot = tempDir.appendingPathComponent(runID.uuidString, isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        return RunWorkspace(runID: runID, workspaceRoot: workspaceRoot, artifactRoot: artifactRoot, worktreeRoot: nil)
    }

    private func makeRun(workspace: RunWorkspace) -> Run {
        let idea = Idea(title: "Test Idea", body: "Test body")
        context.insert(idea)

        let run = Run(
            id: workspace.runID,
            workflowID: "test_wf",
            workflowTitle: "Test Workflow",
            workflowSnapshotHash: "abc123",
            catalogSnapshotHash: "def456",
            workflowSourcePath: "test.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: 1
        ) // RunRepository-exempt
        run.idea = idea
        context.insert(run)
        return run
    }

    private func makeAgent(id: String = "agent_1", outputs: [String] = ["output_1"]) -> ResolvedAgent {
        ResolvedAgent(
            id: id, title: "Agent \(id)", mode: "tool_use",
            provider: "claude_code", model: "opus", effort: "high",
            maxTurns: 10, temperature: 0.0, permissionProfile: "ORCH",
            skillRef: "sk1", skillRole: nil, prompt: "test",
            outputContract: nil, requiresHumanApproval: false,
            inputs: [], outputs: outputs
        )
    }

    // MARK: - Simple Linear Workflow

    func testSimpleLinearWorkflow() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent()

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "agent_1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "agent_1", task: "do_work", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "agent_1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["agent_1": agent],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        var completed = false
        orchestrator.onComplete = { success in
            completed = true
            XCTAssertTrue(success)
        }

        await orchestrator.start()

        XCTAssertTrue(completed)
        XCTAssertEqual(run.status, .completed)
        XCTAssertNotNil(run.completedAt)
        XCTAssertEqual(executor.executedTasks.count, 1)
        XCTAssertFalse(run.stageExecutions.isEmpty)
    }

    // MARK: - Multi-State Workflow

    func testMultiStateWorkflow() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "s1": ExecutableState(
                    id: "s1", label: "Stage 1", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "t1", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "s2", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "s2": ExecutableState(
                    id: "s2", label: "Stage 2", type: nil,
                    ownerAgentID: "a2",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a2", task: "t2", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "s3", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "s3": ExecutableState(
                    id: "s3", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "s1",
            agentBindings: [
                "a1": makeAgent(id: "a1"),
                "a2": makeAgent(id: "a2")
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .completed)
        XCTAssertEqual(executor.executedTasks.count, 2)
        XCTAssertEqual(executor.executedTasks[0].agentID, "a1")
        XCTAssertEqual(executor.executedTasks[1].agentID, "a2")
    }

    // MARK: - Parallel Execution

    func testParallelExecution() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "a1", task: "review1", inputs: nil, outputs: nil),
                            AgentTask(agent: "a2", task: "review2", inputs: nil, outputs: nil),
                            AgentTask(agent: "a3", task: "review3", inputs: nil, outputs: nil)
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "a1": makeAgent(id: "a1"),
                "a2": makeAgent(id: "a2"),
                "a3": makeAgent(id: "a3")
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .completed)
        XCTAssertEqual(executor.executedTasks.count, 3)
        let executedAgentIDs = Set(executor.executedTasks.map(\.agentID))
        XCTAssertEqual(executedAgentIDs, Set(["a1", "a2", "a3"]))
    }

    // MARK: - Approval Gate

    func testApprovalGatePausesExecution() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Approval Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .approvalGranted)],
                    approvalRequired: true, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1")],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        var receivedApprovalRequest: ApprovalRequest?
        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )
        orchestrator.onApprovalRequest = { request in
            receivedApprovalRequest = request
        }

        await orchestrator.start()

        // Should be paused waiting for approval
        XCTAssertEqual(run.status, .waitingApproval)
        XCTAssertTrue(orchestrator.isPaused)
        XCTAssertNotNil(receivedApprovalRequest)
        XCTAssertEqual(receivedApprovalRequest?.stageID, "start")
    }

    func testApprovalGrantedResumesExecution() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .approvalGranted)],
                    approvalRequired: true, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1")],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()
        XCTAssertEqual(run.status, .waitingApproval)

        // Grant approval — this triggers resume
        orchestrator.resolveApproval(stageID: "start", granted: true, comment: "Approved")

        // Wait for resume to complete
        try? await Task.sleep(nanoseconds: 100_000_000) // 100ms

        XCTAssertEqual(run.status, .completed)
    }

    // MARK: - Agent Failure

    func testAgentFailurePausesRun() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "fail", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1")],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(onError: "pause_and_require_human", onLoopBudgetExhausted: "pause_and_require_human", preserveArtifacts: true),
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["a1"]

        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .blocked)
    }

    // MARK: - Cancellation

    func testCancellation() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "long_task", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1")],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor(simulatedDelay: 2.0) // Long-running
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        // Start and then cancel
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 50_000_000) // 50ms
            orchestrator.cancel()
        }

        await orchestrator.start()

        XCTAssertEqual(run.status, .cancelled)
    }

    // MARK: - Transition Conditions

    func testArtifactExistsTransition() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "produce", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [
                        ExecutableTransition(to: "middle", condition: .artifactExists("output_1"))
                    ],
                    approvalRequired: false, loop: nil
                ),
                "middle": ExecutableState(
                    id: "middle", label: "Middle", type: nil,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "consume", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1", outputs: ["output_1"])],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .completed)
        XCTAssertEqual(executor.executedTasks.count, 2)
    }

    // MARK: - Lazy Stage Creation (ARCH-027)

    func testStageExecutionsCreatedLazily() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        XCTAssertTrue(run.stageExecutions.isEmpty, "No stage executions before start")

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "t1", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1")],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertFalse(run.stageExecutions.isEmpty, "Stage executions created during run")
        let startStage = run.stageExecutions.first { $0.stageID == "start" }
        XCTAssertNotNil(startStage)
        XCTAssertEqual(startStage?.status, .completed)
    }

    // MARK: - Cost Aggregation

    func testCostAggregation() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([
                            AgentTask(agent: "a1", task: "t1", inputs: nil, outputs: nil),
                            AgentTask(agent: "a2", task: "t2", inputs: nil, outputs: nil)
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "a1": makeAgent(id: "a1"),
                "a2": makeAgent(id: "a2")
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        XCTAssertNotNil(run.totalCostCents)
        XCTAssertTrue(run.totalCostCents! > 0, "Cost should be aggregated from executed agents")
    }
}
