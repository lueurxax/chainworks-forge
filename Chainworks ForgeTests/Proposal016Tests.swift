import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 016", .serialized, .tags(.fast))
struct Proposal016Tests {
    struct OutcomeMappingCase: Sendable {
        let outcome: AgentCanonicalOutcome
        let expectedStatus: AgentStatus
    }

    @Test("Neutral finish without success criterion does not set completed")
    func neutralFinishWithoutSuccessCriterionDoesNotSetCompleted() {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.providerStopReason = "stop"

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) != .completed)
    }

    @Test("Timeout after output maps to timed out after output")
    func timeoutAfterOutputMapsToTimedOutAfterOutput() throws {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.transportErrorKind = .timeout
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: "state_2",
                runID: UUID(),
                rawPayloadSize: 32,
                rawPayloadPersisted: true,
                provider: "claude_code"
            )
        ])

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .timedOutAfterOutput)
    }

    @Test("Timeout before output maps to timed out before output")
    func timeoutBeforeOutputMapsToTimedOutBeforeOutput() {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.transportErrorKind = .timeout

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .timedOutBeforeOutput)
    }

    @Test("Limit exhaustion after output maps to limit exhausted after output")
    func limitExhaustedAfterOutputMapsToLimitExhaustedAfterOutput() throws {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.providerStopReason = "max_tokens"
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: "state_2",
                runID: UUID(),
                rawPayloadSize: 32,
                rawPayloadPersisted: true,
                provider: "claude_code"
            )
        ])

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .limitExhaustedAfterOutput)
    }

    @Test("Limit exhaustion before output maps to limit exhausted before output")
    func limitExhaustedBeforeOutputMapsToLimitExhaustedBeforeOutput() {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.providerStopReason = "rate_limit"

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .limitExhaustedBeforeOutput)
    }

    @Test("Policy-bound stop is not marked successful")
    func policyBoundStopIsNotMarkedSuccessful() {
        let agent = AgentExecution(
            agentID: "reviewer",
            agentTitle: "Reviewer",
            taskName: "review",
            startedAt: Date(),
            status: .failed,
            provider: "gemini",
            effort: "medium"
        )
        agent.providerStopReason = "policy_violation"

        #expect(ExecutionTruthSupport.isPolicyBoundStopReason(agent.providerStopReason))
        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) != .completed)
    }

    @Test("Cancelled before output maps to cancelled before output")
    func cancelledBeforeOutputMapsToCancelledBeforeOutput() {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .cancelled,
            provider: "claude_code",
            effort: "high"
        )

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .cancelledBeforeOutput)
    }

    @Test("Cancelled after output maps to cancelled after output")
    func cancelledAfterOutputMapsToCancelledAfterOutput() throws {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .cancelled,
            provider: "claude_code",
            effort: "high"
        )
        agent.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_current",
                agentID: agent.agentID,
                stageID: "state_2",
                runID: UUID(),
                rawPayloadSize: 32,
                rawPayloadPersisted: true,
                provider: "claude_code"
            )
        ])

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .cancelledAfterOutput)
    }

    @Test("Canonical outcome maps deterministically to agent status", arguments: [
        OutcomeMappingCase(outcome: .completed, expectedStatus: .completed),
        OutcomeMappingCase(outcome: .completedWithTransportError, expectedStatus: .completed),
        OutcomeMappingCase(outcome: .failedBeforeOutput, expectedStatus: .failed),
        OutcomeMappingCase(outcome: .failedAfterOutputValidation, expectedStatus: .failed),
        OutcomeMappingCase(outcome: .timedOutBeforeOutput, expectedStatus: .failed),
        OutcomeMappingCase(outcome: .timedOutAfterOutput, expectedStatus: .failed),
        OutcomeMappingCase(outcome: .cancelledBeforeOutput, expectedStatus: .cancelled),
        OutcomeMappingCase(outcome: .cancelledAfterOutput, expectedStatus: .cancelled),
        OutcomeMappingCase(outcome: .limitExhaustedBeforeOutput, expectedStatus: .failed),
        OutcomeMappingCase(outcome: .limitExhaustedAfterOutput, expectedStatus: .failed),
    ])
    func canonicalOutcomeMapsDeterministicallyToAgentStatus(testCase: OutcomeMappingCase) {
        #expect(testCase.outcome.coarseStatus == testCase.expectedStatus)
    }

    @Test("Raw receipt cannot override canonical outcome columns")
    func rawReceiptCannotOverrideCanonicalOutcomeColumns() throws {
        let agent = AgentExecution(
            agentID: "writer",
            agentTitle: "Writer",
            taskName: "draft",
            startedAt: Date(),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .timedOutAfterOutput
        agent.outputPresence = .durableOutput
        agent.providerReceiptJSON = try JSONEncoder().encode(
            ProviderExecutionReceipt(
                providerFamily: "anthropic",
                configuredProviderID: nil,
                model: "claude-opus-4.6",
                effort: "high",
                transport: "goose",
                inputTokens: 10,
                outputTokens: 50,
                billedUnits: 60,
                costCents: 12,
                wallClockSeconds: 1.0,
                rawReceiptJSON: nil
            )
        )

        #expect(ExecutionTruthSupport.deterministicLegacyOutcome(for: agent) == .timedOutAfterOutput)
    }

    @Test("Stage retry recommendation defaults to operator inspection for limit exhaustion")
    func limitExhaustionDefaultsToOperatorInspection() throws {
        let context = try makeContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .limitExhaustedAfterOutput
        agent.providerStopReason = "max_tokens"
        agent.outputPresence = .durableOutput
        agent.stageExecution = stage
        context.insert(agent)

        let coordinator = StageRetryCoordinator(modelContext: context)
        let snapshot = coordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )

        #expect(snapshot.recommendedAction?.action == .operatorInspection)
    }

    @Test("Stage retry recommendation defaults to operator inspection for policy-bound terminal stops")
    func policyBoundStopsDefaultToOperatorInspection() throws {
        let context = try makeContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .failedBeforeOutput
        agent.providerStopReason = "policy_violation"
        agent.outputPresence = OutputPresence.none
        agent.stageExecution = stage
        context.insert(agent)

        let coordinator = StageRetryCoordinator(modelContext: context)
        let snapshot = coordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )

        #expect(snapshot.recommendedAction?.action == .operatorInspection)
    }

    @Test("Proposal 016 harness produces a passing proof payload")
    func proposal016HarnessProducesPassingProof() throws {
        let context = try makeContext()
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let harness = Proposal016ExecutionTruthHarness(modelContext: context, repositoryRoot: repoRoot)

        let result = try harness.runProof()

        #expect(result.passed)
        #expect(result.limitReason.localizedCaseInsensitiveContains("limit")
            || result.limitReason.localizedCaseInsensitiveContains("exhaust"))
        #expect(result.runtimeTrust.contains("unverifiable"))
        #expect(result.policySummary.contains("Clone"))
        #expect(result.repairSummary.localizedCaseInsensitiveContains("legacy"))
        #expect(result.reportPath != nil)
    }

    private func makeContext() throws -> ModelContext {
        let config = ModelConfiguration("Proposal016Tests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
        let container = try ModelContainer(
            for: Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, AggregateSettlementRecord.self, Artifact.self,
            configurations: config
        )
        return ModelContext(container)
    }

    private func makeRun(status: RunStatus) -> Run {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal016-\(UUID().uuidString)", isDirectory: true)
        return Run(
            status: status,
            workflowID: "proposal_to_release",
            workflowTitle: "Proposal To Release",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: root.path,
            artifactRoot: root.appendingPathComponent("artifacts", isDirectory: true).path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
    }
}
