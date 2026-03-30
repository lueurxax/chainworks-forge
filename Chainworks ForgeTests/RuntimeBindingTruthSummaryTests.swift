import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Runtime Binding Truth Summary", .serialized, .tags(.fast))
struct RuntimeBindingTruthSummaryTests {
    @Test("Binding summary highlights frozen versus runtime mismatch")
    func bindingSummaryHighlightsFrozenVersusRuntimeMismatch() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)

        let binding = ResolvedProviderBinding(
            agentID: "lead_orchestrator",
            backendProfileID: "lead_profile",
            configuredProviderID: UUID(),
            providerFamily: "claude_code",
            providerIdentifier: "claude-configured",
            model: "claude-3-5-sonnet",
            effort: "high",
            transport: "goose",
            adapterVersion: "v2"
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode(["lead_orchestrator": binding])

        let provenance = FrozenBindingProvenance(
            source: .backendProfileDefault,
            backendProfileID: "lead_profile",
            backendProfileModel: "claude-3-5-sonnet",
            configuredProviderID: nil,
            configuredProviderDefaultModel: "claude-3-5-sonnet",
            runOverrideModel: nil,
            resolvedModel: "claude-3-5-sonnet",
            resolvedProviderFamily: "claude_code"
        )
        run.bindingProvenanceJSON = try JSONEncoder().encode(["lead_orchestrator": provenance])

        let stage = StageExecution(
            stageID: "state_1_idea_received",
            label: "Idea received",
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "capture_idea",
            startedAt: Date(),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agent.runtimeProvider = "claude_code"
        agent.runtimeModel = "claude-3-7-sonnet"
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let summary = RuntimeBindingTruthSummaryBuilder.summaryText(for: run)

        #expect(summary?.contains("frozen=claude_code/claude-3-5-sonnet") == true)
        #expect(summary?.contains("runtime=claude_code/claude-3-7-sonnet") == true)
        #expect(summary?.localizedCaseInsensitiveContains("unverifiable") == true)
    }
}
