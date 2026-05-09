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
        TestModelContainerRetainer.retain(container)
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
                adapterVersion: nil,
                canonicalOutcome: .completed,
                sessionReuseDisposition: .fresh
            )
        }
    }

    private actor SequencedExecutionBox {
        private var remaining: [AgentResult]
        private(set) var models: [String] = []

        init(results: [AgentResult]) {
            self.remaining = results
        }

        func next(model: String) -> AgentResult {
            models.append(model)
            return remaining.removeFirst()
        }
    }

    private struct SequencedExecutor: AgentExecutor {
        let box: SequencedExecutionBox

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            await box.next(model: agent.model)
        }
    }

    private actor AgentResultBox {
        private let resultsByAgentID: [String: AgentResult]

        init(resultsByAgentID: [String: AgentResult]) {
            self.resultsByAgentID = resultsByAgentID
        }

        func result(for agentID: String) -> AgentResult {
            resultsByAgentID[agentID] ?? AgentResult(
                outputs: [:],
                logSnippet: "missing fixture result",
                costCents: nil,
                succeeded: false,
                errorMessage: "missing fixture result",
                sessionID: nil,
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                canonicalOutcome: .failedBeforeOutput,
                sessionReuseDisposition: .fresh,
                outputPresence: .none
            )
        }
    }

    private struct AgentResultExecutor: AgentExecutor {
        let box: AgentResultBox

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            await box.result(for: agent.id)
        }
    }

    private actor BlockingSecondTaskBox {
        private var hasStartedSecondTask = false
        private var releaseContinuation: CheckedContinuation<Void, Never>?

        func run(taskName: String) async -> AgentResult {
            if taskName == "first_task" {
                return AgentResult(
                    outputs: [:],
                    logSnippet: "first",
                    costCents: nil,
                    succeeded: true,
                    errorMessage: nil,
                    sessionID: nil,
                    durationSeconds: 0,
                    providerReceipt: nil,
                    resolvedModel: nil,
                    configuredProviderID: nil,
                    adapterVersion: nil,
                    canonicalOutcome: .completed,
                    sessionReuseDisposition: .fresh
                )
            }

            hasStartedSecondTask = true
            await withCheckedContinuation { continuation in
                releaseContinuation = continuation
            }

            return AgentResult(
                outputs: [:],
                logSnippet: "second",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: nil,
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                canonicalOutcome: .completed,
                sessionReuseDisposition: .fresh
            )
        }

        func waitUntilSecondTaskStarts() async {
            while !hasStartedSecondTask {
                try? await Task.sleep(for: .milliseconds(10))
            }
        }

        func releaseSecondTask() {
            releaseContinuation?.resume()
            releaseContinuation = nil
        }
    }

    private struct BlockingSecondTaskExecutor: AgentExecutor {
        let box: BlockingSecondTaskBox

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            await box.run(taskName: task.task)
        }
    }

    private actor BlockingTaskStartBox {
        private let expectedStartCount: Int
        private var startedTaskNames: [String] = []
        private var releaseContinuations: [CheckedContinuation<Void, Never>] = []

        init(expectedStartCount: Int) {
            self.expectedStartCount = expectedStartCount
        }

        func run(taskName: String) async -> AgentResult {
            startedTaskNames.append(taskName)
            await withCheckedContinuation { continuation in
                releaseContinuations.append(continuation)
            }

            return AgentResult(
                outputs: [:],
                logSnippet: "completed \(taskName)",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: nil,
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                canonicalOutcome: .completed,
                sessionReuseDisposition: .fresh
            )
        }

        func waitUntilExpectedTasksStart() async {
            while startedTaskNames.count < expectedStartCount {
                try? await Task.sleep(for: .milliseconds(10))
            }
        }

        func releaseAll() {
            let continuations = releaseContinuations
            releaseContinuations.removeAll()
            continuations.forEach { $0.resume() }
        }
    }

    private struct BlockingTaskStartExecutor: AgentExecutor {
        let box: BlockingTaskStartBox

        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            await box.run(taskName: task.task)
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

    private func makeImplementationCatalog() -> AgentCatalog {
        AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Implementation test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: ["codex"]
            ),
            paths: [:],
            artifacts: [:],
            skills: [:],
            contracts: [
                "implementation_progress": ArtifactContract(
                    format: "json",
                    requiredFields: ["status", "current_phase", "completed_items", "deferred_items", "notes"]
                ),
                "implementation_self_assessment_v2": ArtifactContract(
                    format: "json",
                    requiredFields: ["implementation_complete", "verification_green", "remaining_code_tasks", "handoff_tasks", "known_risks", "tests_run", "docs_impacted"]
                ),
                "changed_files_manifest": ArtifactContract(
                    format: "json",
                    requiredFields: ["files"]
                ),
                "tests_result": ArtifactContract(
                    format: "json",
                    requiredFields: ["status", "summary"]
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

    @Test("Strategy escalation reruns retryable failures with the escalated tier model")
    func strategyEscalationRerunsWithEscalatedModelTier() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "agent_1": ResolvedProviderBinding(
                agentID: "agent_1",
                backendProfileID: "bp",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.claudeACP.rawValue,
                providerIdentifier: ProviderFamily.claudeACP.runtimeProviderIdentifier,
                model: "sonnet",
                effort: "high",
                transport: "acp_http",
                adapterVersion: "v1"
            )
        ])

        let agent = makeAgent(
            id: "agent_1",
            backendProfileID: "bp",
            outputs: ["output_1"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "agent_1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "agent_1", task: "do_work", inputs: nil, outputs: ["output_1"])])
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
                    ownerAgentID: "agent_1",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["agent_1": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let firstFailure = AgentResult(
            outputs: [:],
            logSnippet: "transport failure",
            costCents: nil,
            succeeded: false,
            errorMessage: "timed out while streaming",
            sessionID: nil,
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "sonnet",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .timedOutBeforeOutput,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .timeout,
            outputPresence: .none,
            runtimeProvider: "claude_code",
            runtimeModel: "sonnet"
        )
        let secondSuccess = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "opus",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "claude_code",
            runtimeModel: "opus"
        )
        let box = SequencedExecutionBox(results: [firstFailure, secondSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        let models = await box.models
        #expect(models == ["sonnet", "opus"])
        #expect(run.status == .completed)

        let agentExec = try #require(run.stageExecutions.first?.agentExecutions.first)
        #expect(agentExec.modelTierUsed == "frontier")
        let signals = try #require(
            agentExec.limitPressureSignalsJSON.flatMap {
                try? JSONDecoder().decode(StrategyLimitPressureSignals.self, from: $0)
            }
        )
        #expect(signals.escalationCount == 1)
        #expect(signals.retryableEscalationCount == 1)
    }

    @Test("Architect reviewer never resolves Codex fast tier to Spark")
    func architectReviewerNeverResolvesCodexFastTierToSpark() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "proposal_reviewer_architect": ResolvedProviderBinding(
                agentID: "proposal_reviewer_architect",
                backendProfileID: "codex_architect_high",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "extra high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let agent = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Proposal Reviewer / Architect",
            mode: "proposal_review.architect",
            backendProfileID: "codex_architect_high",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            model: "GPT-5.4",
            effort: "extra high",
            maxTurns: 12,
            temperature: 0.0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: "architect",
            prompt: "Review the proposal as an architect.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_architect"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "proposal_reviewer_architect", task: "review_architecture", inputs: nil, outputs: ["proposal_review_architect"])])
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
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["proposal_reviewer_architect": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let success = AgentResult(
            outputs: ["proposal_review_architect": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-architect",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let box = SequencedExecutionBox(results: [success])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(await box.models == ["GPT-5.4"])
        #expect(run.status == .completed)

        let agentExec = try #require(run.stageExecutions.first?.agentExecutions.first)
        #expect(agentExec.modelTierUsed == "frontier")
    }

    @Test("Strategy escalation does not rerun apply-patch verification failures")
    func strategyEscalationDoesNotRerunApplyPatchVerificationFailures() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "code_writer": ResolvedProviderBinding(
                agentID: "code_writer",
                backendProfileID: "codex_writer_high",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let agent = makeAgent(
            id: "code_writer",
            backendProfileID: "codex_writer_high",
            outputs: ["implementation_progress"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "continue_implementation", inputs: nil, outputs: ["implementation_progress"])])
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
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let failure = AgentResult(
            outputs: [:],
            logSnippet: "apply_patch verification failed",
            costCents: nil,
            succeeded: false,
            errorMessage: "apply_patch verification failed: Failed to find expected lines in AddTransactionView.swift",
            sessionID: "session-1",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completedWithTransportError,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .provider,
            providerStopReason: "apply_patch_verification_failed",
            outputPresence: .none,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let unusedSuccess = AgentResult(
            outputs: ["implementation_progress": Data("ok".utf8)],
            logSnippet: "should not run",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-2",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.3-codex-spark",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "gpt-5.3-codex-spark"
        )
        let box = SequencedExecutionBox(results: [failure, unusedSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        let attemptedModels = await box.models
        #expect(attemptedModels.count == 1)
        #expect(run.status == .failed)

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let attempts = stage.agentExecutions.filter { $0.agentID == "code_writer" }
        #expect(attempts.count == 1)
        let agentExec = try #require(attempts.first)
        #expect(agentExec.status == .failed)
        #expect(agentExec.providerStopReason == "apply_patch_verification_failed")
    }

    @Test("Strategy escalation reruns Codex usage-limit failures with the frontier model")
    func strategyEscalationRerunsCodexUsageLimitFailuresWithFrontierModel() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "code_writer": ResolvedProviderBinding(
                agentID: "code_writer",
                backendProfileID: "codex_writer_high",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            backendProfileID: "codex_writer_high",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "implement",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["implementation_progress"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "continue_implementation", inputs: nil, outputs: ["implementation_progress"])])
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
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let firstFailure = AgentResult(
            outputs: [:],
            logSnippet: "usage limit exhausted",
            costCents: nil,
            succeeded: false,
            errorMessage: "Internal error (code -32603) {\"codex_error_info\":\"usage_limit_exceeded\"}",
            sessionID: "session-1",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.3-codex-spark",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .limitExhaustedBeforeOutput,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .provider,
            providerStopReason: "usage_limit_exceeded",
            outputPresence: .none,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "gpt-5.3-codex-spark"
        )
        let secondSuccess = AgentResult(
            outputs: ["implementation_progress": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-2",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let box = SequencedExecutionBox(results: [firstFailure, secondSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(await box.models == ["gpt-5.3-codex-spark", "GPT-5.4"])
        #expect(run.status == .completed)
    }

    @Test("Gemini capacity exhaustion reruns sequential execution with a stable fallback model")
    func geminiCapacityFallbackRerunsSequentialExecutionWithStableModel() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "proposal_reviewer_ui": ResolvedProviderBinding(
                agentID: "proposal_reviewer_ui",
                backendProfileID: "gemini_3_1_pro_preview_high",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.geminiACP.rawValue,
                providerIdentifier: ProviderFamily.geminiACP.runtimeProviderIdentifier,
                model: "gemini-3.1-pro-preview",
                effort: "medium",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "gemini_cli_acp",
                adapterFamily: "gemini_cli_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let agent = ResolvedAgent(
            id: "proposal_reviewer_ui",
            title: "Proposal Reviewer / UI",
            mode: "proposal_review.ui",
            backendProfileID: "gemini_3_1_pro_preview_high",
            provider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            model: "gemini-3.1-pro-preview",
            effort: "medium",
            maxTurns: 12,
            temperature: 0.2,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: nil,
            prompt: "review proposal",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_ui"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "proposal_reviewer_ui",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "proposal_reviewer_ui", task: "review_ui", inputs: nil, outputs: ["proposal_review_ui"])])
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
                    ownerAgentID: "proposal_reviewer_ui",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["proposal_reviewer_ui": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let firstFailure = AgentResult(
            outputs: [:],
            logSnippet: "capacity exhausted",
            costCents: nil,
            succeeded: false,
            errorMessage: "No capacity available for model gemini-3.1-pro-preview on the server",
            sessionID: "session-1",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gemini-2.5-flash",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .limitExhaustedBeforeOutput,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .provider,
            providerStopReason: "model_capacity_exhausted",
            outputPresence: .none,
            runtimeProvider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            runtimeModel: "gemini-2.5-flash"
        )
        let secondSuccess = AgentResult(
            outputs: ["proposal_review_ui": Data("ok".utf8)],
            logSnippet: "fallback success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-2",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gemini-3.1-pro-preview",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            runtimeModel: "gemini-3.1-pro-preview"
        )
        let box = SequencedExecutionBox(results: [firstFailure, secondSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(await box.models == ["gemini-3.1-pro-preview", "gemini-2.5-flash"])

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let attempts = stage.agentExecutions.filter { $0.agentID == "proposal_reviewer_ui" }
        #expect(attempts.count == 1)
        let agentExec = try #require(attempts.first)
        #expect(agentExec.status == .completed)
        #expect(agentExec.resolvedModel == "gemini-2.5-flash")
    }

    @Test("Gemini capacity exhaustion reruns parallel execution with a stable fallback model")
    func geminiCapacityFallbackRerunsParallelExecutionWithStableModel() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "proposal_reviewer_ux": ResolvedProviderBinding(
                agentID: "proposal_reviewer_ux",
                backendProfileID: "gemini_3_1_pro_preview_high",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.geminiACP.rawValue,
                providerIdentifier: ProviderFamily.geminiACP.runtimeProviderIdentifier,
                model: "gemini-3.1-pro-preview",
                effort: "medium",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "gemini_cli_acp",
                adapterFamily: "gemini_cli_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let agent = ResolvedAgent(
            id: "proposal_reviewer_ux",
            title: "Proposal Reviewer / UX",
            mode: "proposal_review.ux",
            backendProfileID: "gemini_3_1_pro_preview_high",
            provider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            model: "gemini-3.1-pro-preview",
            effort: "medium",
            maxTurns: 12,
            temperature: 0.2,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: nil,
            prompt: "review proposal",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_ux"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "proposal_reviewer_ux",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([AgentTask(agent: "proposal_reviewer_ux", task: "review_ux", inputs: nil, outputs: ["proposal_review_ux"])])
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
                    ownerAgentID: "proposal_reviewer_ux",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["proposal_reviewer_ux": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let firstFailure = AgentResult(
            outputs: [:],
            logSnippet: "capacity exhausted",
            costCents: nil,
            succeeded: false,
            errorMessage: "No capacity available for model gemini-3.1-pro-preview on the server",
            sessionID: "session-1",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gemini-2.5-flash",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .limitExhaustedBeforeOutput,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .provider,
            providerStopReason: "model_capacity_exhausted",
            outputPresence: .none,
            runtimeProvider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            runtimeModel: "gemini-2.5-flash"
        )
        let secondSuccess = AgentResult(
            outputs: ["proposal_review_ux": Data("ok".utf8)],
            logSnippet: "fallback success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-2",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gemini-3.1-pro-preview",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.geminiACP.runtimeProviderIdentifier,
            runtimeModel: "gemini-3.1-pro-preview"
        )
        let box = SequencedExecutionBox(results: [firstFailure, secondSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(await box.models == ["gemini-3.1-pro-preview", "gemini-2.5-flash"])

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let attempts = stage.agentExecutions.filter { $0.agentID == "proposal_reviewer_ux" }
        #expect(attempts.count == 1)
        let agentExec = try #require(attempts.first)
        #expect(agentExec.status == .completed)
        #expect(agentExec.resolvedModel == "gemini-2.5-flash")
    }

    @Test("Sequential watchdog failures create durable same-stage retry lineage before succeeding")
    func sequentialWatchdogFailureCreatesDurableRetryLineage() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent(id: "agent_1", outputs: ["output_1"])

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "agent_1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "agent_1", task: "do_work", inputs: nil, outputs: ["output_1"])])
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
                    ownerAgentID: "agent_1",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["agent_1": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let firstFailure = AgentResult(
            outputs: [:],
            logSnippet: "stalled after first edit",
            costCents: nil,
            succeeded: false,
            errorMessage: "Execution stalled after first edit without verified filesystem side effects",
            sessionID: "session-1",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .failedBeforeOutput,
            supervisionClassification: .idleHangAfterFirstEdit,
            sessionReuseDisposition: .fresh,
            outputPresence: .none,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )
        let secondSuccess = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-2",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )
        let box = SequencedExecutionBox(results: [firstFailure, secondSuccess])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let attempts = stage.agentExecutions
            .filter { $0.agentID == "agent_1" && $0.taskName == "do_work" }
            .sorted { ($0.agentAttemptNumber ?? 1) < ($1.agentAttemptNumber ?? 1) }

        #expect(attempts.count == 2)
        let firstAttempt = try #require(attempts.first)
        let retryAttempt = try #require(attempts.last)
        #expect(firstAttempt.status == .failed)
        #expect(firstAttempt.supervisionClassification == .idleHangAfterFirstEdit)
        #expect(retryAttempt.status == .completed)
        #expect(retryAttempt.retryReason == "automatic_watchdog_retry")
        #expect(retryAttempt.agentAttemptNumber == 2)
        #expect(retryAttempt.supersedesAgentExecutionID == firstAttempt.id)
        #expect(retryAttempt.stageExecution?.id == stage.id)
    }

    @Test("Implementation loop does not downgrade Codex writer to fast tier after a no-progress attempt")
    func implementationLoopDoesNotDowngradeCodexWriterAfterNoProgressAttempt() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "code_writer": ResolvedProviderBinding(
                agentID: "code_writer",
                backendProfileID: "bp",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let priorStage = StageExecution(
            stageID: "start",
            label: "Start",
            startedAt: Date(timeIntervalSince1970: 1),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        priorStage.lineageID = "start"
        priorStage.run = run
        context.insert(priorStage)

        let priorExecution = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "implement",
            startedAt: Date(timeIntervalSince1970: 1),
            status: .failed,
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            effort: "high"
        )
        priorExecution.completedAt = Date(timeIntervalSince1970: 2)
        priorExecution.modelTierUsed = "fast"
        priorExecution.canonicalOutcome = .timedOutBeforeOutput
        priorExecution.transportErrorKind = .timeout
        priorExecution.outputPresence = .none
        priorExecution.stageExecution = priorStage
        context.insert(priorExecution)

        let previousArtifactRoot = workspace.artifactRoot
            .appendingPathComponent("prior-no-progress", isDirectory: true)
        try FileManager.default.createDirectory(at: previousArtifactRoot, withIntermediateDirectories: true)

        let changedFilesURL = previousArtifactRoot.appendingPathComponent("changed_files_manifest")
        try """
        {
          "files" : [],
          "summary" : "Execution stopped before producing a changed-files report."
        }
        """.write(to: changedFilesURL, atomically: true, encoding: .utf8)

        let progressURL = previousArtifactRoot.appendingPathComponent("implementation_progress")
        try """
        {
          "completed_items" : [],
          "current_phase" : "implementation",
          "deferred_items" : ["changed_files_manifest", "tests_result"],
          "status" : "blocked"
        }
        """.write(to: progressURL, atomically: true, encoding: .utf8)

        let changedFilesArtifact = Artifact(
            name: "changed_files_manifest",
            contractID: "changed_files_manifest",
            format: .json,
            filePath: changedFilesURL.path,
            runID: run.id,
            stageID: "start",
            agentID: "code_writer",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            attemptNumber: 1
        )
        changedFilesArtifact.agentExecution = priorExecution
        context.insert(changedFilesArtifact)

        let progressArtifact = Artifact(
            name: "implementation_progress",
            contractID: "implementation_progress",
            format: .json,
            filePath: progressURL.path,
            runID: run.id,
            stageID: "start",
            agentID: "code_writer",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            attemptNumber: 1
        )
        progressArtifact.agentExecution = priorExecution
        context.insert(progressArtifact)

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            backendProfileID: "bp",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "CODE_WRITE",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "implement",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output_1"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "implement", inputs: nil, outputs: ["output_1"])])
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
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let success = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let box = SequencedExecutionBox(results: [success])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        let models = await box.models
        #expect(models == ["GPT-5.4"])

        let latestExecution = try #require(
            run.stageExecutions
                .flatMap(\.agentExecutions)
                .filter { $0.agentID == "code_writer" && $0.startedAt > priorExecution.startedAt }
                .sorted { $0.startedAt < $1.startedAt }
                .last
        )
        #expect(latestExecution.modelTierUsed == "frontier")
    }

    @Test("Implementation loop does not downgrade Codex writer to fast tier after a usage-limit attempt")
    func implementationLoopDoesNotDowngradeCodexWriterAfterUsageLimitAttempt() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "code_writer": ResolvedProviderBinding(
                agentID: "code_writer",
                backendProfileID: "bp",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let priorStage = StageExecution(
            stageID: "start",
            label: "Start",
            startedAt: Date(timeIntervalSince1970: 1),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        priorStage.lineageID = "start"
        priorStage.run = run
        context.insert(priorStage)

        let priorExecution = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "implement",
            startedAt: Date(timeIntervalSince1970: 1),
            status: .failed,
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            effort: "high"
        )
        priorExecution.completedAt = Date(timeIntervalSince1970: 2)
        priorExecution.modelTierUsed = "fast"
        priorExecution.providerStopReason = "usage_limit_exceeded"
        priorExecution.canonicalOutcome = .limitExhaustedBeforeOutput
        priorExecution.transportErrorKind = .provider
        priorExecution.outputPresence = .none
        priorExecution.stageExecution = priorStage
        context.insert(priorExecution)

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            backendProfileID: "bp",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "CODE_WRITE",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "implement",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output_1"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "implement", inputs: nil, outputs: ["output_1"])])
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
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let success = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let box = SequencedExecutionBox(results: [success])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        let models = await box.models
        #expect(models == ["GPT-5.4"])

        let latestExecution = try #require(
            run.stageExecutions
                .flatMap(\.agentExecutions)
                .filter { $0.agentID == "code_writer" && $0.startedAt > priorExecution.startedAt }
                .sorted { $0.startedAt < $1.startedAt }
                .last
        )
        #expect(latestExecution.modelTierUsed == "frontier")
    }

    @Test("Implementation loop does not reuse Codex fast tier after a usage-limit retry in the same stage")
    func implementationLoopDoesNotReuseCodexFastTierAfterSameStageUsageLimitRetry() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.contextStrategyProfileID = "selective_compression_and_escalation"
        run.strategyAssignmentMode = "manual_override"
        run.contextStrategySnapshotJSON = try JSONEncoder().encode(
            try #require(
                StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                    .runtimeProfile(profileID: "selective_compression_and_escalation")
            )
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "code_writer": ResolvedProviderBinding(
                agentID: "code_writer",
                backendProfileID: "bp",
                configuredProviderID: UUID(),
                providerFamily: ProviderFamily.codexACP.rawValue,
                providerIdentifier: ProviderFamily.codexACP.runtimeProviderIdentifier,
                model: "GPT-5.4",
                effort: "high",
                transport: "cli",
                adapterVersion: "v1",
                runtimeProfileID: "codex_acp",
                adapterFamily: "codex_acp",
                capabilityClass: .operatorGrade
            )
        ])

        let resumableStage = StageExecution(
            stageID: "start",
            label: "Start",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        resumableStage.lineageID = "start"
        resumableStage.run = run
        context.insert(resumableStage)

        let priorExecution = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "implement",
            startedAt: Date(timeIntervalSince1970: 11),
            status: .failed,
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            effort: "high"
        )
        priorExecution.completedAt = Date(timeIntervalSince1970: 12)
        priorExecution.modelTierUsed = "fast"
        priorExecution.providerStopReason = "usage_limit_exceeded"
        priorExecution.canonicalOutcome = .limitExhaustedBeforeOutput
        priorExecution.transportErrorKind = .provider
        priorExecution.outputPresence = .none
        priorExecution.stageExecution = resumableStage
        context.insert(priorExecution)

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            backendProfileID: "bp",
            provider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "CODE_WRITE",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "implement",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output_1"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "code_writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "code_writer", task: "implement", inputs: nil, outputs: ["output_1"])])
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
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let success = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "success",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: ProviderFamily.codexACP.runtimeProviderIdentifier,
            runtimeModel: "GPT-5.4"
        )
        let box = SequencedExecutionBox(results: [success])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SequencedExecutor(box: box),
            modelContext: context
        )

        await orchestrator.start()

        #expect(await box.models == ["GPT-5.4"])

        let latestExecution = try #require(
            resumableStage.agentExecutions
                .filter { $0.agentID == "code_writer" && $0.startedAt > priorExecution.startedAt }
                .sorted { $0.startedAt < $1.startedAt }
                .last
        )
        #expect(latestExecution.modelTierUsed == "frontier")
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

    @Test("Downstream stage materialization is durably visible before first agent result")
    func downstreamStageMaterializationIsDurableBeforeAgentWork() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        try context.save()

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "s1": ExecutableState(
                    id: "s1", label: "Stage 1", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "first_task", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "s2", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "s2": ExecutableState(
                    id: "s2", label: "Stage 2", type: nil,
                    ownerAgentID: "a2",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a2", task: "second_task", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a2", runBlock: nil, runAfterApproval: nil,
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

        let box = BlockingSecondTaskBox()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: BlockingSecondTaskExecutor(box: box), modelContext: context
        )

        let runTask = Task { @MainActor in
            await orchestrator.start()
        }

        await box.waitUntilSecondTaskStarts()

        let freshContext = ModelContext(container)
        let freshRuns = try freshContext.fetch(FetchDescriptor<Run>())
        let persistedRun = try #require(freshRuns.first(where: { $0.id == run.id }))
        #expect(persistedRun.transitionCursor?.settlementPhase == .transitionStarted)
        #expect(
            persistedRun.stageExecutions.contains(where: {
                $0.stageID == "s2" && $0.status == .running
            })
        )

        await box.releaseSecondTask()
        await runTask.value
    }

    @Test("Sequential agent execution is durably visible before first agent result")
    func sequentialAgentExecutionIsDurableBeforeFirstResult() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        try context.save()

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "blocking_task", inputs: nil, outputs: nil)])
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

        let box = BlockingTaskStartBox(expectedStartCount: 1)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: BlockingTaskStartExecutor(box: box),
            modelContext: context
        )

        let runTask = Task { @MainActor in
            await orchestrator.start()
        }

        await box.waitUntilExpectedTasksStart()

        let freshContext = ModelContext(container)
        let persistedRun = try #require(try freshContext.fetch(FetchDescriptor<Run>()).first(where: { $0.id == run.id }))
        let persistedStage = try #require(persistedRun.stageExecutions.first(where: { $0.stageID == "start" }))
        #expect(persistedStage.status == .running)
        #expect(persistedStage.agentExecutions.count == 1)
        #expect(persistedStage.agentExecutions.first?.status == .running)

        await box.releaseAll()
        await runTask.value
    }

    @Test("Parallel agent executions are durably visible before first agent result")
    func parallelAgentExecutionsAreDurablyVisibleBeforeFirstResult() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        try context.save()

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "a1", task: "parallel_one", inputs: nil, outputs: nil),
                            AgentTask(agent: "a2", task: "parallel_two", inputs: nil, outputs: nil)
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

        let box = BlockingTaskStartBox(expectedStartCount: 2)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: BlockingTaskStartExecutor(box: box),
            modelContext: context
        )

        let runTask = Task { @MainActor in
            await orchestrator.start()
        }

        await box.waitUntilExpectedTasksStart()

        let freshContext = ModelContext(container)
        let persistedRun = try #require(try freshContext.fetch(FetchDescriptor<Run>()).first(where: { $0.id == run.id }))
        let persistedStage = try #require(persistedRun.stageExecutions.first(where: { $0.stageID == "start" }))
        #expect(persistedStage.status == .running)
        #expect(persistedStage.agentExecutions.count == 2)
        #expect(persistedStage.agentExecutions.allSatisfy { $0.status == .running })

        await box.releaseAll()
        await runTask.value
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

    @Test("Parallel reviewer failures persist receipt and transcript evidence")
    func parallelFailurePersistsFailureEvidence() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let architectAgent = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Architect",
            mode: "tool_use",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "sk-architect",
            skillRole: nil,
            prompt: "Review the proposal as an architect.",
            outputContract: "proposal_review_v1",
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_architect"]
        )
        let poAgent = makeAgent(id: "proposal_reviewer_product_owner", outputs: ["proposal_review_po"])

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "proposal_reviewer_architect", task: "review_architecture", inputs: nil, outputs: ["proposal_review_architect"]),
                            AgentTask(agent: "proposal_reviewer_product_owner", task: "review_scope", inputs: nil, outputs: ["proposal_review_po"])
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_reviewer_architect", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "proposal_reviewer_architect": architectAgent,
                "proposal_reviewer_product_owner": poAgent
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let failedArchitect = AgentResult(
            outputs: [
                "proposal_reviewer_architect_receipt.json": Data("{\"receipt\":true}".utf8),
                "proposal_reviewer_architect_transcript.md": Data("# transcript".utf8)
            ],
            logSnippet: "Architect stream ended without the required artifact",
            costCents: nil,
            succeeded: false,
            errorMessage: "Required outputs missing: proposal_review_architect",
            sessionID: "session-architect",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "opus",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .failedBeforeOutput,
            sessionReuseDisposition: .fresh,
            outputPresence: .none,
            runtimeProvider: "claude_code",
            runtimeModel: "opus"
        )
        let successfulPO = AgentResult(
            outputs: ["proposal_review_po": Data("{\"decision\":\"approve\"}".utf8)],
            logSnippet: "ok",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-po",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "opus",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "claude_code",
            runtimeModel: "opus"
        )
        let executor = AgentResultExecutor(
            box: AgentResultBox(
                resultsByAgentID: [
                    "proposal_reviewer_architect": failedArchitect,
                    "proposal_reviewer_product_owner": successfulPO
                ]
            )
        )

        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        let failedStage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let failedAgentExec = try #require(failedStage.agentExecutions.first(where: { $0.agentID == "proposal_reviewer_architect" }))
        let evidence = try #require(
            failedStage.evidencePacketJSON.flatMap {
                try? JSONDecoder().decode(FailedStageEvidencePacket.self, from: $0)
            }
        )

        #expect(failedAgentExec.status == .failed)
        #expect(failedAgentExec.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_receipt.json" }))
        #expect(failedAgentExec.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_transcript.md" }))
        #expect(evidence.failureSummary == "Required outputs missing: proposal_review_architect")
        #expect(evidence.receiptExists)
        #expect(evidence.transcriptExists)
    }

    @Test("Parallel success-path validation failures preserve raw evidence")
    func parallelSuccessPathValidationFailurePreservesRawEvidence() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let architectAgent = makeAgent(id: "proposal_reviewer_architect", outputs: ["proposal_review_architect"])
        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "proposal_reviewer_architect", task: "review_architecture", inputs: nil, outputs: ["proposal_review_architect"])
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_reviewer_architect", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "proposal_reviewer_architect": architectAgent
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let malformedArchitect = AgentResult(
            outputs: [
                "proposal_review_architect": Data("{\"agent_id\":\"proposal_reviewer_architect\"}".utf8),
                "proposal_reviewer_architect_receipt.json": Data("{\"receipt\":true}".utf8),
                "proposal_reviewer_architect_transcript.md": Data("# transcript".utf8)
            ],
            logSnippet: "Architect returned malformed review output",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-architect",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: malformedArchitect),
            modelContext: context,
            catalog: makeReviewCatalog()
        )

        await orchestrator.start()

        let failedStage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let failedAgentExec = try #require(failedStage.agentExecutions.first(where: { $0.agentID == "proposal_reviewer_architect" }))
        #expect(failedAgentExec.status == .failed)
        #expect(failedAgentExec.artifacts.contains(where: { $0.name == "proposal_review_architect" }))
        #expect(failedAgentExec.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_receipt.json" }))
        #expect(failedAgentExec.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_transcript.md" }))
        #expect((failedAgentExec.logSnippet ?? "").contains("Output contract validation failed"))
    }

    @Test("Parallel success-path ignores auxiliary receipt and transcript artifacts during contract validation")
    func parallelSuccessPathIgnoresAuxiliaryArtifactsDuringValidation() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let architectAgent = makeAgent(id: "proposal_reviewer_architect", outputs: ["proposal_review_architect"])
        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "proposal_reviewer_architect", task: "review_architecture", inputs: nil, outputs: ["proposal_review_architect"])
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_reviewer_architect", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "proposal_reviewer_architect": architectAgent
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let validArchitect = AgentResult(
            outputs: [
                "proposal_review_architect": Data("""
                {"agent_id":"proposal_reviewer_architect","role":"architect","score":5,"decision":"revise","verdict":"Needs revision","summary":"Architectural gaps remain.","issues":[],"blocking_issues":[],"non_blocking_issues":[],"suggestions":[],"assumptions":[]}
                """.utf8),
                "proposal_reviewer_architect_receipt.json": Data("{\"receipt\":true}".utf8),
                "proposal_reviewer_architect_transcript.md": Data("# transcript".utf8)
            ],
            logSnippet: "Architect returned valid review output",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "session-architect",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: validArchitect),
            modelContext: context,
            catalog: makeReviewCatalog()
        )

        await orchestrator.start()

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let agentExecution = try #require(stage.agentExecutions.first(where: { $0.agentID == "proposal_reviewer_architect" }))
        #expect(stage.status == .completed)
        #expect(agentExecution.status == .completed)
        #expect(agentExecution.validationFailureJSON == nil)
        #expect(agentExecution.artifacts.contains(where: { $0.name == "proposal_review_architect" }))
        #expect(agentExecution.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_receipt.json" }))
        #expect(agentExecution.artifacts.contains(where: { $0.name == "proposal_reviewer_architect_transcript.md" }))
    }

    @Test("Parallel transport timeout after durable output still completes when contract output validates")
    func parallelTimeoutAfterOutputWithValidContractOutputCompletesStage() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let architectAgent = makeAgent(id: "proposal_reviewer_architect", outputs: ["proposal_review_architect"])
        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "proposal_reviewer_architect",
                    runBlock: ExecutableRunBlock(phases: [
                        .parallel([
                            AgentTask(agent: "proposal_reviewer_architect", task: "review_architecture", inputs: nil, outputs: ["proposal_review_architect"])
                        ])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "proposal_reviewer_architect", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: [
                "proposal_reviewer_architect": architectAgent
            ],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let timedOutAfterOutput = AgentResult(
            outputs: [
                "proposal_review_architect": Data("""
                {"agent_id":"proposal_reviewer_architect","role":"architect","score":8,"decision":"approve","verdict":"Looks good","summary":"Contract output is valid.","issues":[],"blocking_issues":[],"non_blocking_issues":[],"suggestions":[],"assumptions":[]}
                """.utf8)
            ],
            logSnippet: "Execution produced output before timing out",
            costCents: nil,
            succeeded: false,
            errorMessage: "The request timed out.",
            sessionID: "session-architect",
            durationSeconds: 1,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .timedOutAfterOutput,
            sessionReuseDisposition: .fresh,
            transportErrorKind: .timeout,
            outputPresence: .durableOutput,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: timedOutAfterOutput),
            modelContext: context,
            catalog: makeReviewCatalog()
        )

        await orchestrator.start()

        let stage = try #require(run.stageExecutions.first(where: { $0.stageID == "start" }))
        let agentExecution = try #require(stage.agentExecutions.first(where: { $0.agentID == "proposal_reviewer_architect" }))
        #expect(run.status == .completed)
        #expect(stage.status == .completed)
        #expect(agentExecution.status == .completed)
        #expect(agentExecution.canonicalOutcome == .completedWithTransportError)
        #expect(agentExecution.transportErrorKind == .timeout)
        #expect(agentExecution.artifacts.contains(where: { $0.name == "proposal_review_architect" }))
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

        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "live-session-001",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: #"{"session_id":"live-session-001"}"#),
                .promptSubmitted(raw: #"{"request_id":"request-123"}"#),
                .toolCallStarted(toolName: "read_artifact", raw: "{}"),
                .textChunk(text: "Drafting proposal..."),
                .finalOutput(content: "{\"proposal\":\"ready\"}"),
                .sessionClosed(raw: "{}")
            ]
        )
        let executor = RuntimeAgentExecutor(transport: transport)
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
        #expect(agentExecution?.runtimeSessionID == "live-session-001")
        #expect(agentExecution?.logSnippet?.contains("Final output") == true)
        #expect(agentExecution?.transcriptArtifactPath != nil)
        if let consumed = agentExecution?.consumedInputArtifactNamesJSON {
            let names = try? JSONDecoder().decode([String].self, from: consumed)
            #expect(names == [])
        } else {
            Issue.record("Expected consumed input artifact names to be captured")
        }
    }

    @Test("Orchestrator creates the cursor-scheduled stage iteration instead of reusing a stale running stage")
    func scheduledIterationBeatsStaleRunningStageReuse() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let staleStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        staleStage.run = run
        context.insert(staleStage)

        run.persistTransitionCursor(
            TransitionCursor.initial().settlingTransition(
                completedStateID: "state_1_idea_received",
                completedStageExecutionID: nil,
                nextStateID: "state_2_proposal_drafted",
                nextIteration: 2,
                nextAttemptNumber: 1
            )
        )

        let plan = RunPlan(
            workflowID: "proposal_loop_live", workflowTitle: "Proposal Loop (Live)",
            states: [
                "state_2_proposal_drafted": ExecutableState(
                    id: "state_2_proposal_drafted",
                    label: "Proposal drafted",
                    type: .start,
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
                    ownerAgentID: "proposal_writer",
                    runBlock: nil, runAfterApproval: nil, transitions: [],
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                )
            ],
            initialStateID: "state_2_proposal_drafted",
            agentBindings: [
                "proposal_writer": makeAgent(
                    id: "proposal_writer",
                    backendProfileID: "codex_writer_high",
                    outputs: ["proposal_current"]
                )
            ],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let result = AgentResult(
            outputs: ["proposal_current": Data("ok".utf8)],
            logSnippet: "done",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: "scheduled-stage-session",
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: "gpt-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput,
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context
        )

        await orchestrator.start(from: "state_2_proposal_drafted")

        let draftedStages = run.stageExecutions
            .filter { $0.stageID == "state_2_proposal_drafted" }
            .sorted { $0.iteration < $1.iteration }

        #expect(draftedStages.count == 2)
        #expect(draftedStages.first?.iteration == 1)
        #expect(draftedStages.last?.iteration == 2)
        #expect(draftedStages.first?.agentExecutions.isEmpty == true)
        #expect(draftedStages.last?.status == .completed)
        #expect(draftedStages.last?.agentExecutions.first?.providerSessionID == "scheduled-stage-session")
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
                providerFamily: "claude_acp",
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
            sessionLineageID: nil,
            sessionGenerationID: nil,
            sessionReuseDisposition: .fresh,
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

    @Test("Orchestrator coalesces rapid live text chunks before routing them into observable state")
    func coalescesRapidLiveTextChunks() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent(id: "agent_1")

        let plan = RunPlan(
            workflowID: "wf_live_chunk_coalescing",
            workflowTitle: "Workflow Live Chunk Coalescing",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [agent.id: agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "hash",
            catalogSnapshotHash: "catalog",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let result = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "unused",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context
        )

        let baseTime = Date()
        let firstChunk = ExecutionEvent(type: .textChunk, timestamp: baseTime, detail: "chunk-1")
        let secondChunk = ExecutionEvent(type: .textChunk, timestamp: baseTime.addingTimeInterval(0.05), detail: "chunk-2")
        let laterChunk = ExecutionEvent(type: .textChunk, timestamp: baseTime.addingTimeInterval(0.60), detail: "chunk-3")
        let finishEvent = ExecutionEvent(type: .finish, timestamp: baseTime.addingTimeInterval(0.61), detail: "Finish: stop")

        #expect(orchestrator.shouldRecordLiveExecutionEvent(agentID: agent.id, event: firstChunk, now: baseTime))
        #expect(
            orchestrator.shouldRecordLiveExecutionEvent(
                agentID: agent.id,
                event: secondChunk,
                now: baseTime.addingTimeInterval(0.05)
            ) == false
        )
        #expect(
            orchestrator.shouldRecordLiveExecutionEvent(
                agentID: agent.id,
                event: laterChunk,
                now: baseTime.addingTimeInterval(0.60)
            )
        )
        #expect(
            orchestrator.shouldRecordLiveExecutionEvent(
                agentID: agent.id,
                event: finishEvent,
                now: baseTime.addingTimeInterval(0.61)
            )
        )
        #expect(
            orchestrator.shouldRecordLiveExecutionEvent(
                agentID: agent.id,
                event: firstChunk,
                now: baseTime.addingTimeInterval(0.62)
            )
        )
    }

    @Test("Buffered live text chunks flush into a single rolling timeline entry")
    func bufferedLiveTextChunksFlushIntoSingleRollingTimelineEntry() {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent(id: "agent_1")

        let plan = RunPlan(
            workflowID: "wf_live_chunk_buffering",
            workflowTitle: "Workflow Live Chunk Buffering",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [agent.id: agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "hash",
            catalogSnapshotHash: "catalog",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let result = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "unused",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context
        )

        let base = Date()
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base, detail: "Hello"),
            now: base
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(0.05), detail: " "),
            now: base.addingTimeInterval(0.05)
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(0.10), detail: "world"),
            now: base.addingTimeInterval(0.10)
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .finish, timestamp: base.addingTimeInterval(0.11), detail: "Finish: stop"),
            now: base.addingTimeInterval(0.11)
        )

        let textEntries = orchestrator.liveTimeline.filter { $0.event.type == .textChunk }
        #expect(textEntries.count == 1)
        #expect(textEntries.first?.event.detail == "Hello world")
    }

    @Test("Structured output envelopes do not leak into live timeline text")
    func structuredOutputEnvelopesDoNotLeakIntoLiveTimelineText() {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let agent = makeAgent(id: "agent_1")

        let plan = RunPlan(
            workflowID: "wf_structured_output_suppression",
            workflowTitle: "WF Structured Output Suppression",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [agent.id: agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "hash",
            catalogSnapshotHash: "catalog",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let result = AgentResult(
            outputs: ["output_1": Data("ok".utf8)],
            logSnippet: "unused",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: StaticResultExecutor(result: result),
            modelContext: context
        )

        let base = Date()
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base, detail: "Visible progress. "),
            now: base
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(0.60), detail: "<<<CHAINWORKS_OUTPUT:implementation_progress>>>"),
            now: base.addingTimeInterval(0.60)
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(1.20), detail: "{\"status\":\"in_progress\"}"),
            now: base.addingTimeInterval(1.20)
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(1.80), detail: "<<<END_CHAINWORKS_OUTPUT>>>"),
            now: base.addingTimeInterval(1.80)
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: agent.id,
            event: ExecutionEvent(type: .textChunk, timestamp: base.addingTimeInterval(2.40), detail: "Next visible progress."),
            now: base.addingTimeInterval(2.40)
        )

        let timelineText = orchestrator.liveTimeline
            .filter { $0.event.type == .textChunk }
            .map(\.event.detail)
            .joined()

        #expect(timelineText.contains("Visible progress."))
        #expect(timelineText.contains("Next visible progress."))
        #expect(timelineText.contains("CHAINWORKS_OUTPUT") == false)
        #expect(timelineText.contains("\"status\":\"in_progress\"") == false)
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
            sessionLineageID: nil,
            sessionGenerationID: nil,
            sessionReuseDisposition: .fresh,
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

    @Test("Orchestrator heals premature blocked state after successful stage completion")
    func healsPrematureBlockedStateAfterSuccessfulStageCompletion() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "a1", task: "write", inputs: nil, outputs: ["output_1"])])
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

        final class PrematureBlockExecutor: @unchecked Sendable, AgentExecutor {
            let run: Run

            init(run: Run) {
                self.run = run
            }

            func execute(
                task: AgentTask,
                agent: ResolvedAgent,
                context: ExecutionContext
            ) async throws -> AgentResult {
                run.status = .blocked
                run.driftDetails = "Execution stalled after the last session closed before the workflow could transition. Resume the run to continue from the current stage."
                return AgentResult(
                    outputs: ["output_1": Data("ok".utf8)],
                    logSnippet: "completed after false block",
                    costCents: nil,
                    succeeded: true,
                    errorMessage: nil,
                    sessionID: "sim-heal",
                    durationSeconds: 0.01,
                    providerReceipt: nil,
                    resolvedModel: "test-model",
                    configuredProviderID: nil,
                    adapterVersion: nil,
                    canonicalOutcome: .completed,
                    sessionReuseDisposition: .fresh
                )
            }
        }

        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: PrematureBlockExecutor(run: run), modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(run.driftDetails == nil)
        #expect(run.stageExecutions.contains { $0.stageID == "start" && $0.status == .completed })
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

    // MARK: - Approval Rejection

    @Test("Approval rejection follows rejected transition")
    func approvalRejectedFollowsRejectedTransition() async {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Approval Gate", type: .start,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [
                        ExecutableTransition(to: "end", condition: .approvalGranted),
                        ExecutableTransition(to: "refine", condition: .approvalRejected)
                    ],
                    approvalRequired: true, approvalPolicy: nil, loop: nil
                ),
                "refine": ExecutableState(
                    id: "refine", label: "Refine", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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

        await awaitCondition("Run should follow rejected approval transition", timeout: 3.0) {
            run.status == .completed
        }
        #expect(orchestrator.currentStateID == "refine")
        #expect(!orchestrator.isCancelled, "Rejected approval must not cancel the workflow when a rejected transition exists")
        #expect(completionCalled, "onComplete must fire after rejected transition reaches an end state")
        #expect(completionSuccess, "onComplete should report success after rejected transition reaches an end state")

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
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
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

    @Test("Implementation partial artifact set recovers failed code writer into continue path")
    func implementationPartialArtifactSetRecoversFailedCodeWriter() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        run.implementationSelfAssessmentSummaryJSON = try JSONSerialization.data(withJSONObject: [
            "status": "needs_code_fixes",
            "implementation_complete": false,
            "verification_green": false,
            "remaining_code_task_count": 2,
            "blocking_remaining_code_task_count": 2,
            "handoff_task_count": 0,
            "blocking_review_handoff_task_count": 0,
            "remaining_code_tasks": [
                [
                    "summary": "Resume implementation",
                    "owner": "code_writer",
                    "blocking": true,
                    "evidence": "Execution stopped before canonical test reporting.",
                    "source_pointer": "/remaining_code_tasks/0"
                ],
                [
                    "summary": "Run verification",
                    "owner": "code_writer",
                    "blocking": true,
                    "evidence": "tests_result was missing from primary execution.",
                    "source_pointer": "/remaining_code_tasks/1"
                ]
            ],
            "handoff_tasks": [],
            "known_risks": ["Execution stopped before canonical test report was written."],
            "tests_run": [],
            "docs_impacted": [],
            "owner_class_counts": [String: Int](),
            "target_stage_summaries": [],
            "validation_errors": [],
            "warnings": []
        ], options: [.sortedKeys])
        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            provider: "codex",
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 12,
            temperature: 0,
            permissionProfile: "CODE_WRITE",
            skillRef: "code_writer_core",
            skillRole: nil,
            prompt: "Implement the approved proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: [
                "implementation_progress",
                "implementation_self_assessment",
                "changed_files_manifest",
                "tests_result"
            ],
            worktreeWriteEnabled: true
        )

        let partialOutputs: [String: Data] = [
            "implementation_progress": try JSONSerialization.data(withJSONObject: [
                "status": "blocked",
                "current_phase": "implementation",
                "completed_items": ["Partial worktree edits preserved."],
                "deferred_items": ["tests_result"],
                "notes": "Execution stopped before canonical test reporting."
            ], options: [.sortedKeys]),
            "implementation_self_assessment": try JSONSerialization.data(withJSONObject: [
                "implementation_complete": false,
                "verification_green": false,
                "remaining_code_tasks": [
                    [
                        "summary": "Resume implementation",
                        "owner": "code_writer",
                        "blocking": true,
                        "evidence": "Execution stopped before canonical test reporting."
                    ],
                    [
                        "summary": "Run verification",
                        "owner": "code_writer",
                        "blocking": true,
                        "evidence": "tests_result was missing from primary execution."
                    ]
                ],
                "handoff_tasks": [],
                "known_risks": ["Execution stopped before canonical test report was written."],
                "tests_run": [],
                "docs_impacted": []
            ], options: [.sortedKeys]),
            "changed_files_manifest": try JSONSerialization.data(withJSONObject: [
                "files": ["Sources/App.swift"]
            ], options: [.sortedKeys]),
            "tests_result": try JSONSerialization.data(withJSONObject: [
                "status": "blocked",
                "summary": "Execution stopped before canonical test reporting."
            ], options: [.sortedKeys])
        ]

        let failedResult = AgentResult(
            outputs: partialOutputs,
            logSnippet: "partial implementation artifacts preserved",
            costCents: 1,
            succeeded: false,
            errorMessage: "Required outputs missing from primary execution; recovered from partial artifact set",
            sessionID: "code-writer-session",
            durationSeconds: 1.0,
            providerReceipt: nil,
            resolvedModel: "GPT-5.4",
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .failedBeforeOutput,
            supervisionClassification: .mutationSideEffectMissing,
            sessionReuseDisposition: .fresh,
            outputPresence: .durableOutput
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
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
                    transitions: [
                        ExecutableTransition(to: "continue", condition: .expression("implementation_self_assessment_v2.status == 'needs_code_fixes'")),
                        ExecutableTransition(to: "end", condition: .expression("implementation_self_assessment_v2.status == 'complete'"))
                    ],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                ),
                "continue": ExecutableState(
                    id: "continue",
                    label: "Continue",
                    type: nil,
                    ownerAgentID: "code_writer",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
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
            executor: StaticResultExecutor(result: failedResult),
            modelContext: context,
            catalog: makeImplementationCatalog()
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(run.currentStageID == "continue")
        let stageExecutionsByID = Dictionary(uniqueKeysWithValues: run.stageExecutions.map { ($0.stageID, $0) })
        #expect(stageExecutionsByID["start"]?.status == .completed)
        #expect(stageExecutionsByID["continue"]?.status == .completed)
        #expect(stageExecutionsByID["end"] == nil)
        let stageExec = try #require(stageExecutionsByID["start"])
        let agentExec = try #require(stageExec.agentExecutions.first)
        #expect(agentExec.status == .completed)
        #expect(agentExec.canonicalOutcome == .completedWithTransportError)
        #expect(agentExec.supervisionClassification == .mutationSideEffectMissing)
        let artifactNames = Set(agentExec.artifacts.map(\.name))
        #expect(artifactNames.contains("implementation_progress"))
        #expect(artifactNames.contains("implementation_self_assessment"))
        #expect(artifactNames.contains("changed_files_manifest"))
        #expect(artifactNames.contains("tests_result"))
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

    @Test("End state with run block executes terminal work before completion")
    func endStateWithRunBlockExecutesBeforeCompletion() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let terminalAgent = makeAgent(id: "terminal_writer", outputs: ["terminal_receipt"])
        let terminalResult = AgentResult(
            outputs: ["terminal_receipt": Data(#"{"status":"done"}"#.utf8)],
            logSnippet: "done",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
        )
        let endState = ExecutableState(
            id: "end",
            label: "Workflow complete",
            type: .end,
            ownerAgentID: "terminal_writer",
            runBlock: ExecutableRunBlock(phases: [
                .sequential([AgentTask(agent: "terminal_writer", task: "finalize", inputs: nil, outputs: nil)])
            ]),
            runAfterApproval: nil,
            transitions: [],
            approvalRequired: false,
            approvalPolicy: nil,
            loop: nil
        )
        let plan = RunPlan(
            workflowID: "terminal_work",
            workflowTitle: "Terminal Work",
            states: ["end": endState],
            initialStateID: "end",
            agentBindings: ["terminal_writer": terminalAgent],
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
            executor: StaticResultExecutor(result: terminalResult),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(run.currentStageID == "end")
        let endStage = try #require(run.stageExecutions.first(where: { $0.stageID == "end" }))
        #expect(endStage.status == .completed)
        let artifactNames = Set(endStage.agentExecutions.flatMap(\.artifacts).map(\.name))
        #expect(artifactNames.contains("terminal_receipt"))
    }

    @Test("End state with terminal work ignores self-loop transitions and completes")
    func endStateWithSelfLoopStillCompletes() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)
        let terminalAgent = makeAgent(id: "terminal_writer", outputs: ["terminal_receipt"])
        let terminalResult = AgentResult(
            outputs: ["terminal_receipt": Data(#"{"status":"done"}"#.utf8)],
            logSnippet: "done",
            costCents: nil,
            succeeded: true,
            errorMessage: nil,
            sessionID: nil,
            durationSeconds: 0,
            providerReceipt: nil,
            resolvedModel: nil,
            configuredProviderID: nil,
            adapterVersion: nil,
            canonicalOutcome: .completed,
            sessionReuseDisposition: .fresh
        )
        let endState = ExecutableState(
            id: "end",
            label: "Workflow complete",
            type: .end,
            ownerAgentID: "terminal_writer",
            runBlock: ExecutableRunBlock(phases: [
                .sequential([AgentTask(agent: "terminal_writer", task: "finalize", inputs: nil, outputs: nil)])
            ]),
            runAfterApproval: nil,
            transitions: [
                ExecutableTransition(to: "end", condition: .always)
            ],
            approvalRequired: false,
            approvalPolicy: nil,
            loop: nil
        )
        let plan = RunPlan(
            workflowID: "terminal_self_loop",
            workflowTitle: "Terminal Self Loop",
            states: ["end": endState],
            initialStateID: "end",
            agentBindings: ["terminal_writer": terminalAgent],
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
            executor: StaticResultExecutor(result: terminalResult),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        let endStage = try #require(run.stageExecutions.first(where: { $0.stageID == "end" }))
        #expect(endStage.status == .completed)
        #expect(endStage.agentExecutions.count == 1)
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
            adapterVersion: nil,
            canonicalOutcome: .failedBeforeOutput,
            sessionReuseDisposition: .fresh
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
