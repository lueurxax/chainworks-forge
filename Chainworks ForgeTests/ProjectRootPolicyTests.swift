import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("Project Root Policy")
struct ProjectRootPolicyTests {
    @Test("Effective project root prefers linked workspace root")
    func effectiveProjectRootPrefersWorkspaceRoot() {
        let resolved = ProjectRootPolicy.effectiveProjectRoot(
            workspaceRootPath: "/tmp/workspace",
            deliveryRepoRootPath: "/tmp/delivery"
        )

        #expect(resolved == "/tmp/workspace")
    }

    @Test("Effective project root falls back to delivery repo root")
    func effectiveProjectRootFallsBackToDeliveryRepoRoot() {
        let resolved = ProjectRootPolicy.effectiveProjectRoot(
            workspaceRootPath: nil,
            deliveryRepoRootPath: "/tmp/delivery"
        )

        #expect(resolved == "/tmp/delivery")
    }

    @Test("Required project access rejects missing effective project root")
    func requiredProjectAccessRejectsMissingRoot() {
        #expect(throws: ProjectRootPolicyError.self) {
            try ProjectRootPolicy.requireProjectRoot(
                workspaceRootPath: nil,
                deliveryRepoRootPath: nil
            )
        }
    }

    @Test("Proposal loop workflow requires project access")
    func proposalLoopWorkflowRequiresProjectAccess() throws {
        let workflowURL = testRepositoryRootURL()
            .appendingPathComponent("examples/workflows/proposal-loop-live.yaml", isDirectory: false)
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)

        #expect(workflow.workflow.execution.requiresProjectAccess)
    }

    @Test("Full MVP workflow requires project access")
    func fullMVPWorkflowRequiresProjectAccess() throws {
        let workflowURL = testRepositoryRootURL()
            .appendingPathComponent("examples/workflows/full-mvp-live.yaml", isDirectory: false)
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)

        #expect(workflow.workflow.execution.requiresProjectAccess)
    }

    @Test("Canonical workflow requires project access")
    func canonicalWorkflowRequiresProjectAccess() throws {
        let workflowURL = testRepositoryRootURL()
            .appendingPathComponent("examples/workflows/workflow.yaml", isDirectory: false)
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)

        #expect(workflow.workflow.execution.requiresProjectAccess)
    }
}
