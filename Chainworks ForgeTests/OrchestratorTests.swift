import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("Orchestrator", .serialized, .tags(.fast))
struct OrchestratorTests {
    let container: ModelContainer
    let context: ModelContext
    let tempDir: URL

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("OrchestratorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("OrchestratorTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    // MARK: - Helpers

    private func makeWorkspace(worktreeRoot: URL? = nil) -> RunWorkspace {
        let runID = UUID()
        let workspaceRoot = tempDir.appendingPathComponent(runID.uuidString, isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        return RunWorkspace(runID: runID, workspaceRoot: workspaceRoot, artifactRoot: artifactRoot, worktreeRoot: worktreeRoot)
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

    private func makeDeliveryConfig(repoRoot: String, targetBranch: String = "dogfood/test") -> DeliveryConfiguration {
        DeliveryConfiguration(
            profileID: "dogfood",
            profileLabel: "Dogfood",
            sampleProfileID: nil,
            repoIdentifier: "local/repo",
            repoRoot: repoRoot,
            baseBranch: "main",
            worktreeBasePath: tempDir.appendingPathComponent("worktrees").path,
            targetBranch: targetBranch,
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )
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

    private actor CapturedContextBox {
        private(set) var artifactNames: [String] = []

        func store(_ names: [String]) {
            artifactNames = names
        }
    }

    private struct CapturingExecutor: AgentExecutor {
        let box: CapturedContextBox

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            await box.store(Array(context.inputArtifacts.keys).sorted())
            return AgentResult(
                outputs: ["output_1": Data("ok".utf8)],
                logSnippet: "captured",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: nil,
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: agent.model,
                configuredProviderID: nil,
                adapterVersion: nil
            )
        }
    }

    private func runGit(_ arguments: [String], in directory: URL) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = arguments
        process.currentDirectoryURL = directory

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        try process.run()
        process.waitUntilExit()

        let output = String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let error = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        if process.terminationStatus != 0 {
            throw NSError(
                domain: "OrchestratorTests.git",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: error]
            )
        }
        return output
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

    @Test("Simple linear workflow completes successfully")
    func simpleLinearWorkflow() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "agent_1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
            #expect(success)
        }

        await orchestrator.start()

        #expect(completed)
        #expect(run.status == .completed)
        #expect(run.completedAt != nil)
        #expect(executor.executedTasks.count == 1)
        #expect(!run.stageExecutions.isEmpty)
    }

    // MARK: - Multi-State Workflow

