import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("MVP Golden Run", .serialized, .tags(.live))
struct MVPGoldenRunTests {
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
            catalogSourcePath: "test/agents.yaml"
        )
        let transport = FixtureGooseTransport(scenario: .fullMVPSuccess)
        let executor = GooseAgentExecutor(transport: transport)
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
        #expect(
            run.currentStageID == "state_11_manual_release" || run.currentStageID == "state_12_workflow_complete",
            "Completed full-MVP runs currently persist the last executed state ID; reaching the terminal end state may leave currentStageID at manual release."
        )
    }
}
