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
        agent.resolvedModel = "opus"
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

    @Test("Implementation started recovery prefers stage retry when only lead orchestrator failed before code writer starts")
    func implementationStartedRecoveryPrefersStageRetryOverLeadRetry() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Blocked Implementation", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(),
            status: .blocked,
            iteration: 7,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "freeze_proposal_and_provision_worktree",
            startedAt: Date(),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.stageExecution = stage
        agent.supervisionClassification = .idleHangBeforeFirstProgress
        stage.agentExecutions.append(agent)
        context.insert(agent)

        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: RecoveryActionDetail(
                action: .retryFailedAgent,
                stageID: stage.stageID,
                agentID: agent.agentID,
                explanation: "Retry only the failed agent.",
                staysInSameRun: true,
                reusesSiblingOutputs: true,
                reExecutesWholeStage: false
            ),
            availableActions: [
                RecoveryActionDetail(
                    action: .retryFailedStage,
                    stageID: stage.stageID,
                    agentID: nil,
                    explanation: "Retry the entire implementation_started stage.",
                    staysInSameRun: true,
                    reusesSiblingOutputs: false,
                    reExecutesWholeStage: true
                )
            ],
            validationFailureID: nil,
            source: .runtimePolicy
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(recoveryContext.suggestedAction == .retryStage(stageID: stage.stageID))
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

    @Test("Stage retry coordinator scopes automatic retry lineage to the failed task packet")
    func stageRetryCoordinatorScopesRetryLineageToTaskPacket() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let siblingTaskOne = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "task_a",
            startedAt: Date(timeIntervalSince1970: 101),
            status: .completed,
            provider: "codex",
            effort: "high"
        )
        siblingTaskOne.agentAttemptNumber = 1
        siblingTaskOne.stageExecution = stage
        stage.agentExecutions.append(siblingTaskOne)
        context.insert(siblingTaskOne)

        let siblingTaskTwo = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "task_a",
            startedAt: Date(timeIntervalSince1970: 102),
            status: .completed,
            provider: "codex",
            effort: "high"
        )
        siblingTaskTwo.agentAttemptNumber = 2
        siblingTaskTwo.stageExecution = stage
        stage.agentExecutions.append(siblingTaskTwo)
        context.insert(siblingTaskTwo)

        let targetCompleted = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "task_b",
            startedAt: Date(timeIntervalSince1970: 103),
            status: .completed,
            provider: "codex",
            effort: "high"
        )
        targetCompleted.agentAttemptNumber = 1
        targetCompleted.stageExecution = stage
        stage.agentExecutions.append(targetCompleted)
        context.insert(targetCompleted)

        let failedTarget = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "task_b",
            startedAt: Date(timeIntervalSince1970: 104),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedTarget.agentAttemptNumber = 2
        failedTarget.stageExecution = stage
        stage.agentExecutions.append(failedTarget)
        context.insert(failedTarget)

        let retryCoordinator = StageRetryCoordinator(modelContext: context)
        let retryExec = try retryCoordinator.retryFailedAgent(run: run, stage: stage, failedAgent: failedTarget, retryReason: "automatic_watchdog_retry")

        #expect(retryExec.taskName == "task_b")
        #expect(retryExec.agentAttemptNumber == 3)
        #expect(retryExec.supersedesAgentExecutionID == failedTarget.id)
    }

    @Test("Retry stage targets the latest failed stage attempt for a repeated stage lineage")
    func retryStageTargetsLatestFailedStageAttempt() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .failed)
        context.insert(run)

        let earlierStage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        earlierStage.run = run
        context.insert(earlierStage)

        let latestStage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .blocked,
            iteration: 1,
            attemptNumber: 2
        )
        latestStage.run = run
        context.insert(latestStage)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: "state_8_implementation_continued")

        let retryStage = try #require(
            run.stageExecutions
                .filter { $0.stageID == "state_8_implementation_continued" }
                .max { lhs, rhs in lhs.attemptNumber < rhs.attemptNumber }
        )
        #expect(retryStage.id != earlierStage.id)
        #expect(retryStage.id != latestStage.id)
        #expect(retryStage.attemptNumber == 3)
        #expect(retryStage.status == .ready)
    }

    @Test("Retry stage rebinds continuation cursor and supersedes stale running attempts in the same state iteration")
    func retryStageRebindsCursorAndSupersedesStaleRunningAttempt() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let staleBlockedStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .blocked,
            iteration: 7,
            attemptNumber: 1
        )
        staleBlockedStage.run = run
        context.insert(staleBlockedStage)

        let staleRunningStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 101),
            status: .running,
            iteration: 7,
            attemptNumber: 1
        )
        staleRunningStage.run = run
        context.insert(staleRunningStage)

        let latestBlockedStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 102),
            status: .blocked,
            iteration: 7,
            attemptNumber: 2
        )
        latestBlockedStage.run = run
        context.insert(latestBlockedStage)

        let staleRunningAgent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "freeze_proposal_and_provision_worktree",
            startedAt: Date(timeIntervalSince1970: 103),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        staleRunningAgent.stageExecution = staleRunningStage
        staleRunningStage.agentExecutions.append(staleRunningAgent)
        context.insert(staleRunningAgent)

        run.persistTransitionCursor(
            TransitionCursor(
                sequenceNumber: 12,
                lastCompletedStateID: "state_6_implementation_approval",
                lastCompletedStageExecutionID: UUID(),
                nextScheduledStateID: "state_7_implementation_started",
                nextScheduledIteration: 7,
                nextScheduledAttemptNumber: 2,
                scheduledStageExecutionID: staleRunningStage.id,
                settlementPhase: .transitionStarted,
                updatedAt: Date(timeIntervalSince1970: 104)
            )
        )

        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: "state_7_implementation_started")

        let retriedStages = run.stageExecutions.filter { stage in
            stage.stageID == "state_7_implementation_started"
                && stage.iteration == 7
                && stage.attemptNumber == 3
        }
        let newStage = try #require(retriedStages.first)

        #expect(newStage.status == .ready)
        #expect(run.transitionCursor?.nextScheduledStateID == "state_7_implementation_started")
        #expect(run.transitionCursor?.nextScheduledIteration == 7)
        #expect(run.transitionCursor?.nextScheduledAttemptNumber == 3)
        #expect(run.transitionCursor?.scheduledStageExecutionID == newStage.id)
        #expect(run.transitionCursor?.settlementPhase == .transitionSettled)
        #expect(staleRunningStage.status != .running)
        #expect(staleRunningAgent.status != .running)
        #expect(latestBlockedStage.status == .blocked)
    }

    @Test("Stage retry coordinator snapshot explains exhausted automatic watchdog retries")
    func stageRetryCoordinatorSnapshotExplainsExhaustedWatchdogRetries() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let failedAgent = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date(timeIntervalSince1970: 101),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedAgent.retryReason = "automatic_watchdog_retry"
        failedAgent.agentAttemptNumber = 2
        failedAgent.supervisionClassification = .idleHangAfterFirstEdit
        failedAgent.stageExecution = stage
        stage.agentExecutions.append(failedAgent)
        context.insert(failedAgent)

        let snapshot = StageRetryCoordinator(modelContext: context).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: failedAgent,
            validationFailure: nil
        )

        #expect(snapshot.recommendedAction?.action == .retryFailedAgent)
        #expect(snapshot.recommendedAction?.agentID == failedAgent.agentID)
        #expect(snapshot.recommendedAction?.explanation.localizedCaseInsensitiveContains("automatic watchdog retry") == true)
        #expect(snapshot.recommendedAction?.explanation.localizedCaseInsensitiveContains("first edit") == true)
    }

    @Test("Interrupted transition prefers resume action over invalid retry stage")
    func interruptedTransitionPrefersResumeAction() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Interrupted Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let completedStage = StageExecution(
            stageID: "state_9_implementation_reviewed",
            label: "Implementation reviewed",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        completedStage.run = run
        run.stageExecutions.append(completedStage)
        context.insert(completedStage)

        let continuationStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .ready,
            iteration: 2,
            attemptNumber: 1
        )
        continuationStage.run = run
        run.stageExecutions.append(continuationStage)
        context.insert(continuationStage)

        let staleBlockedStage = StageExecution(
            stageID: "state_10_implementation_refined",
            label: "Implementation refined",
            startedAt: Date(timeIntervalSince1970: 150),
            status: .blocked,
            iteration: 1,
            attemptNumber: 2
        )
        staleBlockedStage.run = run
        run.stageExecutions.append(staleBlockedStage)
        context.insert(staleBlockedStage)

        let staleSnapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: RecoveryActionDetail(
                action: .retryFailedStage,
                stageID: "state_10_implementation_refined",
                agentID: nil,
                explanation: "Stale retry-stage snapshot",
                staysInSameRun: true,
                reusesSiblingOutputs: false,
                reExecutesWholeStage: true
            ),
            availableActions: [],
            validationFailureID: nil,
            source: .runtimePolicy
        )
        staleBlockedStage.recoverySnapshotJSON = try JSONEncoder().encode(staleSnapshot)

        run.persistTransitionCursor(
            TransitionCursor(
                sequenceNumber: 2,
                lastCompletedStateID: "state_9_implementation_reviewed",
                lastCompletedStageExecutionID: completedStage.id,
                nextScheduledStateID: "state_7_implementation_started",
                nextScheduledIteration: 2,
                nextScheduledAttemptNumber: 1,
                scheduledStageExecutionID: continuationStage.id,
                settlementPhase: .transitionSettled,
                updatedAt: Date()
            )
        )

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(actions.first == .resumeInterrupted(stageID: "state_7_implementation_started"))
        #expect(actions.contains(.resumeInterrupted(stageID: "state_7_implementation_started")))
        #expect(!actions.contains(.retryStage(stageID: "state_7_implementation_started")))
        #expect(recoveryContext.suggestedAction == .resumeInterrupted(stageID: "state_7_implementation_started"))
        #expect(recoveryContext.reason.contains("state_7_implementation_started"))
    }

    @Test("Interrupted drift without cursor still prefers resume action")
    func interruptedDriftWithoutCursorPrefersResumeAction() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Interrupted Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        run.driftDetails = "Workflow source has changed (hash mismatch); Agent catalog source has changed (hash mismatch) Run was interrupted by app restart before reaching a terminal state. Use Resume Interrupted to continue."
        idea.runs.append(run)
        context.insert(run)

        let completedStage = StageExecution(
            stageID: "state_9_implementation_reviewed",
            label: "Implementation reviewed",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        completedStage.run = run
        run.stageExecutions.append(completedStage)
        context.insert(completedStage)

        let continuationStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .blocked,
            iteration: 2,
            attemptNumber: 1
        )
        continuationStage.run = run
        run.stageExecutions.append(continuationStage)
        context.insert(continuationStage)

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(actions.first == .resumeInterrupted(stageID: "state_7_implementation_started"))
        #expect(actions.contains(.resumeInterrupted(stageID: "state_7_implementation_started")))
        #expect(recoveryContext.suggestedAction == .resumeInterrupted(stageID: "state_7_implementation_started"))
    }

    @Test("Exhausted watchdog retry truth beats stale interrupted continuation cursor")
    func exhaustedWatchdogRetryBeatsInterruptedContinuationCursor() throws {
        let context = try makeRecoveryContext()
        let idea = Idea(title: "Watchdog Idea", body: "Body", status: .active)
        context.insert(idea)

        let run = makeRun(status: .blocked)
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        let failedStage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .blocked,
            iteration: 3,
            attemptNumber: 1
        )
        failedStage.run = run
        run.stageExecutions.append(failedStage)
        context.insert(failedStage)

        let failedRetry = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date(timeIntervalSince1970: 210),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedRetry.completedAt = Date(timeIntervalSince1970: 240)
        failedRetry.retryReason = "automatic_watchdog_retry"
        failedRetry.agentAttemptNumber = 2
        failedRetry.canonicalOutcome = .failedBeforeOutput
        failedRetry.supervisionClassification = .idleHangAfterFirstEdit
        failedRetry.stageExecution = failedStage
        failedStage.agentExecutions.append(failedRetry)
        context.insert(failedRetry)

        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: RecoveryActionDetail(
                action: .retryFailedAgent,
                stageID: failedStage.stageID,
                agentID: failedRetry.agentID,
                explanation: "Automatic watchdog retry already consumed; retry the failed code writer explicitly.",
                staysInSameRun: true,
                reusesSiblingOutputs: true,
                reExecutesWholeStage: false
            ),
            availableActions: [],
            validationFailureID: nil,
            source: .runtimePolicy
        )
        failedStage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)

        let staleContinuation = StageExecution(
            stageID: "state_9_implementation_reviewed",
            label: "Implementation reviewed",
            startedAt: Date(timeIntervalSince1970: 250),
            status: .ready,
            iteration: 3,
            attemptNumber: 1
        )
        staleContinuation.run = run
        run.stageExecutions.append(staleContinuation)
        context.insert(staleContinuation)

        run.persistTransitionCursor(
            TransitionCursor(
                sequenceNumber: 4,
                lastCompletedStateID: failedStage.stageID,
                lastCompletedStageExecutionID: failedStage.id,
                nextScheduledStateID: staleContinuation.stageID,
                nextScheduledIteration: staleContinuation.iteration,
                nextScheduledAttemptNumber: staleContinuation.attemptNumber,
                scheduledStageExecutionID: staleContinuation.id,
                settlementPhase: .transitionSettled,
                updatedAt: Date()
            )
        )

        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let recoveryContext = coordinator.recoveryContext(for: run)

        #expect(actions.first == .retryAgent(stageID: failedStage.stageID, agentID: failedRetry.agentID))
        #expect(!actions.contains(.resumeInterrupted(stageID: staleContinuation.stageID)))
        #expect(recoveryContext.suggestedAction == .retryAgent(stageID: failedStage.stageID, agentID: failedRetry.agentID))
        #expect(recoveryContext.reason.localizedCaseInsensitiveContains("first edit"))
        #expect(recoveryContext.failureClass == SupervisionClassification.idleHangAfterFirstEdit.rawValue)
    }

    @Test("Evidence packet prefers exhausted watchdog retry stage over a later generic blocked stage")
    func evidencePacketPrefersExhaustedWatchdogStage() throws {
        let context = try makeRecoveryContext()
        let run = makeRun(status: .blocked)
        context.insert(run)

        let watchdogStage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .blocked,
            iteration: 1,
            attemptNumber: 1
        )
        watchdogStage.run = run
        context.insert(watchdogStage)

        let watchdogAgent = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date(timeIntervalSince1970: 101),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        watchdogAgent.completedAt = Date(timeIntervalSince1970: 120)
        watchdogAgent.supervisionClassification = .idleHangAfterFirstEdit
        watchdogAgent.retryReason = "automatic_watchdog_retry"
        watchdogAgent.stageExecution = watchdogStage
        context.insert(watchdogAgent)

        let laterStage = StageExecution(
            stageID: "state_9_implementation_reviewed",
            label: "Implementation reviewed",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .blocked,
            iteration: 1,
            attemptNumber: 1
        )
        laterStage.run = run
        context.insert(laterStage)

        let laterAgent = AgentExecution(
            agentID: "reviewer",
            agentTitle: "Reviewer",
            taskName: "review",
            startedAt: Date(timeIntervalSince1970: 201),
            status: .failed,
            provider: "claude",
            effort: "high"
        )
        laterAgent.completedAt = Date(timeIntervalSince1970: 220)
        laterAgent.logSnippet = "generic blocked stage"
        laterAgent.stageExecution = laterStage
        context.insert(laterAgent)
        try context.save()

        let packet = try #require(RecoveryCoordinator(modelContext: context).buildEvidencePacket(for: run))
        #expect(packet.stageID == watchdogStage.stageID)
        #expect(packet.supervisionClassification == .idleHangAfterFirstEdit)
    }
}

private func makeRecoveryContext() throws -> ModelContext {
    let config = ModelConfiguration("RecoveryTests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        configurations: config
    )
    TestModelContainerRetainer.retain(container)
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
            runtime: "claude_agent",
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
                model: "opus",
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
