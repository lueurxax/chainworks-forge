import XCTest
import SwiftData
@testable import Chainworks_Forge

@MainActor
final class RunPlanCompilerTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var compiler: RunPlanCompiler!

    override func setUp() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    // MARK: - Helpers

    private func loadCanonicalWorkflow() throws -> WorkflowDefinition {
        let url = Bundle(for: type(of: self)).url(forResource: "workflow", withExtension: "yaml")!
        return try YAMLParser.loadWorkflow(from: url)
    }

    private func loadCanonicalCatalog() throws -> AgentCatalog {
        let url = Bundle(for: type(of: self)).url(forResource: "agents", withExtension: "yaml")!
        return try YAMLParser.loadAgentCatalog(from: url)
    }

    private func loadCompactWorkflow() throws -> CompactWorkflowDefinition {
        let url = Bundle(for: type(of: self)).url(forResource: "proposal-to-release", withExtension: "yaml")!
        return try YAMLParser.loadCompactWorkflow(from: url)
    }

    // MARK: - Phase 1: previewCompile

    func testCompileCanonicalWorkflow() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        XCTAssertEqual(plan.workflowID, "proposal_to_release")
        XCTAssertEqual(plan.states.count, 12, "Canonical workflow has 12 states")
        XCTAssertEqual(plan.initialStateID, "state_1_idea_received")
        XCTAssertEqual(plan.planCompilerVersion, RunPlan.currentCompilerVersion)
    }

    func testAllAgentsResolved() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // All 13 agents should be in bindings
        XCTAssertEqual(plan.agentBindings.count, 13, "All 13 agents resolved")

        // Verify a sample agent's backend is resolved
        let leadOrch = plan.agentBindings["lead_orchestrator"]
        XCTAssertNotNil(leadOrch)
        XCTAssertEqual(leadOrch?.provider, "claude_code")
        XCTAssertEqual(leadOrch?.effort, "high")
    }

    func testInitialState() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        XCTAssertEqual(plan.initialStateID, "state_1_idea_received")
        let state1 = plan.states["state_1_idea_received"]
        XCTAssertNotNil(state1)
        XCTAssertEqual(state1?.type, .start)
    }

    func testEndState() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let state12 = plan.states["state_12_workflow_complete"]
        XCTAssertNotNil(state12)
        XCTAssertEqual(state12?.type, .end)
    }

    func testProvenanceHashes() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Hashes should match direct DefinitionHasher output
        let (_, directWorkflowHash) = try DefinitionHasher.hash(workflow)
        let (_, directCatalogHash) = try DefinitionHasher.hash(catalog)

        XCTAssertEqual(plan.workflowSnapshotHash, directWorkflowHash)
        XCTAssertEqual(plan.catalogSnapshotHash, directCatalogHash)
    }

    func testVariablesPreserved() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Canonical workflow has variables section
        XCTAssertFalse(plan.variables.isEmpty, "Variables should be preserved")
    }

    func testScoringPreserved() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        XCTAssertNotNil(plan.scoring, "Scoring config should be preserved")
        XCTAssertNotNil(plan.scoring?.proposal)
    }

    func testApprovalGatesDetected() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let approvalStates = plan.states.values.filter { $0.approvalRequired }
        XCTAssertEqual(approvalStates.count, 3, "Canonical workflow has 3 approval gates")
    }

    func testLoopResolution() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_5_proposal_refined has loop with max from vars
        let state5 = plan.states["state_5_proposal_refined"]
        XCTAssertNotNil(state5?.loop)
        XCTAssertEqual(state5?.loop?.counter, "proposal_revision_cycles")
        XCTAssertEqual(state5?.loop?.resolvedMax, 6, "vars.max_proposal_revision_cycles = 6")
    }

    func testTransitionConditionParsing() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_1 has: when: exists('idea_brief') -> artifactExists
        let state1 = plan.states["state_1_idea_received"]!
        if case .artifactExists(let name) = state1.transitions.first?.condition {
            XCTAssertEqual(name, "idea_brief")
        } else {
            XCTFail("Expected .artifactExists condition for state_1")
        }

        // state_3 has: when: approval.granted == true -> approvalGranted
        let state3 = plan.states["state_3_initial_proposal_approval"]!
        if case .approvalGranted = state3.transitions.first?.condition {
            // pass
        } else {
            XCTFail("Expected .approvalGranted condition for state_3")
        }

        // state_5 has: when: 'true' -> always
        let state5 = plan.states["state_5_proposal_refined"]!
        if case .always = state5.transitions.first?.condition {
            // pass
        } else {
            XCTFail("Expected .always condition for state_5")
        }
    }

    func testRunBlockPhases() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_4 has parallel + then -> two phases
        let state4 = plan.states["state_4_proposal_reviewed"]!
        let phases = state4.runBlock!.phases
        XCTAssertEqual(phases.count, 2, "parallel + then = two phases")

        if case .parallel(let tasks) = phases[0] {
            XCTAssertEqual(tasks.count, 4, "4 parallel reviewers")
        } else {
            XCTFail("First phase should be parallel")
        }

        if case .sequential(let tasks) = phases[1] {
            XCTAssertGreaterThan(tasks.count, 0, "then block should have tasks")
        } else {
            XCTFail("Second phase should be sequential (then)")
        }
    }

    // MARK: - Error Cases

    func testMissingAgentThrows() throws {
        let catalog = try loadCanonicalCatalog()

        // Create a workflow referencing a non-existent agent
        var workflow = try loadCanonicalWorkflow()
        var modifiedStates = workflow.states
        modifiedStates["bad_state"] = WorkflowState(
            label: "Bad", type: nil, owner: "nonexistent_agent",
            approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil
        )
        workflow = WorkflowDefinition(
            schemaVersion: workflow.schemaVersion,
            workflow: workflow.workflow,
            variables: workflow.variables,
            failurePolicy: workflow.failurePolicy,
            scoring: workflow.scoring,
            initialState: workflow.initialState,
            states: modifiedStates
        )

        XCTAssertThrowsError(try compiler.previewCompile(workflow: workflow, catalog: catalog)) { error in
            // Validation runs before agent resolution, so the validator catches the bad owner first
            if case CompilationError.validationFailed(let issues) = error {
                let ownerErrors = issues.filter { $0.message.contains("nonexistent_agent") }
                XCTAssertFalse(ownerErrors.isEmpty, "Should report nonexistent_agent in validation issues")
            } else if case CompilationError.agentNotFound(let agentID, _) = error {
                XCTAssertEqual(agentID, "nonexistent_agent")
            } else {
                XCTFail("Expected validation or agentNotFound error, got: \(error)")
            }
        }
    }

    // MARK: - Compact Normalization

    func testCompactNormalization() throws {
        let compact = try loadCompactWorkflow()
        let catalog = try loadCanonicalCatalog()

        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        XCTAssertEqual(plan.workflowID, "proposal-to-release")
        XCTAssertGreaterThan(plan.states.count, 0, "Compact should produce states")
    }

    func testCompactAliasResolution() throws {
        let compact = try loadCompactWorkflow()
        let catalog = try loadCanonicalCatalog()

        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        // proposal-writer should resolve to proposal_writer
        XCTAssertNotNil(plan.agentBindings["proposal_writer"], "proposal-writer alias should resolve")
        // proposal-po-reviewer should resolve via alias map
        XCTAssertNotNil(plan.agentBindings["proposal_reviewer_product_owner"], "proposal-po-reviewer alias should resolve via map")
    }

    // MARK: - Phase 2: createRun

    func testCreateRunPersistsCorrectly() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Test", body: "Test idea")
        context.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml"
        )

        XCTAssertEqual(run.workflowID, "proposal_to_release")
        XCTAssertEqual(run.workflowSnapshotHash, plan.workflowSnapshotHash)
        XCTAssertEqual(run.planCompilerVersion, RunPlan.currentCompilerVersion)
        XCTAssertFalse(run.workspaceRoot.isEmpty)
        XCTAssertFalse(run.artifactRoot.isEmpty)
        XCTAssertEqual(run.id, workspace.runID)
        XCTAssertTrue(run.stageExecutions.isEmpty, "StageExecutions are created lazily (ARCH-027)")
    }

    func testPreviewCompileDoesNotPersist() throws {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()

        // previewCompile should not touch SwiftData
        _ = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // No runs should exist
        let descriptor = FetchDescriptor<Run>()
        let runs = try context.fetch(descriptor)
        XCTAssertTrue(runs.isEmpty, "previewCompile must not create any SwiftData records")
    }
}
