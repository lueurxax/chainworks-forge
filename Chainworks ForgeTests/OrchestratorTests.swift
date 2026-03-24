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

    private func makeAgent(
        id: String = "agent_1",
        backendProfileID: String? = nil,
        outputs: [String] = ["output_1"]
    ) -> ResolvedAgent {
        ResolvedAgent(
            id: id, title: "Agent \(id)", mode: "tool_use",
            backendProfileID: backendProfileID,
            provider: "claude_code", model: "opus", effort: "high",
            maxTurns: 10, temperature: 0.0, permissionProfile: "ORCH",
            skillRef: "sk1", skillRole: nil, prompt: "test",
            outputContract: nil, requiresHumanApproval: false,
            inputs: [], outputs: outputs
        )
    }

    private final class MockGooseTransport: GooseTransport, @unchecked Sendable {
        var streamEvents: [GooseStreamEvent] = []

        init() {
            super.init(baseURL: URL(string: "http://localhost:0")!)
        }

        override func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            GooseSessionResponse(
                sessionId: "live-session-001",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        override func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
            let events = streamEvents
            return AsyncThrowingStream { continuation in
                Task {
                    for event in events {
                        continuation.yield(event)
                    }
                    continuation.finish()
                }
            }
        }

        override func closeSession(sessionID: String) async throws {}
    }

    private struct StaticResultExecutor: AgentExecutor {
        let result: AgentResult

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            result
        }
    }

    private func makeReviewCatalog() -> AgentCatalog {
        AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: ["claude_code", "codex"]
            ),
            paths: [:],
            artifacts: [:],
            skills: [:],
            contracts: [
                "proposal_review_v1": ArtifactContract(
                    format: "json",
                    requiredFields: [
                        "agent_id",
                        "role",
                        "score",
                        "decision",
                        "verdict",
                        "summary",
                        "issues",
                        "blocking_issues",
                        "non_blocking_issues",
                        "suggestions",
                        "assumptions"
                    ]
                )
            ],
            backendProfiles: [:],
            permissionProfiles: [:],
            agents: []
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

    func testLiveExecutorPublishesTimelineEvents() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "proposal_loop_live", workflowTitle: "Proposal Loop (Live)",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "proposal_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "proposal_writer", task: "draft_initial_proposal", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_writer", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "proposal_writer": makeAgent(
                    id: "proposal_writer",
                    backendProfileID: "claude_writer_high",
                    outputs: ["proposal_current"]
                )
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let transport = MockGooseTransport()
        transport.streamEvents = [
            .sessionStarted(raw: #"{"session_id":"live-session-001"}"#),
            .promptSubmitted(raw: #"{"request_id":"request-123"}"#),
            .toolCallStarted(toolName: "read_artifact", raw: "{}"),
            .textChunk(text: "Drafting proposal..."),
            .finalOutput(content: "{\"proposal\":\"ready\"}"),
            .sessionClosed(raw: "{}")
        ]
        let executor = GooseAgentExecutor(transport: transport)
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        // Allow fire-and-forget live event routing tasks to complete.
        // configureLiveEventBridge() schedules MainActor tasks via Task { @MainActor in ... },
        // which may not have run yet when start() returns.
        // Uses pollUntil instead of manual for-loop with sleep.
        try? await pollUntil(timeout: 5.0, message: "Live timeline should populate after execution") {
            !orchestrator.liveTimeline.isEmpty
        }

        XCTAssertFalse(orchestrator.liveTimeline.isEmpty, "Live timeline should have entries after execution")
        if !orchestrator.liveTimeline.isEmpty {
            XCTAssertTrue(orchestrator.liveTimeline.contains { $0.event.type == .sessionStarted })
            XCTAssertTrue(orchestrator.liveTimeline.contains { $0.event.type == .toolCallStarted })
            XCTAssertTrue(orchestrator.liveTimeline.contains { $0.event.type == .finalOutput })
        }

        let agentExecution = try? XCTUnwrap(run.stageExecutions.first?.agentExecutions.first)
        XCTAssertEqual(agentExecution?.providerSessionID, "live-session-001")
        XCTAssertEqual(agentExecution?.providerRequestID, "request-123")
        XCTAssertEqual(agentExecution?.resolvedBackendProfileID, "claude_writer_high")
        XCTAssertEqual(agentExecution?.gooseSessionID, "live-session-001")
        XCTAssertTrue(agentExecution?.logSnippet?.contains("Final output") == true)
        XCTAssertNotNil(agentExecution?.transcriptArtifactPath)
        if let consumed = agentExecution?.consumedInputArtifactNamesJSON {
            let names = try? JSONDecoder().decode([String].self, from: consumed)
            XCTAssertEqual(names, [])
        } else {
            XCTFail("Expected consumed input artifact names to be captured")
        }
    }

    func testCompletedRunPersistsFinalFeatureReport() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

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
                    approvalRequired: false,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": makeAgent(id: "a1", backendProfileID: "claude_orchestrator_high")],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .completed)

        let descriptor = FetchDescriptor<Artifact>()
        let reports = try context.fetch(descriptor)
            .filter { $0.runID == run.id && $0.name == "final_feature_report" }
        XCTAssertEqual(reports.count, 1)

        let report = try XCTUnwrap(reports.first)
        let data = try ArtifactStorage.read(filePath: report.filePath, workspaceRoot: workspace.workspaceRoot)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["final_status"] as? String, RunStatus.completed.rawValue)
        XCTAssertEqual(json["cost_currency"] as? String, "USD")
        XCTAssertNotNil(json["summary"] as? String)
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

        // Wait for resume to complete using pollUntil instead of fixed sleep
        try? await pollUntil(timeout: 3.0, message: "Run should complete after approval") {
            run.status == .completed
        }

        assertRunCompleted(run)
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

    // MARK: - Approval Rejection (REQ-005: rejection cancels, not fails)

    /// Proposal contract: approval rejection must cancel the run, not mark it as failed.
    func testApprovalRejectedCancels() async {
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

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        var completionCalled = false
        var completionSuccess = false
        orchestrator.onComplete = { success in
            completionCalled = true
            completionSuccess = success
        }

        await orchestrator.start()
        XCTAssertEqual(run.status, .waitingApproval)
        XCTAssertTrue(orchestrator.isPaused)

        // Reject the approval
        orchestrator.resolveApproval(stageID: "start", granted: false, comment: "Rejected in test")

        // Proposal contract: rejection cancels (not fails)
        XCTAssertEqual(run.status, .cancelled, "Rejected approval must cancel the run, not fail it")
        XCTAssertTrue(orchestrator.isCancelled, "Orchestrator must be marked cancelled")
        XCTAssertFalse(orchestrator.isRunning, "Orchestrator must stop running")
        XCTAssertTrue(completionCalled, "onComplete must fire on rejection")
        XCTAssertFalse(completionSuccess, "onComplete should report failure")

        // Verify the approval record was updated
        let rejectedApproval = run.approvals.first { $0.stageID == "start" }
        XCTAssertNotNil(rejectedApproval)
        XCTAssertEqual(rejectedApproval?.decision, .rejected)
        XCTAssertNotNil(rejectedApproval?.decidedAt)
        XCTAssertEqual(rejectedApproval?.comment, "Rejected in test")
    }

    // MARK: - Run After Approval (REQ-005: run_after_approval block)

    /// Verifies that the run_after_approval block executes after approval is granted.
    func testRunAfterApproval() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Gate with post-approval", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "pre_approval_work", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a2", task: "post_approval_work", inputs: nil, outputs: nil)])
                    ]),
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

        // Should have executed the pre-approval work
        XCTAssertEqual(run.status, .waitingApproval)
        XCTAssertEqual(executor.executedTasks.count, 1, "Should execute pre-approval block")
        XCTAssertEqual(executor.executedTasks[0].task, "pre_approval_work")

        // Grant approval — triggers run_after_approval + transitions
        orchestrator.resolveApproval(stageID: "start", granted: true, comment: "Approved")

        // Wait for the post-approval work + transition to complete using pollUntil
        try? await pollUntil(timeout: 5.0, message: "Run should complete after post-approval work") {
            run.status == .completed
        }

        // Verify the post-approval block executed
        XCTAssertEqual(executor.executedTasks.count, 2, "Should execute both pre- and post-approval blocks")
        XCTAssertEqual(executor.executedTasks[1].agentID, "a2", "Post-approval should use agent a2")
        XCTAssertEqual(executor.executedTasks[1].task, "post_approval_work")
        assertRunCompleted(run)
    }

    func testMalformedReviewJSONFailsBeforeTransitionEvaluation() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = ResolvedAgent(
            id: "reviewer",
            title: "Reviewer",
            mode: "tool_use",
            provider: "claude_code",
            model: "sonnet",
            effort: "high",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "sk-review",
            skillRole: nil,
            prompt: "Review the proposal.",
            outputContract: "proposal_review_v1",
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_po"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Review",
                    type: .start,
                    ownerAgentID: "reviewer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "reviewer", task: "review", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end",
                    label: "End",
                    type: .end,
                    ownerAgentID: "reviewer",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["reviewer": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(
                onError: "fail_run",
                onLoopBudgetExhausted: "fail_run",
                preserveArtifacts: true
            ),
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let result = AgentResult(
            outputs: ["proposal_review_po": Data("not valid json".utf8)],
            logSnippet: "malformed reviewer output",
            costCents: 1,
            succeeded: true,
            errorMessage: nil,
            sessionID: "test-session",
            durationSeconds: 0.1,
            providerReceipt: nil,
            resolvedModel: "fixture-model",
            configuredProviderID: nil,
            adapterVersion: nil
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context,
            catalog: makeReviewCatalog()
        )

        await orchestrator.start()

        XCTAssertEqual(run.status, .failed)
        XCTAssertEqual(run.stageExecutions.count, 1)
        XCTAssertEqual(run.stageExecutions.first?.agentExecutions.first?.status, .failed)
        XCTAssertTrue(run.stageExecutions.first?.agentExecutions.first?.logSnippet?.contains("not valid JSON") == true)
        XCTAssertTrue(run.stageExecutions.first?.agentExecutions.first?.artifacts.isEmpty ?? true)
    }
}
