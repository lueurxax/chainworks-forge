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

    @Test("Retry stage preserves lineage and rotates active owner token")
    func retryStagePreservesLineageAndRotatesOwnerToken() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "state_2_proposal_drafted::iteration:1"
        stage.activeOwnerToken = "owner-old"
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: "state_2_proposal_drafted")

        let retryStage = try #require(run.stageExecutions.first(where: { $0.attemptNumber == 2 }))
        #expect(retryStage.lineageID == stage.lineageID)
        #expect(retryStage.activeOwnerToken != nil)
        #expect(retryStage.activeOwnerToken != "owner-old")
        #expect(stage.activeOwnerToken == nil)
        #expect(stage.settlementKind == .superseded)
        #expect(stage.settledAt != nil)
    }

    @Test("Recovery coordinator resolves retry target from canonical stage lineage")
    func recoveryCoordinatorUsesCanonicalStageLineage() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let staleStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -120),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        staleStage.lineageID = "draft-lineage"
        staleStage.settlementKind = .repaired
        staleStage.settledAt = Date(timeIntervalSinceNow: -90)
        staleStage.run = run
        run.stageExecutions.append(staleStage)
        context.insert(staleStage)

        let staleAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -119),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        staleAgent.stageExecution = staleStage
        staleStage.agentExecutions.append(staleAgent)
        context.insert(staleAgent)

        let canonicalStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .failed,
            iteration: 1,
            attemptNumber: 2
        )
        canonicalStage.lineageID = "draft-lineage"
        canonicalStage.run = run
        run.stageExecutions.append(canonicalStage)
        context.insert(canonicalStage)

        let canonicalAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -59),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        canonicalAgent.stageExecution = canonicalStage
        canonicalStage.agentExecutions.append(canonicalAgent)
        context.insert(canonicalAgent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: "state_2_proposal_drafted")

        let retryStage = try #require(run.stageExecutions.first(where: { $0.attemptNumber == 3 }))
        #expect(retryStage.lineageID == "draft-lineage")
        #expect(retryStage.supersedesAttemptNumber == 2)
        #expect(canonicalStage.settlementKind == .superseded)
        #expect(staleStage.settlementKind == .repaired)
    }

    @Test("Blocked run with failed aggregate step offers retry aggregate before clone")
    func blockedRunWithAggregateFailureOffersRetryAggregate() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
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

        let aggregateAgent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        aggregateAgent.stageExecution = stage
        stage.agentExecutions.append(aggregateAgent)
        context.insert(aggregateAgent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)

        #expect(actions.contains(.retryAggregateStep(stageID: "state_4_proposal_reviewed")))
        #expect(!actions.contains(.retryAgent(stageID: "state_4_proposal_reviewed", agentID: "lead_orchestrator")))

        _ = try coordinator.retryAggregateStep(run: run, stageID: "state_4_proposal_reviewed")

        let retryExec = try #require(stage.agentExecutions.sorted(by: { $0.startedAt < $1.startedAt }).last)
        #expect(retryExec.agentID == "lead_orchestrator")
        #expect(retryExec.agentAttemptNumber == 2)
        #expect(run.status == .running)
    }

    @Test("Blocked run with subordinate aggregate settlement record still offers retry aggregate")
    func blockedRunWithAggregateSettlementRecordOffersRetryAggregate() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "review-lineage"
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let aggregateAgent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: Date(),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        aggregateAgent.agentAttemptNumber = 1
        aggregateAgent.stageExecution = stage
        stage.agentExecutions.append(aggregateAgent)
        context.insert(aggregateAgent)

        let record = AggregateSettlementRecord(
            runID: run.id,
            stageExecutionID: stage.id,
            aggregateStepID: "aggregate_proposal_reviews",
            lineageID: "review-lineage",
            canonicalOutcome: .failedBeforeOutput
        )
        context.insert(record)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)

        #expect(actions.contains(.retryAggregateStep(stageID: "state_4_proposal_reviewed")))

        _ = try coordinator.retryAggregateStep(run: run, stageID: "state_4_proposal_reviewed")

        let retryExec = try #require(stage.agentExecutions.sorted(by: { $0.startedAt < $1.startedAt }).last)
        #expect(retryExec.agentID == "lead_orchestrator")
        #expect(retryExec.agentAttemptNumber == 2)
        #expect(run.status == .running)
    }

    @Test("Canonical transport-error completion remains eligible for same-run retry")
    func completedWithTransportErrorStillSurfacesRetryAction() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
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
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .completedWithTransportError
        agent.transportErrorKind = .stream
        agent.outputPresence = .durableOutput
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)

        #expect(actions.contains(.retryAgent(stageID: "state_2_proposal_drafted", agentID: "proposal_writer")))
    }

    @Test("Limit exhaustion defaults to clone-only recovery actions")
    func limitExhaustionSuppressesSameRunRetryActions() throws {
        let context = try makeRecoveryContext()
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
        run.stageExecutions.append(stage)
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
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(!actions.contains(.retryStage(stageID: "state_5_proposal_refined")))
        #expect(!actions.contains(.retryAgent(stageID: "state_5_proposal_refined", agentID: "proposal_writer")))
        #expect(actions.contains(.cloneRunFrozenSnapshot))
        #expect(recoveryContext.suggestedAction == nil)
    }

    @Test("Policy-bound stop defaults to clone-only recovery actions")
    func policyBoundStopSuppressesSameRunRetryActions() throws {
        let context = try makeRecoveryContext()
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
        run.stageExecutions.append(stage)
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
        agent.outputPresence = .none
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(!actions.contains(.retryStage(stageID: "state_5_proposal_refined")))
        #expect(!actions.contains(.retryAgent(stageID: "state_5_proposal_refined", agentID: "proposal_writer")))
        #expect(actions.contains(.cloneRunCurrentConfig))
        #expect(recoveryContext.suggestedAction == nil)
    }

    @Test("Recovery context includes binding summary when runtime differs from frozen snapshot")
    func recoveryContextIncludesBindingSummaryWhenRuntimeDiffersFromFrozenSnapshot() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.unverifiable.rawValue
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([
            "lead_orchestrator": ResolvedProviderBinding(
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
        ])
        run.bindingProvenanceJSON = try JSONEncoder().encode([
            "lead_orchestrator": FrozenBindingProvenance(
                source: .backendProfileDefault,
                backendProfileID: "lead_profile",
                backendProfileModel: "claude-3-5-sonnet",
                configuredProviderID: nil,
                configuredProviderDefaultModel: "claude-3-5-sonnet",
                runOverrideModel: nil,
                resolvedModel: "claude-3-5-sonnet",
                resolvedProviderFamily: "claude_code"
            )
        ])
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
        run.stageExecutions.append(stage)
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "refine_proposal",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.canonicalOutcome = .limitExhaustedAfterOutput
        agent.runtimeProvider = "claude_code"
        agent.runtimeModel = "claude-3-7-sonnet"
        agent.outputPresence = .durableOutput
        agent.logSnippet = "Provider or app limit exhausted after output was produced"
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let recoveryContext = RecoveryCoordinator(modelContext: context).recoveryContext(for: run)

        #expect(recoveryContext.bindingSummary?.contains("frozen=claude_code/claude-3-5-sonnet") == true)
        #expect(recoveryContext.bindingSummary?.contains("runtime=claude_code/claude-3-7-sonnet") == true)
        #expect(recoveryContext.bindingSummary?.localizedCaseInsensitiveContains("unverifiable") == true)
    }
}

private func makeRecoveryContext() throws -> ModelContext {
    let config = ModelConfiguration("RecoveryTests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, AggregateSettlementRecord.self, Artifact.self,
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
                    Transition(to: "state_end", when: "always")
                ]
            ),
            "state_end": WorkflowState(
                label: "End",
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
                type: "markdown",
                path: "/tmp/orchestrator.md",
                name: "Orchestrator Core",
                description: "test"
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
                filesystem: FilesystemPermissions(read: [], write: [], deny: nil),
                git: GitPermissions(status: true, diff: true, checkout: false, commit: false, push: false),
                shell: ShellPermissions(allow: [], deny: nil),
                network: NetworkPermissions(allow: []),
                mcp: MCPPermissions(allow: [])
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
