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

    @Test("Recovery context prefers persisted recovery snapshot suggestion")
    func recoveryContextPrefersPersistedSnapshotSuggestion() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Blocked Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let failureRecord = ValidationFailureRecord(
            agentID: agent.agentID,
            stageID: stage.stageID,
            runID: run.id,
            outputResults: [],
            failureSummary: "Persistence failed after aggregate output generation",
            failureClass: .persistenceFailure,
            contractMetadata: [],
            rawOutputExists: true,
            receiptExists: true,
            transcriptExists: false,
            recoveryRecommendation: RecoveryRecommendation(
                action: .cloneRun,
                explanation: "Use the current config clone for this proof.",
                source: .runtimePolicy
            )
        )
        agent.validationFailureJSON = try JSONEncoder().encode(failureRecord)

        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: RecoveryActionDetail(
                action: .cloneRunCurrentConfig,
                stageID: nil,
                agentID: nil,
                explanation: "Use the current config clone for this proof.",
                staysInSameRun: false,
                reusesSiblingOutputs: false,
                reExecutesWholeStage: false
            ),
            availableActions: [],
            validationFailureID: failureRecord.id,
            source: .runtimePolicy
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(recoveryContext.suggestedAction == .cloneRunCurrentConfig)
    }

    @Test("Retry agent targets latest failed attempt in repeated stage lineage")
    func retryAgentTargetsLatestFailedAttemptInRepeatedStageLineage() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Blocked Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let earlierStage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        earlierStage.run = run
        run.stageExecutions.append(earlierStage)
        context.insert(earlierStage)

        let earlierAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(timeIntervalSince1970: 110),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        earlierAgent.stageExecution = earlierStage
        earlierStage.agentExecutions.append(earlierAgent)
        context.insert(earlierAgent)

        let failedStage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .failed,
            iteration: 1,
            attemptNumber: 2
        )
        failedStage.run = run
        run.stageExecutions.append(failedStage)
        context.insert(failedStage)

        let failedAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(timeIntervalSince1970: 210),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        failedAgent.stageExecution = failedStage
        failedStage.agentExecutions.append(failedAgent)
        context.insert(failedAgent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let updatedRun = try coordinator.retryAgent(
            run: run,
            stageID: "state_5_proposal_refined",
            agentID: "proposal_writer"
        )

        #expect(updatedRun.status == .running)
        #expect(failedStage.status == .running)

        let retryAttempts = failedStage.agentExecutions.filter { $0.agentID == "proposal_writer" && $0.status == .pending }
        #expect(retryAttempts.count == 1)
        #expect(retryAttempts.first?.supersedesAgentExecutionID == failedAgent.id)
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
                transitions: [
                    Transition(to: "state_2", when: "always")
                ]
            ),
            "state_2": WorkflowState(
                label: "Completed",
                type: "end",
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
        skills: [
            "orchestrator_core": SkillRef(
                type: "inline_skill",
                path: nil,
                name: "Orchestrator Core",
                description: "Recovery test skill"
            )
        ],
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
        permissionProfiles: [
            "ORCH": PermissionProfile(
                filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
                git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
                shell: ShellPermissions(allow: nil, deny: nil),
                network: NetworkPermissions(allow: nil),
                mcp: MCPPermissions(allow: nil)
            )
        ],
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
                notes: nil,
                sessionReuseScope: nil,
                sessionFamilyID: nil
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
