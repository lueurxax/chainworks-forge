import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Legacy Execution Truth Backfill", .serialized, .tags(.fast))
struct LegacyExecutionTruthBackfillTests {
    private let context: ModelContext
    private let compiler: RunPlanCompiler

    init() throws {
        context = try makeTestModelContext()
        compiler = RunPlanCompiler(modelContext: context)
    }

    @Test("legacyReceiptWithOutputAndTimeoutBackfillsTimedOutAfterOutput")
    func legacyReceiptWithOutputAndTimeoutBackfillsTimedOutAfterOutput() throws {
        let (run, stage, agent) = try makeLegacyRun()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        agent.transportErrorKind = .timeout
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 256,
                rawPayloadPersisted: true,
                contractID: "proposal_current_v1",
                normalizedArtifactProduced: false,
                provider: "anthropic",
                model: "claude-opus-4.6",
                effort: "high",
                sessionID: "legacy-timeout-after-output",
                durationSeconds: 1.0
            )
        ])
        agent.completedAt = Date()
        try context.save()

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        #expect(agent.canonicalOutcome == .timedOutAfterOutput)
        #expect(agent.outputPresence == .durableOutput)
        #expect(stage.status == .failed)
    }

    @Test("legacyReceiptWithNoOutputAndTimeoutBackfillsTimedOutBeforeOutput")
    func legacyReceiptWithNoOutputAndTimeoutBackfillsTimedOutBeforeOutput() throws {
        let (run, stage, agent) = try makeLegacyRun()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        agent.transportErrorKind = .timeout
        agent.completedAt = Date()
        try context.save()

        _ = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(agent.canonicalOutcome == .timedOutBeforeOutput)
        #expect(agent.outputPresence == OutputPresence.none)
        #expect(stage.status == .failed)
    }

    @Test("legacyReceiptWithStopAndQuotaSignalBackfillsLimitExhaustedAfterOutput")
    func legacyReceiptWithStopAndQuotaSignalBackfillsLimitExhaustedAfterOutput() throws {
        let (run, stage, agent) = try makeLegacyRun()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        agent.providerStopReason = "max_tokens"
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 512,
                rawPayloadPersisted: true,
                contractID: "proposal_current_v1",
                normalizedArtifactProduced: false,
                provider: "anthropic",
                model: "claude-opus-4.6",
                effort: "high",
                sessionID: "legacy-limit",
                durationSeconds: 1.0
            )
        ])
        agent.completedAt = Date()
        try context.save()

        _ = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(agent.canonicalOutcome == .limitExhaustedAfterOutput)
        #expect(agent.outputPresence == .durableOutput)
        #expect(stage.status == .failed)
    }

    @Test("legacyFailedWithoutDurableEvidenceBecomesLegacyUnverifiable")
    func legacyFailedWithoutDurableEvidenceBecomesLegacyUnverifiable() throws {
        let (run, _, agent) = try makeLegacyRun()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        agent.completedAt = Date()
        try context.save()

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(agent.canonicalOutcome == nil)
        #expect(run.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)
        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.localizedCaseInsensitiveContains("legacy") || reason.localizedCaseInsensitiveContains("unverifiable"))
        } else {
            Issue.record("Expected explicit decision for unverifiable legacy run")
        }
    }

    @Test("legacyRowWithoutLineageSkipsRepairAndRequiresDecision")
    func legacyRowWithoutLineageSkipsRepairAndRequiresDecision() throws {
        let (run, _, _) = try makeLegacyRun()
        run.status = .waitingApproval
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let approval = Approval(
            stageID: "state_4_proposal_approval",
            requestedAt: Date(timeIntervalSinceNow: -30),
            decision: .requested
        )
        approval.run = run
        run.approvals.append(approval)
        context.insert(approval)
        try context.save()

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.localizedCaseInsensitiveContains("legacy") || reason.localizedCaseInsensitiveContains("unverifiable"))
        } else {
            Issue.record("Expected explicit decision when approval lineage cannot be reconstructed")
        }
        #expect(approval.lineageID == nil)
    }

    @Test("legacyBackfillIsFailClosedWhenSignalsConflict")
    func legacyBackfillIsFailClosedWhenSignalsConflict() throws {
        let (run, _, agent) = try makeLegacyRun()
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        agent.status = .cancelled
        agent.transportErrorKind = .timeout
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: "state_2_proposal_drafted",
                runID: run.id,
                rawPayloadSize: 256,
                rawPayloadPersisted: true,
                contractID: "proposal_current_v1",
                normalizedArtifactProduced: false,
                provider: "anthropic",
                model: "claude-opus-4.6",
                effort: "high",
                sessionID: "legacy-conflict",
                durationSeconds: 1.0
            )
        ])
        agent.completedAt = Date()
        try context.save()

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(agent.canonicalOutcome == nil)
        #expect(run.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)
        #expect(actions.count == 1)
        if case .needsDecision = actions[0] {
            #expect(true)
        } else {
            Issue.record("Expected conflicting legacy signals to stay fail-closed")
        }
    }

    private func makeLegacyRun() throws -> (Run, StageExecution, AgentExecution) {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Legacy", body: "Legacy backfill")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("LegacyBackfill-\(runID.uuidString)", isDirectory: true)
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

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -120),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
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
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)
        try context.save()

        return (run, stage, agent)
    }
}
