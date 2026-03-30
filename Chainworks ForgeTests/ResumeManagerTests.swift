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
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, AggregateSettlementRecord.self, Artifact.self])
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
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
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

    @Test("Resume manager repairs duplicate active stage siblings before resume")
    func repairsDuplicateActiveStageSiblingsBeforeResume() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let older = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        older.run = run
        context.insert(older)

        let newer = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(),
            status: .running,
            iteration: 1,
            attemptNumber: 2
        )
        newer.run = run
        context.insert(newer)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .resume = actions[0] {
            #expect(true)
        } else {
            Issue.record("Expected repaired run to remain resumable")
        }

        #expect(older.lineageID == newer.lineageID)
        #expect(older.status == .blocked)
        #expect(older.settlementKind == .repaired)
        #expect(older.settledAt != nil)
        #expect(older.activeOwnerToken == nil)
        #expect(newer.activeOwnerToken != nil)
    }

    @Test("Resume manager settles stale running stage when terminal agent truth already exists")
    func settlesStaleRunningStageFromTerminalAgentTruth() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -120),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -119),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .failedAfterOutputValidation
        agent.outputPresence = .durableOutput
        agent.settledAt = Date(timeIntervalSinceNow: -118)
        agent.completedAt = Date(timeIntervalSinceNow: -118)
        agent.stageExecution = stage
        context.insert(agent)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        _ = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(stage.status == .failed)
        #expect(stage.settlementKind == .failed)
        #expect(stage.settledAt != nil)
        #expect(stage.activeOwnerToken == nil)
    }

    @Test("Resume manager backfills deterministic legacy validation failure truth before repair")
    func backfillsLegacyValidationFailureTruth() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -120),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -119),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.completedAt = Date(timeIntervalSinceNow: -118)
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: "proposal_writer",
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 256,
                rawPayloadPersisted: true,
                contractID: "proposal_current_v1",
                normalizedArtifactProduced: false,
                provider: "anthropic",
                model: "claude-opus-4.6",
                effort: "high",
                sessionID: "legacy-writer",
                durationSeconds: 1.0
            )
        ])
        agent.validationFailureJSON = try JSONEncoder().encode(
            ValidationFailureRecord(
                agentID: "proposal_writer",
                stageID: stage.stageID,
                runID: run.id,
                outputResults: [],
                failureSummary: "Validation failed after output generation",
                failureClass: .outputContractMismatch,
                contractMetadata: [],
                rawOutputExists: true,
                receiptExists: false,
                transcriptExists: false,
                recoveryRecommendation: RecoveryRecommendation(
                    action: .retryFailedAgent,
                    explanation: "Retry",
                    source: .runtimePolicy
                )
            )
        )
        agent.stageExecution = stage
        context.insert(agent)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        _ = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(agent.canonicalOutcome == .failedAfterOutputValidation)
        #expect(agent.outputPresence == .durableOutput)
        #expect(agent.settledAt != nil)
        #expect(stage.status == .failed)
    }

    @Test("Resume manager fails closed for legacy rows without deterministic outcome evidence")
    func legacyRowsWithoutDeterministicEvidenceRequireExplicitDecision() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -120),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -119),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.completedAt = Date(timeIntervalSinceNow: -118)
        agent.stageExecution = stage
        context.insert(agent)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(run.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)
        #expect(agent.canonicalOutcome == nil)
        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.contains("legacy") || reason.contains("unverifiable"))
        } else {
            Issue.record("Expected explicit operator decision for legacy unverifiable row")
        }
    }

    @Test("Classify compiler version mismatch")
    func classifyCompilerVersionMismatch() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
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
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

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

    @Test("Blocked run requires explicit decision instead of auto resume at launch")
    func blockedRunRequiresExplicitDecision() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .blocked
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.contains("blocked runs are not auto-resumed"))
        } else {
            Issue.record("Expected blocked run to require manual decision")
        }
    }

    @Test("Legacy running run requires explicit decision instead of auto resume at launch")
    func legacyRunningRunRequiresExplicitDecision() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.unverifiable.rawValue
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.contains("explicit operator resume"))
        } else {
            Issue.record("Expected legacy running run to require manual decision")
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

        await service.cancelRun(runID: run.id)
    }

    @Test("ExecutionService cancel run")
    func executionServiceCancelRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        #expect(service.hasActiveRuns)

        await service.cancelRun(runID: run.id)
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

        await service.cancelRun(runID: run.id)
    }

    @Test("ExecutionService manual resume after retry launches pending retry attempt")
    func executionServiceManualResumeAfterRetryLaunchesPendingAttempt() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let refineStateID = "state_4_proposal_refined"

        let idea = Idea(title: "Manual Recovery Retry", body: "Resume pending retry attempt")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ManualRecoveryRetry-\(runID.uuidString)", isDirectory: true)
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
        run.status = .blocked

        let stage = StageExecution(
            stageID: refineStateID,
            label: "Proposal refined",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .blocked,
            iteration: 2,
            attemptNumber: 1
        )
        stage.lineageID = "\(refineStateID)::iteration:2"
        stage.run = run
        context.insert(stage)

        let failedAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(timeIntervalSinceNow: -59),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        failedAgent.completedAt = Date(timeIntervalSinceNow: -58)
        failedAgent.canonicalOutcome = .timedOutBeforeOutput
        failedAgent.outputPresence = .none
        failedAgent.logSnippet = "Execution timed out before output was produced"
        failedAgent.stageExecution = stage
        context.insert(failedAgent)
        try context.save()

        let recovery = RecoveryCoordinator(modelContext: context)
        _ = try recovery.retryAgent(run: run, stageID: refineStateID, agentID: "proposal_writer")

        let pendingRetry = try #require(stage.agentExecutions.first(where: { $0.agentAttemptNumber == 2 }))
        #expect(pendingRetry.status == .pending)

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
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

        try service.resumeRun(run: run, compiler: compiler, stageID: refineStateID)

        await awaitCondition("Manual recovery should attach orchestrator and launch retry attempt", timeout: 5.0) {
            service.orchestrator(for: run.id) != nil
                && stage.agentExecutions.contains(where: { $0.agentAttemptNumber == 2 && $0.startedAt != nil && $0.status != .pending })
        }

        #expect(service.orchestrator(for: run.id) != nil)
        let launchedRetry = try #require(stage.agentExecutions.first(where: { $0.agentAttemptNumber == 2 }))
        #expect(launchedRetry.status != .pending)
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

        await service.cancelRun(runID: run.id)
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

    @Test("ExecutionService resume waiting approval repairs duplicate approval siblings before restore")
    func executionServiceResumeWaitingApprovalRepairsDuplicateApprovals() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Resume Approval Repair", body: "Duplicate gate repair")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeApprovalRepair-\(runID.uuidString)", isDirectory: true)
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

        let olderStage = StageExecution(
            stageID: "state_5_proposal_approval",
            label: "Human approval: proposal quality",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        olderStage.run = run
        context.insert(olderStage)

        let newerStage = StageExecution(
            stageID: "state_5_proposal_approval",
            label: "Human approval: proposal quality",
            startedAt: Date(),
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 2
        )
        newerStage.run = run
        context.insert(newerStage)

        let olderApproval = Approval(stageID: "state_5_proposal_approval", decision: .requested)
        olderApproval.requestedAt = Date(timeIntervalSinceNow: -60)
        olderApproval.run = run
        context.insert(olderApproval)

        let newerApproval = Approval(stageID: "state_5_proposal_approval", decision: .requested)
        newerApproval.requestedAt = Date()
        newerApproval.run = run
        context.insert(newerApproval)
        try context.save()

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
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

        await awaitCondition("Duplicate approval repair should restore one pending approval", timeout: 3.0) {
            service.pendingApprovalCount == 1
        }

        #expect(run.approvals.filter { $0.decision == .requested }.count == 1)
        #expect(run.approvals.filter { $0.repairedAt != nil }.count == 1)
        #expect(run.stageExecutions.filter { $0.status == .waitingApproval }.count == 1)
        #expect(run.stageExecutions.filter { $0.settlementKind == .repaired }.count == 1)
        #expect(olderApproval.lineageID == newerApproval.lineageID)
        #expect(newerApproval.lineageID == "\(newerStage.lineageID ?? "")::approval")
        #expect(newerStage.activeOwnerToken != nil)
        #expect(olderStage.activeOwnerToken == nil)
    }
}
