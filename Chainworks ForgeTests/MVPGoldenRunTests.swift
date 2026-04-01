import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("MVP Golden Run", .serialized, .tags(.live))
struct MVPGoldenRunTests {
    @Test("full-mvp-live reaches workflow_complete with fixture transport")
    func fullMVPLiveReachesWorkflowComplete() async throws {
        let (container, context) = try makeTestModelContainer()
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }
        
        let run = makeTestRun(workspace: workspace, context: context)
        let transport = FixtureGooseTransport(scenario: .fullMVPSuccess)
        let executor = GooseAgentExecutor(transport: transport)
        let service = ExecutionService(modelContext: context, executor: executor)

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
        #expect(run.currentStageID == "state_6_workflow_complete")
    }
}
