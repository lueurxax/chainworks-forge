import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Historical Run Replay", .serialized, .tags(.fast))
struct HistoricalRunReplayTests {
    @Test("legacyStopAfterOutputReplayProducesCanonicalInterruptedTruth")
    func legacyStopAfterOutputReplayProducesCanonicalInterruptedTruth() throws {
        let fixture = try loadLegacyStopFixture()
        let context = try makeTestModelContext()
        let (run, stage, agent) = try seedLegacyStopAfterOutputFixture(fixture, into: context)

        #expect(agent.canonicalOutcome == .limitExhaustedAfterOutput)
        #expect(run.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)

        let snapshot = StageRetryCoordinator(modelContext: context).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidenceBuilder.buildEvidencePacket(
                stageExecution: stage,
                failedAgent: agent,
                validationFailure: nil,
                outputEnvelopes: ExecutionTruthSupport.decodeOutputEnvelopes(from: agent),
                recoverySnapshot: snapshot
            )
        )
        try context.save()

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let recovery = RecoveryCoordinator(modelContext: context).recoveryContext(for: run)

        #expect(payload.blockedReason?.localizedCaseInsensitiveContains("limit") == true
            || payload.blockedReason?.localizedCaseInsensitiveContains("exhaust") == true)
        #expect(payload.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)
        #expect(recovery.allowedActions.contains(.cloneRunFrozenSnapshot))
        #expect(!recovery.allowedActions.contains(.retryAgent(stageID: stage.stageID, agentID: agent.agentID)))
    }

    @Test("duplicateActiveLineageReplayRepairsToSingleActiveOwner")
    func duplicateActiveLineageReplayRepairsToSingleActiveOwner() throws {
        let fixture = try loadDuplicateActiveLineageFixture()
        let context = try makeTestModelContext()
        let compiler = RunPlanCompiler(modelContext: context)
        let run = try seedDuplicateActiveLineageFixture(fixture, into: context)

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)
        let activeStages = run.stageExecutions.filter {
            $0.lineageID == fixture.lineageID && ($0.status == .running || $0.status == .ready || $0.status == .waitingApproval)
        }

        #expect(activeStages.count == 1)
        #expect(actions.count == 1)
        if case .resume = actions[0] {
            #expect(true)
        } else {
            Issue.record("Expected repaired duplicate lineage to remain resumable")
        }
    }

    @Test("aggregateMissingReplayOffersRetryAggregateStep")
    func aggregateMissingReplayOffersRetryAggregateStep() throws {
        let fixture = try loadAggregateMissingFixture()
        let context = try makeTestModelContext()
        let run = try seedAggregateMissingFixture(fixture, into: context)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let recovery = RecoveryCoordinator(modelContext: context).recoveryContext(for: run)

        #expect(payload.blockedReason == fixture.expectedFailureSummary)
        #expect(payload.retryPath == "Retry aggregate step in stage '\(fixture.stageID)'")
        #expect(recovery.allowedActions.contains(.retryAggregateStep(stageID: fixture.stageID)))
        #expect(!recovery.allowedActions.contains(.retryAgent(stageID: fixture.stageID, agentID: "lead_orchestrator")))
    }

    @Test("legacyReplayDoesNotInventVerifiedRuntimeTruth")
    func legacyReplayDoesNotInventVerifiedRuntimeTruth() throws {
        let fixture = try loadLegacyStopFixture()
        let context = try makeTestModelContext()
        let (run, _, _) = try seedLegacyStopAfterOutputFixture(fixture, into: context)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)

        #expect(payload.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue)
    }

    private func seedLegacyStopAfterOutputFixture(
        _ fixture: LegacyStopAfterOutputFixture,
        into context: ModelContext
    ) throws -> (Run, StageExecution, AgentExecution) {
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        run.status = RunStatus(rawValue: fixture.runStatus) ?? .blocked
        run.runtimeTrustLevel = fixture.runtimeTrustLevel

        let stage = StageExecution(
            stageID: fixture.stage.stageID,
            label: fixture.stage.label,
            startedAt: Date(timeIntervalSince1970: fixture.stage.startedAt),
            status: StageStatus(rawValue: fixture.stage.status) ?? .blocked,
            iteration: fixture.stage.iteration,
            attemptNumber: fixture.stage.attemptNumber
        )
        stage.lineageID = fixture.stage.lineageID
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let agent = AgentExecution(
            agentID: fixture.agent.agentID,
            agentTitle: fixture.agent.agentTitle,
            taskName: fixture.agent.taskName,
            startedAt: Date(timeIntervalSince1970: fixture.agent.startedAt),
            status: AgentStatus(rawValue: fixture.agent.status) ?? .failed,
            provider: fixture.agent.provider,
            effort: fixture.agent.effort
        )
        agent.completedAt = Date(timeIntervalSince1970: fixture.agent.completedAt)
        agent.logSnippet = "Provider or app limit exhausted after output was produced"
        agent.transportErrorKind = TransportErrorKind(rawValue: fixture.agent.transportErrorKind)
        agent.providerStopReason = fixture.agent.providerStopReason
        agent.providerReceiptJSON = try Data(contentsOf: fixtureURL(for: "legacy-stop-after-output").appendingPathComponent(fixture.agent.receiptFile))
        agent.transcriptArtifactPath = fixtureURL(for: "legacy-stop-after-output").appendingPathComponent(fixture.agent.transcriptFile).path
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "idea_brief",
                agentID: agent.agentID,
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 192,
                rawPayloadPersisted: true,
                contractID: "idea_brief",
                normalizedArtifactProduced: false,
                provider: fixture.agent.provider,
                model: "default",
                effort: fixture.agent.effort,
                sessionID: "legacy-stop-after-output",
                durationSeconds: 1.0
            )
        ])
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let legacyOutcome = try #require(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent))
        ExecutionTruthSupport.persistTerminalTruth(
            for: agent,
            canonicalOutcome: legacyOutcome,
            transportErrorKind: agent.transportErrorKind,
            providerStopReason: agent.providerStopReason,
            outputPresence: ExecutionTruthSupport.derivedOutputPresence(for: agent),
            runtimeProvider: nil,
            runtimeModel: nil,
            rawErrorMessage: "Execution did not produce final output",
            rawFinishEvent: "stop"
        )
        stage.status = .failed
        stage.settlementKind = .failed
        stage.settledAt = agent.settledAt
        stage.completedAt = agent.completedAt
        stage.activeOwnerToken = nil
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidencePacket(
                id: UUID(),
                timestamp: Date(timeIntervalSince1970: 13),
                stageID: stage.stageID,
                stageLabel: stage.label,
                stageAttemptNumber: stage.attemptNumber,
                failedAgentID: agent.agentID,
                failedAgentTitle: agent.agentTitle,
                failureSummary: "Provider or app limit exhausted after output was produced",
                failureClass: .transportFailure,
                rawOutputsExist: true,
                receiptExists: true,
                transcriptExists: true,
                validationFailure: nil,
                outputEnvelopes: ExecutionTruthSupport.decodeOutputEnvelopes(from: agent),
                timing: StageTiming(
                    stageStartedAt: stage.startedAt,
                    stageCompletedAt: stage.completedAt,
                    agentStartedAt: agent.startedAt,
                    agentCompletedAt: agent.completedAt,
                    agentDurationSeconds: 1.0
                ),
                recoverySnapshot: nil
            )
        )
        run.driftDetails = "Provider or app limit exhausted after output was produced"
        run.runtimeTrustLevel = RuntimeBindingTruthResolver.deriveRunTrustLevel(
            agents: run.stageExecutions.flatMap(\.agentExecutions),
            persisted: run.runtimeTrustLevel
        )

        let artifactPath = URL(fileURLWithPath: run.artifactRoot, isDirectory: true)
            .appendingPathComponent("state_1_idea_received.2/lead_orchestrator/1", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactPath, withIntermediateDirectories: true)
        let outputURL = artifactPath.appendingPathComponent(fixture.agent.artifactName)
        try "# Idea brief\n\nCaptured before limit exhaustion.\n".write(to: outputURL, atomically: true, encoding: .utf8)
        let artifact = Artifact(
            name: fixture.agent.artifactName,
            contractID: "idea_brief",
            format: .markdown,
            filePath: outputURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: agent.agentID,
            provider: agent.provider
        )
        artifact.agentExecution = agent
        agent.artifacts.append(artifact)
        context.insert(artifact)
        try context.save()

        return (run, stage, agent)
    }

    private func seedDuplicateActiveLineageFixture(
        _ fixture: DuplicateActiveLineageFixture,
        into context: ModelContext
    ) throws -> Run {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Duplicate lineage", body: "Replay")
        context.insert(idea)
        let workspace = makeTestWorkspace()
        let run = Run(
            id: workspace.runID,
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
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue
        context.insert(run)

        for stageFixture in fixture.stages {
            let stage = StageExecution(
                stageID: stageFixture.stageID,
                label: stageFixture.label,
                startedAt: Date(timeIntervalSince1970: stageFixture.startedAt),
                status: StageStatus(rawValue: stageFixture.status) ?? .running,
                iteration: stageFixture.iteration,
                attemptNumber: stageFixture.attemptNumber
            )
            stage.lineageID = fixture.lineageID
            stage.activeOwnerToken = stageFixture.activeOwnerToken
            stage.run = run
            run.stageExecutions.append(stage)
            context.insert(stage)
        }
        try context.save()
        return run
    }

    private func seedAggregateMissingFixture(
        _ fixture: AggregateMissingFixture,
        into context: ModelContext
    ) throws -> Run {
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        run.status = .blocked
        context.insert(run)

        let stage = StageExecution(
            stageID: fixture.stageID,
            label: fixture.stageLabel,
            startedAt: Date(timeIntervalSince1970: fixture.startedAt),
            status: .blocked,
            iteration: 1,
            attemptNumber: fixture.attemptNumber
        )
        stage.lineageID = fixture.lineageID
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        for reviewerID in fixture.reviewerAgentIDs {
            let reviewer = AgentExecution(
                agentID: reviewerID,
                agentTitle: reviewerID,
                taskName: "review_proposal",
                startedAt: Date(timeIntervalSince1970: fixture.startedAt + 1),
                status: .completed,
                provider: "claude_code",
                effort: "high"
            )
            reviewer.completedAt = Date(timeIntervalSince1970: fixture.startedAt + 2)
            reviewer.canonicalOutcome = .completed
            reviewer.outputPresence = .durableOutput
            reviewer.stageExecution = stage
            stage.agentExecutions.append(reviewer)
            context.insert(reviewer)
        }

        let aggregateAgent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: Date(timeIntervalSince1970: fixture.startedAt + 3),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        aggregateAgent.completedAt = Date(timeIntervalSince1970: fixture.startedAt + 4)
        aggregateAgent.stageExecution = stage
        stage.agentExecutions.append(aggregateAgent)
        context.insert(aggregateAgent)

        let failure = ValidationFailureRecord(
            agentID: aggregateAgent.agentID,
            stageID: stage.stageID,
            runID: run.id,
            outputResults: [],
            failureSummary: fixture.expectedFailureSummary,
            failureClass: .noOutputProduced,
            contractMetadata: [],
            rawOutputExists: false,
            receiptExists: true,
            transcriptExists: true,
            recoveryRecommendation: RecoveryRecommendation(
                action: .retryFailedStage,
                explanation: "Retry only the aggregate proposal review step.",
                source: .runtimePolicy
            )
        )
        let record = AggregateSettlementRecord(
            runID: run.id,
            stageExecutionID: stage.id,
            aggregateStepID: "aggregate_proposal_reviews",
            lineageID: fixture.lineageID,
            canonicalOutcome: .failedBeforeOutput
        )
        record.validationFailureJSON = try JSONEncoder().encode(failure)
        context.insert(record)

        let retryAggregate = RecoveryActionDetail(
            action: .retryAggregateStep,
            stageID: stage.stageID,
            agentID: nil,
            explanation: "Retry only the aggregate proposal review step. Contract-valid reviewer outputs are reused.",
            staysInSameRun: true,
            reusesSiblingOutputs: true,
            reExecutesWholeStage: false
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(
            RecoveryActionSnapshot(
                id: UUID(),
                timestamp: Date(timeIntervalSince1970: fixture.startedAt + 5),
                runID: run.id,
                recommendedAction: retryAggregate,
                availableActions: [
                    retryAggregate,
                    RecoveryActionDetail(
                        action: .cloneRunFrozenSnapshot,
                        stageID: nil,
                        agentID: nil,
                        explanation: "Clone fallback.",
                        staysInSameRun: false,
                        reusesSiblingOutputs: false,
                        reExecutesWholeStage: false
                    )
                ],
                validationFailureID: failure.id,
                source: .runtimePolicy
            )
        )
        try context.save()
        return run
    }

    private func loadLegacyStopFixture() throws -> LegacyStopAfterOutputFixture {
        try JSONDecoder().decode(
            LegacyStopAfterOutputFixture.self,
            from: Data(contentsOf: fixtureURL(for: "legacy-stop-after-output").appendingPathComponent("legacy-stop-after-output.json"))
        )
    }

    private func loadDuplicateActiveLineageFixture() throws -> DuplicateActiveLineageFixture {
        try JSONDecoder().decode(
            DuplicateActiveLineageFixture.self,
            from: Data(contentsOf: fixtureURL(for: "duplicate-active-lineage").appendingPathComponent("duplicate-active-lineage.json"))
        )
    }

    private func loadAggregateMissingFixture() throws -> AggregateMissingFixture {
        try JSONDecoder().decode(
            AggregateMissingFixture.self,
            from: Data(contentsOf: fixtureURL(for: "aggregate-missing-after-valid-fanout").appendingPathComponent("aggregate-missing-after-valid-fanout.json"))
        )
    }

    private func fixtureURL(for name: String) -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("Fixtures/Proposal016/\(name)", isDirectory: true)
    }
}

private struct LegacyStopAfterOutputFixture: Decodable {
    let runStatus: String
    let runtimeTrustLevel: String?
    let stage: ReplayStage
    let agent: ReplayAgent
}

private struct DuplicateActiveLineageFixture: Decodable {
    let lineageID: String
    let stages: [ReplayStage]
}

private struct AggregateMissingFixture: Decodable {
    let stageID: String
    let stageLabel: String
    let lineageID: String
    let attemptNumber: Int
    let startedAt: TimeInterval
    let reviewerAgentIDs: [String]
    let expectedFailureSummary: String
}

private struct ReplayStage: Decodable {
    let stageID: String
    let label: String
    let status: String
    let iteration: Int
    let attemptNumber: Int
    let lineageID: String?
    let startedAt: TimeInterval
    let activeOwnerToken: String?
}

private struct ReplayAgent: Decodable {
    let agentID: String
    let agentTitle: String
    let taskName: String
    let status: String
    let provider: String
    let effort: String
    let startedAt: TimeInterval
    let completedAt: TimeInterval
    let transportErrorKind: String
    let providerStopReason: String
    let receiptFile: String
    let transcriptFile: String
    let artifactName: String
}