    @Test("Multi-state workflow executes agents in order")
    func multiStateWorkflow() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "s2": ExecutableState(
                    id: "s2", label: "Stage 2", type: nil,
                    ownerAgentID: "a2",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a2", task: "t2", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "s3", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "s3": ExecutableState(
                    id: "s3", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.status == .completed)
        #expect(executor.executedTasks.count == 2)
        #expect(executor.executedTasks[0].agentID == "a1")
        #expect(executor.executedTasks[1].agentID == "a2")
    }

    // MARK: - Parallel Execution

    @Test("Parallel execution runs all agents concurrently")
    func parallelExecution() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.status == .completed)
        #expect(executor.executedTasks.count == 3)
        let executedAgentIDs = Set(executor.executedTasks.map(\.agentID))
        #expect(executedAgentIDs == Set(["a1", "a2", "a3"]))
    }

    @Test("Live executor publishes timeline events")
    func liveExecutorPublishesTimelineEvents() async throws {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_writer", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "live-session-001",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            events: [
                .sessionStarted(raw: #"{"session_id":"live-session-001"}"#),
                .promptSubmitted(raw: #"{"request_id":"request-123"}"#),
                .toolCallStarted(toolName: "read_artifact", raw: "{}"),
                .textChunk(text: "Drafting proposal..."),
                .finalOutput(content: "{\"proposal\":\"ready\"}"),
                .sessionClosed(raw: "{}")
            ]
        )
        let executor = GooseAgentExecutor(transport: transport)
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        // Allow fire-and-forget live event routing tasks to complete.
        // configureLiveEventBridge() schedules MainActor tasks via Task { @MainActor in ... },
        // which may not have run yet when start() returns.
        // Uses awaitCondition instead of pollUntil.
        await awaitCondition("Live timeline should populate after execution", timeout: 5.0) {
            !orchestrator.liveTimeline.isEmpty
        }

        #expect(!orchestrator.liveTimeline.isEmpty, "Live timeline should have entries after execution")
        if !orchestrator.liveTimeline.isEmpty {
            #expect(orchestrator.liveTimeline.contains { $0.event.type == .sessionStarted })
            #expect(orchestrator.liveTimeline.contains { $0.event.type == .toolCallStarted })
            #expect(orchestrator.liveTimeline.contains { $0.event.type == .finalOutput })
        }

        let agentExecution = run.stageExecutions.first?.agentExecutions.first
        try #require(agentExecution != nil)
        #expect(agentExecution?.providerSessionID == "live-session-001")
        #expect(agentExecution?.providerRequestID == "request-123")
        #expect(agentExecution?.resolvedBackendProfileID == "claude_writer_high")
        #expect(agentExecution?.gooseSessionID == "live-session-001")
        #expect(agentExecution?.logSnippet?.contains("Final output") == true)
        #expect(agentExecution?.transcriptArtifactPath != nil)
        if let consumed = agentExecution?.consumedInputArtifactNamesJSON {
            let names = try? JSONDecoder().decode([String].self, from: consumed)
            #expect(names == [])
        } else {
            Issue.record("Expected consumed input artifact names to be captured")
        }
    }

    @Test("Orchestrator persists canonical execution truth on successful agent execution")
    func persistsCanonicalExecutionTruthOnSuccess() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent(id: "agent_1")

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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "agent_1",
                    runBlock: nil, runAfterApproval: nil, transitions: [],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["agent_1": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(onError: "fail_run", onLoopBudgetExhausted: "fail_run", preserveArtifacts: true),
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let result = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "done",
            costCents: 5,
            succeeded: true,
            errorMessage: nil,
            sessionID: "runtime-session-1",
            durationSeconds: 0.1,
            providerReceipt: ProviderExecutionReceipt(
                providerFamily: "goose_openai",
                configuredProviderID: nil,
                model: "gpt-5.4",
                effort: "high",
                transport: "http_sse",
                inputTokens: 10,
                outputTokens: 20,
                billedUnits: nil,
                costCents: 5,
                wallClockSeconds: 0.1,
                rawReceiptJSON: nil
            ),
            resolvedModel: "configured-model",
            configuredProviderID: nil,
            adapterVersion: "adapter-v2",
            canonicalOutcome: .completed,
            transportErrorKind: nil,
            providerStopReason: "end_turn",
            outputPresence: .durableOutput,
            runtimeProvider: "runtime-provider",
            runtimeModel: "runtime-model",
            outcomeEnvelope: OutcomeEnvelope(
                canonicalOutcome: .completed,
                transportErrorKind: nil,
                providerStopReason: "end_turn",
                outputPresence: .durableOutput,
                rawErrorMessage: nil,
                rawFinishEvent: #"{"stop_reason":"end_turn"}"#
            )
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context
        )

        await orchestrator.start()

        let agentExec = try #require(run.stageExecutions.first?.agentExecutions.first)
        #expect(agentExec.status == .completed)
        #expect(agentExec.canonicalOutcome == .completed)
        #expect(agentExec.outputPresence == .durableOutput)
        #expect(agentExec.providerStopReason == "end_turn")
        #expect(agentExec.runtimeProvider == "runtime-provider")
        #expect(agentExec.runtimeModel == "runtime-model")
        #expect(agentExec.settledAt != nil)
        #expect(agentExec.providerSessionID == "runtime-session-1")

        let envelopeData = try #require(agentExec.outcomeEnvelopeJSON)
        let envelope = try JSONDecoder().decode(OutcomeEnvelope.self, from: envelopeData)
        #expect(envelope.canonicalOutcome == .completed)
        #expect(envelope.providerStopReason == "end_turn")
        #expect(envelope.outputPresence == .durableOutput)
        #expect(envelope.rawFinishEvent == #"{"stop_reason":"end_turn"}"#)
    }

    @Test("Validation failure overrides provisional completed outcome with failed-after-output-validation")
    func validationFailureOverridesCompletedOutcome() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: ["claude_code"]
            ),
            paths: [:],
            artifacts: [:],
            skills: [:],
            contracts: [
                "output_1_v1": ArtifactContract(format: "json", requiredFields: ["score"])
            ],
            backendProfiles: [:],
            permissionProfiles: [:],
            agents: []
        )

        let agent = makeAgent(id: "agent_1", outputs: ["output_1"])
        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "agent_1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "agent_1", task: "produce", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["agent_1": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(onError: "fail_run", onLoopBudgetExhausted: "fail_run", preserveArtifacts: true),
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let result = AgentResult(
            outputs: ["output_1": Data("not valid json".utf8)],
            logSnippet: "produced invalid payload",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0.1,
            providerReceipt: nil,
            resolvedModel: "fixture-model",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            transportErrorKind: nil,
            providerStopReason: "end_turn",
            outputPresence: .durableOutput,
            runtimeProvider: "claude-code",
            runtimeModel: "fixture-model"
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context,
            catalog: catalog
        )

        await orchestrator.start()

        let agentExec = try #require(run.stageExecutions.first?.agentExecutions.first)
        #expect(run.status == .failed)
        #expect(agentExec.status == .failed)
        #expect(agentExec.canonicalOutcome == .failedAfterOutputValidation)
        #expect(agentExec.outputPresence == .durableOutput)
        #expect(agentExec.settledAt != nil)

        let envelopeData = try #require(agentExec.outcomeEnvelopeJSON)
        let envelope = try JSONDecoder().decode(OutcomeEnvelope.self, from: envelopeData)
        #expect(envelope.canonicalOutcome == .failedAfterOutputValidation)
    }

    @Test("Completed run persists final feature report")
    func completedRunPersistsFinalFeatureReport() async throws {
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
                    approvalPolicy: nil,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
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

        #expect(run.status == .completed)

        let descriptor = FetchDescriptor<Artifact>()
        let reports = try context.fetch(descriptor)
            .filter { $0.runID == run.id && $0.name == "final_feature_report" }
        #expect(reports.count == 1)

        let report = try #require(reports.first)
        let data = try ArtifactStorage.read(filePath: report.filePath, workspaceRoot: workspace.workspaceRoot)
        let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(json["final_status"] as? String == RunStatus.completed.rawValue)
        #expect(json["cost_currency"] as? String == "USD")
        #expect(json["summary"] as? String != nil)
    }

    // MARK: - Approval Gate

    @Test("Approval gate pauses execution")
    func approvalGatePausesExecution() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Approval Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .approvalGranted)],
                    approvalRequired: true, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status == .waitingApproval)
        #expect(orchestrator.isPaused)
        #expect(receivedApprovalRequest != nil)
        #expect(receivedApprovalRequest?.stageID == "start")
    }

    @Test("Approval granted resumes execution")
    func approvalGrantedResumesExecution() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .approvalGranted)],
                    approvalRequired: true, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status == .waitingApproval)

        // Grant approval — this triggers resume
        orchestrator.resolveApproval(stageID: "start", granted: true, comment: "Approved")

        // Wait for resume to complete using awaitCondition instead of pollUntil
        await awaitCondition("Run should complete after approval", timeout: 3.0) {
            run.status == .completed
        }

        expectRunCompleted(run)
    }

    // MARK: - Agent Failure

    @Test("Agent failure pauses run with failure policy")
    func agentFailurePausesRun() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.status == .blocked)
    }

    // MARK: - Cancellation

    @Test("Cancellation stops the run")
    func cancellation() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.status == .cancelled)
    }

    // MARK: - Transition Conditions

    @Test("Artifact-exists transition advances to next state")
    func artifactExistsTransition() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "middle": ExecutableState(
                    id: "middle", label: "Middle", type: nil,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "consume", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.status == .completed)
        #expect(executor.executedTasks.count == 2)
    }

    // MARK: - Lazy Stage Creation (ARCH-027)

    @Test("Stage executions are created lazily during run")
    func stageExecutionsCreatedLazily() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        #expect(run.stageExecutions.isEmpty, "No stage executions before start")

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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(!run.stageExecutions.isEmpty, "Stage executions created during run")
        let startStage = run.stageExecutions.first { $0.stageID == "start" }
        #expect(startStage != nil)
        #expect(startStage?.status == .completed)
    }

    // MARK: - Cost Aggregation

    @Test("Cost is aggregated from executed agents")
    func costAggregation() async {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        #expect(run.totalCostCents != nil)
        #expect(run.totalCostCents! > 0, "Cost should be aggregated from executed agents")
    }

    // MARK: - Approval Rejection (REQ-005: rejection cancels, not fails)

    /// Proposal contract: approval rejection must cancel the run, not mark it as failed.
    @Test("Approval rejection cancels the run")
    func approvalRejectedCancels() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Approval Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .approvalGranted)],
                    approvalRequired: true, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status == .waitingApproval)
        #expect(orchestrator.isPaused)

        // Reject the approval
        orchestrator.resolveApproval(stageID: "start", granted: false, comment: "Rejected in test")

        // Proposal contract: rejection cancels (not fails)
        #expect(run.status == .cancelled, "Rejected approval must cancel the run, not fail it")
        #expect(orchestrator.isCancelled, "Orchestrator must be marked cancelled")
        #expect(!orchestrator.isRunning, "Orchestrator must stop running")
        #expect(completionCalled, "onComplete must fire on rejection")
        #expect(!completionSuccess, "onComplete should report failure")

        // Verify the approval record was updated
        let rejectedApproval = run.approvals.first { $0.stageID == "start" }
        #expect(rejectedApproval != nil)
        #expect(rejectedApproval?.decision == .rejected)
        #expect(rejectedApproval?.decidedAt != nil)
        #expect(rejectedApproval?.comment == "Rejected in test")
    }

    // MARK: - Run After Approval (REQ-005: run_after_approval block)

    /// Verifies that the run_after_approval block executes after approval is granted.
    @Test("Run-after-approval block executes post approval")
    func runAfterApproval() async {
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
                    approvalRequired: true, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status == .waitingApproval)
        #expect(executor.executedTasks.count == 1, "Should execute pre-approval block")
        #expect(executor.executedTasks[0].task == "pre_approval_work")

        // Grant approval — triggers run_after_approval + transitions
        orchestrator.resolveApproval(stageID: "start", granted: true, comment: "Approved")

        // Wait for the post-approval work + transition to complete using awaitCondition
        await awaitCondition("Run should complete after post-approval work", timeout: 5.0) {
            run.status == .completed
        }

        // Verify the post-approval block executed
        #expect(executor.executedTasks.count == 2, "Should execute both pre- and post-approval blocks")
        #expect(executor.executedTasks[1].agentID == "a2", "Post-approval should use agent a2")
        #expect(executor.executedTasks[1].task == "post_approval_work")
        expectRunCompleted(run)
    }

    @Test("Malformed review JSON fails before transition evaluation")
    func malformedReviewJSONFailsBeforeTransitionEvaluation() async {
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
                    approvalPolicy: nil,
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
                    approvalPolicy: nil,
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

        // Proposal-review outputs are strict JSON. Markdown-only output must block the run.
        #expect(run.status == .blocked || run.status == .failed)
        let agentExec = run.stageExecutions.first?.agentExecutions.first
        #expect(agentExec?.status == .failed)
        // Raw outputs are persisted as artifacts
        #expect(agentExec?.artifacts.isEmpty == false)
    }

    @Test("Repo-backed execution injects source context into agent inputs")
    func repoBackedExecutionInjectsSourceContextIntoInputs() async throws {
        let repoRoot = tempDir.appendingPathComponent("repo-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)

        try "struct App {}\n".write(
            to: repoRoot.appendingPathComponent("App.swift"),
            atomically: true,
            encoding: .utf8
        )

        _ = try runGit(["init", "-b", "main"], in: repoRoot)
        _ = try runGit(["config", "user.email", "tests@example.com"], in: repoRoot)
        _ = try runGit(["config", "user.name", "Chainworks Tests"], in: repoRoot)
        _ = try runGit(["add", "App.swift"], in: repoRoot)
        _ = try runGit(["commit", "-m", "Initial commit"], in: repoRoot)

        try "struct App { let version = 2 }\n".write(
            to: repoRoot.appendingPathComponent("App.swift"),
            atomically: true,
            encoding: .utf8
        )

        let workspace = makeWorkspace(worktreeRoot: repoRoot)
        let run = makeRun(workspace: workspace)
        let config = DeliveryConfiguration(
            profileID: "dogfood",
            profileLabel: "Dogfood",
            sampleProfileID: nil,
            repoIdentifier: "local/repo",
            repoRoot: repoRoot.path,
            baseBranch: "main",
            worktreeBasePath: tempDir.appendingPathComponent("worktrees").path,
            targetBranch: "feature/test",
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )
        run.deliveryConfigurationJSON = try JSONEncoder().encode(config)
        run.worktreeRoot = repoRoot.path
        run.baseRevision = try runGit(["rev-parse", "HEAD"], in: repoRoot).trimmingCharacters(in: .whitespacesAndNewlines)

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "tool_use",
            provider: "claude_code",
            model: "default",
            effort: "high",
            maxTurns: 12,
            temperature: 0,
            permissionProfile: "write",
            skillRef: "implementation_core",
            skillRole: nil,
            prompt: "Implement the change.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output_1"],
            worktreeWriteEnabled: true
        )

        let plan = RunPlan(
            workflowID: "delivery",
            workflowTitle: "Delivery",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Implementation",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "implement", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end",
                    label: "Done",
                    type: .end,
                    ownerAgentID: "code_writer",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["code_writer": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            requiresProjectAccess: true,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let box = CapturedContextBox()
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: CapturingExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        let artifactNames = await box.artifactNames
        #expect(artifactNames.contains("source_context"))
        #expect(artifactNames.contains("source_diff_summary"))
        #expect(artifactNames.contains("source_changed_files_manifest"))
        #expect(run.status == .completed)
    }

    @Test("Delivery receipt is emitted from real release artifacts on terminal delivery run")
    func deliveryReceiptEmittedFromReleaseArtifacts() async throws {
        let repoRoot = tempDir.appendingPathComponent("delivery-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)

        let workspace = makeWorkspace(worktreeRoot: repoRoot)
        let run = makeRun(workspace: workspace)
        run.deliveryConfigurationJSON = try JSONEncoder().encode(makeDeliveryConfig(repoRoot: repoRoot.path))
        run.worktreeRoot = repoRoot.path
        run.baseRevision = "abc123def456"

        let releaseStage = StageExecution(
            stageID: "state_11_manual_release",
            label: "Manual release gate",
            startedAt: Date().addingTimeInterval(-120),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        releaseStage.completedAt = Date().addingTimeInterval(-60)
        releaseStage.run = run
        context.insert(releaseStage)

        let gitExecution = AgentExecution(
            agentID: "commit_and_push_to_github",
            agentTitle: "Commit and Push",
            taskName: "commit_and_push",
            startedAt: Date().addingTimeInterval(-100),
            status: .completed,
            provider: "system",
            effort: "high"
        )
        gitExecution.completedAt = Date().addingTimeInterval(-90)
        gitExecution.stageExecution = releaseStage
        context.insert(gitExecution)

        let connectExecution = AgentExecution(
            agentID: "build_archive_and_push_connect",
            agentTitle: "Build and Distribute",
            taskName: "build_and_distribute",
            startedAt: Date().addingTimeInterval(-80),
            status: .completed,
            provider: "system",
            effort: "high"
        )
        connectExecution.completedAt = Date().addingTimeInterval(-70)
        connectExecution.stageExecution = releaseStage
        context.insert(connectExecution)

        let artifactManager = ArtifactManager(modelContext: context)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let gitAgent = makeAgent(
            id: "commit_and_push_to_github",
            outputs: ["release_manifest", "git_push_receipt"]
        )
        let connectAgent = makeAgent(
            id: "build_archive_and_push_connect",
            outputs: ["release_bundle_manifest", "connect_upload_receipt"]
        )
        let reviewAgent = makeAgent(
            id: "implementation_reviewer",
            outputs: ["implementation_review_summary"]
        )

        let gitManifest = GitReleaseService.ReleaseManifest(
            commitSHA: "abc123def456",
            branch: "dogfood/test",
            remote: "origin",
            commitMessage: "[local/repo] Apply approved_proposal via Chainworks Forge",
            filesChanged: 3,
            insertions: 42,
            deletions: 5,
            timestamp: Date()
        )
        let gitReceipt = GitReleaseService.GitPushReceipt(
            commitSHA: "abc123def456",
            remote: "origin",
            branch: "dogfood/test",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )
        let bundleManifest = ConnectPublishService.ReleaseBundleManifest(
            bundleIdentifier: "com.chainworks.forge.sandbox",
            bundleVersion: "1.0.0",
            buildNumber: "abc123de",
            archivePath: repoRoot.appendingPathComponent(".build").path,
            checksumSHA256: "deadbeef",
            sizeBytes: 1024,
            timestamp: Date()
        )
        let uploadReceipt = ConnectPublishService.ConnectUploadReceipt(
            artifactID: UUID().uuidString,
            destination: "sandbox://sandbox_local",
            releaseTargetID: "sandbox_local",
            releaseMode: "sandbox",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )

        _ = try artifactManager.persistOutputs(
            outputs: [
                "release_manifest": try encoder.encode(gitManifest),
                "git_push_receipt": try encoder.encode(gitReceipt)
            ],
            agent: gitAgent,
            agentExecution: gitExecution,
            workspace: workspace,
            stageID: releaseStage.stageID,
            iteration: releaseStage.iteration,
            attemptNumber: releaseStage.attemptNumber,
            catalog: nil
        )
        _ = try artifactManager.persistOutputs(
            outputs: [
                "release_bundle_manifest": try encoder.encode(bundleManifest),
                "connect_upload_receipt": try encoder.encode(uploadReceipt)
            ],
            agent: connectAgent,
            agentExecution: connectExecution,
            workspace: workspace,
            stageID: releaseStage.stageID,
            iteration: releaseStage.iteration,
            attemptNumber: releaseStage.attemptNumber,
            catalog: nil
        )

        let reviewExecution = AgentExecution(
            agentID: "implementation_reviewer",
            agentTitle: "Implementation Reviewer",
            taskName: "review",
            startedAt: Date().addingTimeInterval(-85),
            status: .completed,
            provider: "system",
            effort: "high"
        )
        reviewExecution.completedAt = Date().addingTimeInterval(-75)
        reviewExecution.stageExecution = releaseStage
        context.insert(reviewExecution)
        _ = try artifactManager.persistOutputs(
            outputs: [
                "implementation_review_summary": Data("""
                {"decision":"implemented","pass":true}
                """.utf8)
            ],
            agent: reviewAgent,
            agentExecution: reviewExecution,
            workspace: workspace,
            stageID: releaseStage.stageID,
            iteration: releaseStage.iteration,
            attemptNumber: releaseStage.attemptNumber,
            catalog: nil
        )

        let endState = ExecutableState(
            id: "state_12_workflow_complete",
            label: "Workflow complete",
            type: .end,
            ownerAgentID: "lead_orchestrator",
            runBlock: nil,
            runAfterApproval: nil,
            transitions: [],
            approvalRequired: false,
            approvalPolicy: nil,
            loop: nil
        )
        let plan = RunPlan(
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live",
            states: ["state_12_workflow_complete": endState],
            initialStateID: "state_12_workflow_complete",
            agentBindings: [:],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context
        )

        await orchestrator.start()

        let artifacts = try artifactManager.artifacts(forRunID: run.id)
        let receiptArtifact = try #require(artifacts.last(where: { $0.name == "delivery_receipt" }))
        let receiptData = try artifactManager.readArtifact(receiptArtifact, workspace: workspace)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let receipt = try decoder.decode(DeliveryReceiptBuilder.DeliveryReceipt.self, from: receiptData)

        #expect(run.status == .completed)
        #expect(receipt.releaseResult?.succeeded == true)
        #expect(receipt.releaseResult?.commitSHA == "abc123def456")
        #expect(receipt.implementationReviewStatus == "implemented")
    }

    @Test("Delivery receipt is emitted for partial delivery failure")
    func deliveryReceiptEmittedForPartialFailure() async throws {
        let repoRoot = tempDir.appendingPathComponent("delivery-failed-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)

        let workspace = makeWorkspace(worktreeRoot: repoRoot)
        let run = makeRun(workspace: workspace)
        run.deliveryConfigurationJSON = try JSONEncoder().encode(makeDeliveryConfig(repoRoot: repoRoot.path))
        run.worktreeRoot = repoRoot.path
        run.baseRevision = "fff111222"

        let releaseStage = StageExecution(
            stageID: "state_11_manual_release",
            label: "Manual release gate",
            startedAt: Date().addingTimeInterval(-120),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        releaseStage.completedAt = Date().addingTimeInterval(-60)
        releaseStage.run = run
        context.insert(releaseStage)

        let gitExecution = AgentExecution(
            agentID: "commit_and_push_to_github",
            agentTitle: "Commit and Push",
            taskName: "commit_and_push",
            startedAt: Date().addingTimeInterval(-100),
            status: .completed,
            provider: "system",
            effort: "high"
        )
        gitExecution.completedAt = Date().addingTimeInterval(-90)
        gitExecution.stageExecution = releaseStage
        context.insert(gitExecution)

        let connectExecution = AgentExecution(
            agentID: "build_archive_and_push_connect",
            agentTitle: "Build and Distribute",
            taskName: "build_and_distribute",
            startedAt: Date().addingTimeInterval(-80),
            status: .failed,
            provider: "system",
            effort: "high"
        )
        connectExecution.completedAt = Date().addingTimeInterval(-70)
        connectExecution.logSnippet = "ConnectPublishService failed: Upload failed"
        connectExecution.stageExecution = releaseStage
        context.insert(connectExecution)

        let artifactManager = ArtifactManager(modelContext: context)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let gitAgent = makeAgent(
            id: "commit_and_push_to_github",
            outputs: ["release_manifest", "git_push_receipt"]
        )
        let gitManifest = GitReleaseService.ReleaseManifest(
            commitSHA: "fff111222",
            branch: "dogfood/test",
            remote: "origin",
            commitMessage: "[local/repo] Apply approved_proposal via Chainworks Forge",
            filesChanged: 1,
            insertions: 10,
            deletions: 2,
            timestamp: Date()
        )
        let gitReceipt = GitReleaseService.GitPushReceipt(
            commitSHA: "fff111222",
            remote: "origin",
            branch: "dogfood/test",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )
        _ = try artifactManager.persistOutputs(
            outputs: [
                "release_manifest": try encoder.encode(gitManifest),
                "git_push_receipt": try encoder.encode(gitReceipt)
            ],
            agent: gitAgent,
            agentExecution: gitExecution,
            workspace: workspace,
            stageID: releaseStage.stageID,
            iteration: releaseStage.iteration,
            attemptNumber: releaseStage.attemptNumber,
            catalog: nil
        )

        let failingState = ExecutableState(
            id: "state_11_manual_release",
            label: "Manual release gate",
            type: .start,
            ownerAgentID: "build_archive_and_push_connect",
            runBlock: ExecutableRunBlock(phases: [
                .sequential([AgentTask(agent: "failing_agent", task: "fail", inputs: nil, outputs: nil)])
            ]),
            runAfterApproval: nil,
            transitions: [],
            approvalRequired: false,
            approvalPolicy: nil,
            loop: nil
        )
        let plan = RunPlan(
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live",
            states: ["state_11_manual_release": failingState],
            initialStateID: "state_11_manual_release",
            agentBindings: ["failing_agent": makeAgent(id: "failing_agent")],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(
                onError: "pause_and_require_human",
                onLoopBudgetExhausted: "pause_and_require_human",
                preserveArtifacts: true
            ),
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let failingResult = AgentResult(
            outputs: [:],
            logSnippet: "failed",
            costCents: nil,
            succeeded: false,
            errorMessage: "failure",
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: failingResult),
            modelContext: context
        )

        await orchestrator.start()

        let artifacts = try artifactManager.artifacts(forRunID: run.id)
        let receiptArtifact = try #require(artifacts.last(where: { $0.name == "delivery_receipt" }))
        let receiptData = try artifactManager.readArtifact(receiptArtifact, workspace: workspace)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let receipt = try decoder.decode(DeliveryReceiptBuilder.DeliveryReceipt.self, from: receiptData)

        #expect(run.status == .blocked)
        #expect(receipt.releaseResult?.succeeded == false)
        #expect(receipt.releaseResult?.failureStage == "build_archive_and_push")
        #expect(receipt.releaseResult?.commitSHA == "fff111222")
    }
}
