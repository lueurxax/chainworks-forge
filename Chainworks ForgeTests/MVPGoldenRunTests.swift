import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("MVP Golden Run", .serialized, .tags(.live))
struct MVPGoldenRunTests {
    private func encodeFixtureProviderBindings(for plan: RunPlan) throws -> Data {
        let bindings = Dictionary(uniqueKeysWithValues: plan.agentBindings.map { agentID, agent in
            let family = ProviderFamily.from(runtimeIdentifier: agent.provider) ?? .claudeACP
            let binding = ResolvedProviderBinding(
                agentID: agentID,
                backendProfileID: agent.backendProfileID,
                configuredProviderID: UUID(),
                providerFamily: family.rawValue,
                providerIdentifier: family.runtimeProviderIdentifier,
                model: agent.model,
                effort: agent.effort,
                transport: ProviderTransport.cli.rawValue,
                adapterVersion: "fixture-v1"
            )
            return (agentID, binding)
        })

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try encoder.encode(bindings)
    }

    @Test("full-mvp-live reaches workflow_complete with fixture transport")
    func fullMVPLiveReachesWorkflowComplete() async throws {
        let (_, context) = try makeTestModelContainer()
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Golden Run", body: "Repo-backed golden run proof")
        context.insert(idea)
        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: try encodeFixtureProviderBindings(for: plan)
            )
        )
        #expect(run.providerBindingSnapshotJSON != nil)
        let transport = FixtureGooseTransport(scenario: .fullMVPSuccess)
        let executor = RuntimeAgentExecutor(transport: transport)
        let liveConfiguration = LiveRuntimeConfiguration(
            baseURL: URL(string: "http://fixture.local")!,
            apiKey: nil,
            override: LiveExecutionOverride(
                enabled: true,
                provider: "claude_code",
                model: "fixture-model",
                effort: "high"
            ),
            transportMode: .fixtureFullMVPSuccess,
            transportAPI: .bespoke
        )
        let service = ExecutionService(
            modelContext: context,
            executor: executor,
            catalog: catalog,
            liveRuntimeConfiguration: liveConfiguration
        )

        service.startRun(run: run, plan: plan, workspace: workspace)
        
        // Use a reasonable timeout for the golden run proof
        await awaitCondition("Full MVP golden run should reach completion", timeout: 30.0) {
            if let orchestrator = service.activeOrchestrators[run.id] {
                // Auto-resolve any pending approvals to keep the run moving
                if let request = service.pendingApprovals.values.first(where: { $0.runID == run.id }) {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Golden run auto-approve")
                }
            }
            return run.status == .completed || run.status == .blocked || run.status == .failed
        }

        #expect(run.status == .completed)
        let artifactManager = ArtifactManager(modelContext: context)
        let artifactNames = Set(try artifactManager.artifacts(forRunID: run.id).map(\.name))
        #expect(
            artifactNames.isSuperset(of: ["release_manifest", "git_push_receipt", "connect_upload_receipt", "delivery_receipt"]),
            "Completed full-MVP runs must persist the release and delivery artifacts that prove the manual-release and terminal workflow steps executed."
        )
    }
}
