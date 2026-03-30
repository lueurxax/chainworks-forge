import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("RunCancellationCoordinator", .serialized, .tags(.fast))
struct RunCancellationCoordinatorTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, AggregateSettlementRecord.self, Artifact.self])
        let config = ModelConfiguration("RunCancellationCoordinatorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    @Test("Cancellation settlement persists cancelled_after_output canonical truth")
    func cancellationAfterOutputPersistsCanonicalTruth() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
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
            startedAt: Date(timeIntervalSinceNow: -59),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: "proposal_writer",
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 512,
                rawPayloadPersisted: true,
                contractID: "proposal_current_v1",
                normalizedArtifactProduced: false,
                provider: "anthropic",
                model: "claude-opus-4.6",
                effort: "high",
                sessionID: "session-1",
                durationSeconds: 1.0
            )
        ])
        agent.providerReceiptJSON = try JSONEncoder().encode(
            ProviderExecutionReceipt(
                providerFamily: "anthropic",
                configuredProviderID: nil,
                model: "claude-opus-4.6",
                effort: "high",
                transport: "goose_server",
                inputTokens: 10,
                outputTokens: 20,
                billedUnits: nil,
                costCents: nil,
                wallClockSeconds: 1.0,
                rawReceiptJSON: nil
            )
        )
        agent.gooseSessionID = "session-1"
        agent.stageExecution = stage
        context.insert(agent)
        try context.save()

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context,
            catalog: try loadTestCanonicalCatalog()
        )

        let coordinator = RunCancellationCoordinator(
            run: run,
            orchestrator: orchestrator,
            modelContext: context
        )
        coordinator.beginSettlement()

        #expect(agent.canonicalOutcome == .cancelledAfterOutput)
        #expect(agent.status == .cancelled)
        #expect(agent.outputPresence == .durableOutput)
        #expect(agent.providerStopReason == "operator_cancelled")
        #expect(agent.runtimeProvider == "anthropic")
        #expect(agent.runtimeModel == "claude-opus-4.6")
        #expect(agent.settledAt != nil)
        #expect(agent.outcomeEnvelopeJSON != nil)
    }

    @Test("Cancellation settlement persists cancelled_before_output canonical truth")
    func cancellationBeforeOutputPersistsCanonicalTruth() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
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
            startedAt: Date(timeIntervalSinceNow: -59),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        agent.gooseSessionID = "session-2"
        agent.stageExecution = stage
        context.insert(agent)
        try context.save()

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context,
            catalog: try loadTestCanonicalCatalog()
        )

        let coordinator = RunCancellationCoordinator(
            run: run,
            orchestrator: orchestrator,
            modelContext: context
        )
        coordinator.beginSettlement()

        #expect(agent.canonicalOutcome == .cancelledBeforeOutput)
        #expect(agent.status == .cancelled)
        #expect(agent.outputPresence == OutputPresence.none)
        #expect(agent.providerStopReason == "operator_cancelled")
        #expect(agent.settledAt != nil)
    }

    private func makeRunFromPlan() throws -> (Run, RunPlan, RunWorkspace) {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Cancellation", body: "Cancellation truth test")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("RunCancellationCoordinatorTests-\(runID.uuidString)", isDirectory: true)
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
        )
        run.idea = idea
        context.insert(run)
        try context.save()

        return (run, plan, workspace)
    }
}
