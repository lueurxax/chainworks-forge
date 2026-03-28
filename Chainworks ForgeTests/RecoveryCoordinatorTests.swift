import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Recovery Coordinator")
struct RecoveryCoordinatorTests {
    @Test("Blocked run with failed stage exposes retry actions before clone")
    func blockedRunWithFailedStageOffersRetryActions() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Blocked Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(),
            status: .failed,
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
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.stageExecution = stage
        agent.resolvedBackendProfileID = "writer_profile"
        agent.resolvedModel = "claude-opus-4.6"
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)

        #expect(actions.contains(.retryAgent(stageID: "state_2_proposal_drafted", agentID: "proposal_writer")))
        #expect(actions.contains(.retryStage(stageID: "state_2_proposal_drafted")))
    }

    @Test("Cloning blocked run settles source run into terminal history")
    func cloneBlockedRunSettlesSourceRun() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Blocked Idea", body: "Body", status: .active)
        context.insert(idea)

        let workflow = makeRecoveryWorkflow()
        let catalog = makeRecoveryCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let (run, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "/tmp/workflow.yaml",
            catalogSourcePath: "/tmp/agents.yaml"
        )
        run.status = .blocked
        run.driftDetails = "Run blocked and cannot proceed"

        let coordinator = RecoveryCoordinator(modelContext: context)
        let clone = try coordinator.cloneRunFrozenSnapshot(
            original: run,
            idea: idea,
            compiler: compiler
        )

        #expect(run.status == .cancelled)
        #expect(run.completedAt != nil)
        #expect(clone.id != run.id)
        #expect(idea.runs.contains(where: { $0.id == clone.id }))
    }
}

private func makeRecoveryContext() throws -> ModelContext {
    let config = ModelConfiguration("RecoveryTests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        configurations: config
    )
    return ModelContext(container)
}

private func makeRecoveryWorkflow() -> WorkflowDefinition {
    WorkflowDefinition(
        schemaVersion: 1,
        workflow: WorkflowMeta(
            id: "wf-recovery",
            name: "Recovery Workflow",
            usesAgentCatalog: nil,
            description: "test",
            ideaInput: nil,
            execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
            requiredProviders: []
        ),
        variables: nil,
        failurePolicy: nil,
        scoring: nil,
        initialState: "state_1",
        states: [
            "state_1": WorkflowState(
                label: "Idea received",
                type: "start",
                owner: "lead_orchestrator",
                approval: nil,
                run: nil,
                runAfterApproval: nil,
                loop: nil,
                transitions: []
            )
        ]
    )
}

private func makeRecoveryCatalog() -> AgentCatalog {
    AgentCatalog(
        schemaVersion: 1,
        app: AppConfig(
            name: "test",
            runtime: "goose",
            transport: "http",
            description: "test",
            ideaInputMode: "text",
            singleActiveRunPerIdea: true,
            runResumePolicy: "automatic_on_launch",
            requiredProviders: []
        ),
        paths: [:],
        artifacts: [:],
        skills: [:],
        contracts: [:],
        backendProfiles: [
            "writer_profile": BackendProfile(
                provider: "claude_code",
                model: "claude-opus-4.6",
                effort: "high",
                temperature: 0,
                maxTurns: 20,
                structuredOutput: ""
            )
        ],
        permissionProfiles: [:],
        agents: [
            AgentDefinition(
                id: "lead_orchestrator",
                title: "Lead / Orchestrator",
                mode: "orchestration",
                backendProfile: "writer_profile",
                permissionProfile: "ORCH",
                skillRef: "orchestrator_core",
                skillRole: nil,
                worktreePolicy: nil,
                requiredTools: nil,
                inputs: [],
                outputs: [],
                outputContract: nil,
                requiresHumanApproval: false,
                prompt: "test",
                notes: nil
            )
        ]
    )
}

private func makeRun(status: RunStatus) -> Run {
    Run(
        startedAt: Date(),
        status: status,
        workflowID: "wf",
        workflowTitle: "WF",
        workflowSnapshotHash: "hash",
        catalogSnapshotHash: "catalog",
        workflowSourcePath: "workflow.yaml",
        catalogSourcePath: "agents.yaml",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        workspaceRoot: "/tmp/workspace",
        artifactRoot: "/tmp/artifacts",
        planCompilerVersion: 1
    )
}
