import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("RunPlanCompiler")
struct RunPlanCompilerTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("RunPlanCompilerTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    // MARK: - Phase 1: previewCompile

    @Test("Compile canonical workflow produces correct plan")
    func compileCanonicalWorkflow() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        #expect(plan.workflowID == "proposal_to_release")
        #expect(plan.states.count == 12, "Canonical workflow has 12 states")
        #expect(plan.initialStateID == "state_1_idea_received")
        #expect(plan.planCompilerVersion == RunPlan.currentCompilerVersion)
    }

    @Test("All agents resolved in bindings")
    func allAgentsResolved() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // All 13 agents should be in bindings
        #expect(plan.agentBindings.count == 13, "All 13 agents resolved")

        // Verify a sample agent's backend is resolved
        let leadOrch = plan.agentBindings["lead_orchestrator"]
        #expect(leadOrch != nil)
        #expect(leadOrch?.provider == "claude_code")
        #expect(leadOrch?.effort == "high")
    }

    @Test("Initial state is start type")
    func initialState() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        #expect(plan.initialStateID == "state_1_idea_received")
        let state1 = plan.states["state_1_idea_received"]
        #expect(state1 != nil)
        #expect(state1?.type == .start)
    }

    @Test("End state is end type")
    func endState() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let state12 = plan.states["state_12_workflow_complete"]
        #expect(state12 != nil)
        #expect(state12?.type == .end)
    }

    @Test("Provenance hashes match direct hasher output")
    func provenanceHashes() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Hashes should match direct DefinitionHasher output
        let (_, directWorkflowHash) = try DefinitionHasher.hash(workflow)
        let (_, directCatalogHash) = try DefinitionHasher.hash(catalog)

        #expect(plan.workflowSnapshotHash == directWorkflowHash)
        #expect(plan.catalogSnapshotHash == directCatalogHash)
    }

    @Test("Variables are preserved in plan")
    func variablesPreserved() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // Canonical workflow has variables section
        #expect(!plan.variables.isEmpty, "Variables should be preserved")
    }

    @Test("Scoring config is preserved in plan")
    func scoringPreserved() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        #expect(plan.scoring != nil, "Scoring config should be preserved")
        #expect(plan.scoring?.proposal != nil)
    }

    @Test("Approval gates detected in workflow")
    func approvalGatesDetected() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let approvalStates = plan.states.values.filter { $0.approvalRequired }
        #expect(approvalStates.count == 3, "Canonical workflow has 3 approval gates")
    }

    @Test("Loop resolution with variable max")
    func loopResolution() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_5_proposal_refined has loop with max from vars
        let state5 = plan.states["state_5_proposal_refined"]
        #expect(state5?.loop != nil)
        #expect(state5?.loop?.counter == "proposal_revision_cycles")
        #expect(state5?.loop?.resolvedMax == 6, "vars.max_proposal_revision_cycles = 6")
    }

    @Test("Transition condition parsing for various types")
    func transitionConditionParsing() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_1 has: when: exists('idea_brief') -> artifactExists
        let state1 = plan.states["state_1_idea_received"]!
        if case .artifactExists(let name) = state1.transitions.first?.condition {
            #expect(name == "idea_brief")
        } else {
            Issue.record("Expected .artifactExists condition for state_1")
        }

        // state_3 has: when: approval.granted == true -> approvalGranted
        let state3 = plan.states["state_3_initial_proposal_approval"]!
        if case .approvalGranted = state3.transitions.first?.condition {
            // pass
        } else {
            Issue.record("Expected .approvalGranted condition for state_3")
        }

        // state_5 has: when: 'true' -> always
        let state5 = plan.states["state_5_proposal_refined"]!
        if case .always = state5.transitions.first?.condition {
            // pass
        } else {
            Issue.record("Expected .always condition for state_5")
        }
    }

    @Test("Run block phases with parallel and then")
    func runBlockPhases() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // state_4 has parallel + then -> two phases
        let state4 = plan.states["state_4_proposal_reviewed"]!
        let phases = state4.runBlock!.phases
        #expect(phases.count == 2, "parallel + then = two phases")

        if case .parallel(let tasks) = phases[0] {
            #expect(tasks.count == 4, "4 parallel reviewers")
        } else {
            Issue.record("First phase should be parallel")
        }

        if case .sequential(let tasks) = phases[1] {
            #expect(tasks.count > 0, "then block should have tasks")
        } else {
            Issue.record("Second phase should be sequential (then)")
        }
    }

    // MARK: - Error Cases

    @Test("Missing agent throws compilation error")
    func missingAgentThrows() throws {
        let catalog = try loadTestCanonicalCatalog()

        // Create a workflow referencing a non-existent agent
        var workflow = try loadTestCanonicalWorkflow()
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

        #expect {
            try compiler.previewCompile(workflow: workflow, catalog: catalog)
        } throws: { error in
            // Validation runs before agent resolution, so the validator catches the bad owner first
            if case CompilationError.validationFailed(let issues) = error {
                let ownerErrors = issues.filter { $0.message.contains("nonexistent_agent") }
                return !ownerErrors.isEmpty
            } else if case CompilationError.agentNotFound(let agentID, _) = error {
                return agentID == "nonexistent_agent"
            }
            return false
        }
    }

    // MARK: - Compact Normalization

    @Test("Compact workflow normalization produces states")
    func compactNormalization() throws {
        let compact = try loadTestCompactWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        #expect(plan.workflowID == "proposal-to-release")
        #expect(plan.states.count > 0, "Compact should produce states")
    }

    @Test("Compact alias resolution for agents")
    func compactAliasResolution() throws {
        let compact = try loadTestCompactWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompileCompact(compact: compact, catalog: catalog)

        // proposal-writer should resolve to proposal_writer
        #expect(plan.agentBindings["proposal_writer"] != nil, "proposal-writer alias should resolve")
        // proposal-po-reviewer should resolve via alias map
        #expect(plan.agentBindings["proposal_reviewer_product_owner"] != nil, "proposal-po-reviewer alias should resolve via map")
    }

    // MARK: - Phase 2: createRun

    @Test("createRun persists correctly to SwiftData")
    func createRunPersistsCorrectly() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Test", body: "Test idea")
        context.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml"
        )

        #expect(run.workflowID == "proposal_to_release")
        #expect(run.workflowSnapshotHash == plan.workflowSnapshotHash)
        #expect(run.planCompilerVersion == RunPlan.currentCompilerVersion)
        #expect(!run.workspaceRoot.isEmpty)
        #expect(!run.artifactRoot.isEmpty)
        #expect(run.id == workspace.runID)
        #expect(run.stageExecutions.isEmpty, "StageExecutions are created lazily (ARCH-027)")
    }

    @Test("previewCompile does not persist SwiftData records")
    func previewCompileDoesNotPersist() throws {
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        // previewCompile should not touch SwiftData
        _ = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        // No runs should exist
        let descriptor = FetchDescriptor<Run>()
        let runs = try context.fetch(descriptor)
        #expect(runs.isEmpty, "previewCompile must not create any SwiftData records")
    }
}
