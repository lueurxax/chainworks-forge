import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("WorkflowMapProjection", .tags(.fast, .provider))
struct WorkflowMapProjectionTests {
    @Test("Projection reuses cached frozen plan for repeated requests on the same run")
    mutating func projectionReusesCachedFrozenPlan() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        WorkflowMapProjectionService.resetPlanCacheForTesting()

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        let firstProjection = try #require(service.projection(for: run))
        let secondProjection = try #require(service.projection(for: run))

        #expect(firstProjection.workflowID == secondProjection.workflowID)
        #expect(WorkflowMapProjectionService.planCacheMissCountForTesting == 1)
        #expect(WorkflowMapProjectionService.cachedPlanCountForTesting == 1)
    }

    @Test("Projection loads persisted stage snapshots only once per request")
    mutating func projectionLoadsStageSnapshotsOncePerRequest() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        RunStageSnapshotLoader.resetLoadInvocationCountForTesting()
        RunStageSnapshotLoader.resetCacheForTesting()

        _ = try #require(service.projection(for: run))

        #expect(RunStageSnapshotLoader.loadInvocationCountForTesting == 1)
    }

    @Test("Snapshot loader reuses hot cache for repeated run status reads")
    mutating func snapshotLoaderReusesHotCacheForRepeatedRunReads() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        RunStageSnapshotLoader.resetLoadInvocationCountForTesting()
        RunStageSnapshotLoader.resetCacheForTesting()

        _ = RunStageSnapshotLoader.load(for: run)
        _ = RunStageSnapshotLoader.load(for: run)

        #expect(RunStageSnapshotLoader.loadInvocationCountForTesting == 1)
        #expect(RunStageSnapshotLoader.cacheEntryCountForTesting == 1)
    }

    @Test("Lightweight run status avoids full stage snapshot loading")
    mutating func lightweightRunStatusAvoidsFullSnapshotLoading() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        RunStageSnapshotLoader.resetLoadInvocationCountForTesting()
        RunStageSnapshotLoader.resetCacheForTesting()
        RunLatestStageStatusLoader.resetLoadInvocationCountForTesting()
        RunLatestStageStatusLoader.resetCacheForTesting()

        _ = service.runStatus(for: run)

        #expect(RunStageSnapshotLoader.loadInvocationCountForTesting == 0)
        #expect(RunLatestStageStatusLoader.loadInvocationCountForTesting == 1)
    }

    @Test("Lightweight current stage summary avoids full stage snapshot loading")
    mutating func lightweightCurrentStageSummaryAvoidsFullSnapshotLoading() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        RunStageSnapshotLoader.resetLoadInvocationCountForTesting()
        RunStageSnapshotLoader.resetCacheForTesting()
        RunLatestStageStatusLoader.resetLoadInvocationCountForTesting()
        RunLatestStageStatusLoader.resetCacheForTesting()

        let summary = service.currentStageSummary(for: run)

        #expect(summary != nil)
        #expect(summary?.stageID.isEmpty == false)
        #expect(summary?.label.isEmpty == false)
        #expect(RunStageSnapshotLoader.loadInvocationCountForTesting == 0)
        #expect(RunLatestStageStatusLoader.loadInvocationCountForTesting == 1)
    }

    @Test("Projection run status prefers stored blocked truth over stale running stage without live retry")
    mutating func projectionRunStatusPrefersStoredBlockedTruthOverStaleRunningStage() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context, workflowID: "wf", workflowTitle: "WF")
        run.status = .blocked
        context.insert(run)

        let blockedStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .blocked
        )
        blockedStage.run = run
        context.insert(blockedStage)

        let staleRetryStage = StageExecution(
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .running
        )
        staleRetryStage.run = run
        context.insert(staleRetryStage)

        let failedAgent = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date(timeIntervalSince1970: 21),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedAgent.completedAt = Date(timeIntervalSince1970: 30)
        failedAgent.logSnippet = "Execution did not produce final output"
        failedAgent.stageExecution = staleRetryStage
        context.insert(failedAgent)

        let executionService = ExecutionService(modelContext: context, executor: SimulatedAgentExecutor())
        let service = WorkflowMapProjectionService(modelContext: context, executionService: executionService)

        #expect(service.runStatus(for: run) == .blocked)
    }

    @Test("Projection derives topology, handoffs, loops, and agent panels from frozen snapshot")
    mutating func projectionDerivesRuntimeMap() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        guard let run = try container.mainContext.fetch(descriptor).first else {
            Issue.record("Expected a seeded run for the workflow map preview")
            return
        }

        let projection = service.projection(for: run)
        #expect(projection != nil)
        #expect(projection?.stageCount == 6)
        #expect(projection?.currentStageLabel == "Proposal reviewed")
        #expect((projection?.activeOccurrenceCount ?? 0) > 0)
        #expect((projection?.completedOccurrenceCount ?? 0) > 0)
        #expect((projection?.pendingOccurrenceCount ?? 0) > 0)
        #expect((projection?.communicationCount ?? 0) > 0)
        #expect((projection?.loops.count ?? 0) == 1)
        #expect((projection?.edges.contains(where: { $0.kind == .transition }) ?? false))
        #expect((projection?.edges.contains(where: { $0.kind == .fanout || $0.kind == .join || $0.kind == .sequence }) ?? false))
    }

    @Test("Projection provides persisted timeline fallback when no live orchestrator is attached")
    mutating func projectionProvidesPersistedTimelineFallback() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        let projection = try #require(service.projection(for: run))
        #expect(projection.liveTimeline.isEmpty)
        #expect(!projection.persistedTimeline.isEmpty)
        #expect(projection.persistedTimeline.contains { $0.detail.contains("Persisted") })
    }

    @Test("Projection reads live timeline without triggering stalled-orchestrator reconcile")
    mutating func projectionDoesNotTriggerReconcileSideEffects() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)

        let reconcileCountBefore = executionService.reconcileInvocationCountForTesting
        _ = try #require(service.projection(for: run))
        let reconcileCountAfter = executionService.reconcileInvocationCountForTesting

        #expect(reconcileCountAfter == reconcileCountBefore)
    }

    @Test("ExecutionService UI counters do not trigger stalled-orchestrator reconcile")
    mutating func executionServiceUICountersAvoidReconcileSideEffects() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

        let reconcileCountBefore = executionService.reconcileInvocationCountForTesting
        _ = executionService.hasActiveRuns
        _ = executionService.blockedRunCount
        _ = executionService.failedRunCount
        let reconcileCountAfter = executionService.reconcileInvocationCountForTesting

        #expect(reconcileCountAfter == reconcileCountBefore)
    }

    @Test("Projection persists runtime session identifiers into the focused timeline fallback")
    mutating func projectionPersistsRuntimeSessionIdentifiersIntoTimelineFallback() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)
        let stage = try #require(run.stageExecutions.last)
        let agent = try #require(stage.agentExecutions.first)
        agent.runtimeSessionID = "persisted-session-123"
        try container.mainContext.save()

        let projection = try #require(service.projection(for: run))
        let persistedSessionEntry = try #require(
            projection.persistedTimeline.filter { $0.sessionID == "persisted-session-123" }.first
        )
        #expect(persistedSessionEntry.detail.contains("persisted-session-123"))
    }

    @Test("Projection persists watchdog supervision and automatic retry history into the focused timeline data path")
    mutating func projectionIncludesWatchdogRetryHistoryInPersistedTimeline() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)
        let stage = try #require(run.stageExecutions.first)

        let failedAttempt = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date().addingTimeInterval(-60),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedAttempt.supervisionClassification = .idleHangAfterFirstEdit
        failedAttempt.canonicalOutcome = .failedBeforeOutput
        failedAttempt.stageExecution = stage

        let retryAttempt = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date().addingTimeInterval(-30),
            status: .running,
            provider: "codex",
            effort: "high"
        )
        retryAttempt.retryReason = "automatic_watchdog_retry"
        retryAttempt.agentAttemptNumber = 2
        retryAttempt.supersedesAgentExecutionID = failedAttempt.id
        retryAttempt.stageExecution = stage

        container.mainContext.insert(failedAttempt)
        container.mainContext.insert(retryAttempt)
        try container.mainContext.save()

        let projection = try #require(service.projection(for: run))
        #expect(projection.persistedTimeline.contains { $0.detail.contains("Execution stalled after the first edit boundary") })
        #expect(projection.persistedTimeline.contains { $0.detail.contains("automatic watchdog retry") })
    }

    @Test("Projection persists exhausted watchdog retry outcome and next action into the focused timeline data path")
    mutating func projectionIncludesExhaustedWatchdogRetryOutcomeAndNextAction() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)
        let stage = try #require(run.stageExecutions.first)
        stage.status = .blocked

        let retryAttempt = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date().addingTimeInterval(-15),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        retryAttempt.retryReason = "automatic_watchdog_retry"
        retryAttempt.agentAttemptNumber = 2
        retryAttempt.supervisionClassification = .idleHangAfterFirstEdit
        retryAttempt.stageExecution = stage

        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: RecoveryActionDetail(
                action: .retryFailedAgent,
                stageID: stage.stageID,
                agentID: retryAttempt.agentID,
                explanation: "Automatic watchdog retry already consumed after a first edit stall. Retry the failed code writer explicitly.",
                staysInSameRun: true,
                reusesSiblingOutputs: true,
                reExecutesWholeStage: false
            ),
            availableActions: [],
            validationFailureID: nil,
            source: .runtimePolicy
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)

        container.mainContext.insert(retryAttempt)
        try container.mainContext.save()

        let projection = try #require(service.projection(for: run))
        #expect(projection.persistedTimeline.contains { $0.detail.contains("automatic watchdog retry exhausted") })
        #expect(projection.persistedTimeline.contains { $0.detail.contains("Automatic watchdog retry already consumed") })
    }

    @Test("Projection prefers live retry activity over stale terminal persisted stage truth")
    mutating func projectionPrefersLiveRetryOverStaleBlockedRunTruth() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let context = container.mainContext
        let executionService = PreviewSupport.makeExecutionService(modelContext: context)
        let service = WorkflowMapProjectionService(
            modelContext: context,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(context.fetch(descriptor).first)
        let compiler = RunPlanCompiler(modelContext: context)
        let (plan, _) = try compiler.rebuildPlanFromSnapshot(run: run)
        let catalog = try loadTestCanonicalCatalog()

        run.status = .running
        let blockedStage = try #require(run.stageExecutions.sorted { $0.startedAt < $1.startedAt }.last)
        blockedStage.status = .blocked
        blockedStage.completedAt = Date(timeIntervalSince1970: 110)
        try context.save()

        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot, isDirectory: true),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot, isDirectory: true),
            worktreeRoot: run.worktreeRoot.map { URL(fileURLWithPath: $0, isDirectory: true) }
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context,
            catalog: catalog
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "code_writer",
            event: ExecutionEvent(
                type: .textChunk,
                timestamp: Date(timeIntervalSince1970: 200),
                detail: "Retry is streaming"
            ),
            now: Date(timeIntervalSince1970: 200)
        )
        executionService.registerTestingOrchestrator(orchestrator)

        let projection = try #require(service.projection(for: run))
        #expect(projection.runStatus == .running)
        #expect(projection.currentStageID == plan.initialStateID)
        #expect(projection.liveTimeline.isEmpty == false)
    }

    @Test("Lightweight run status prefers live retry activity over stale terminal persisted stage truth")
    mutating func lightweightRunStatusPrefersLiveRetryOverStaleBlockedTruth() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let context = container.mainContext
        let executionService = PreviewSupport.makeExecutionService(modelContext: context)
        let service = WorkflowMapProjectionService(
            modelContext: context,
            executionService: executionService
        )

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(context.fetch(descriptor).first)
        let compiler = RunPlanCompiler(modelContext: context)
        let (plan, _) = try compiler.rebuildPlanFromSnapshot(run: run)
        let catalog = try loadTestCanonicalCatalog()

        run.status = .running
        let blockedStage = try #require(run.stageExecutions.sorted { $0.startedAt < $1.startedAt }.last)
        blockedStage.status = .blocked
        blockedStage.completedAt = Date(timeIntervalSince1970: 110)
        try context.save()

        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot, isDirectory: true),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot, isDirectory: true),
            worktreeRoot: run.worktreeRoot.map { URL(fileURLWithPath: $0, isDirectory: true) }
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context,
            catalog: catalog
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "code_writer",
            event: ExecutionEvent(
                type: .textChunk,
                timestamp: Date(timeIntervalSince1970: 200),
                detail: "Retry is streaming"
            ),
            now: Date(timeIntervalSince1970: 200)
        )
        executionService.registerTestingOrchestrator(orchestrator)

        #expect(service.runStatus(for: run) == .running)
    }

    @Test("Snapshot loader preserves transport and output truth for persisted agent attempts")
    mutating func snapshotLoaderPreservesTransportAndOutputTruth() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(descriptor).first)
        let stage = try #require(run.stageExecutions.first)

        let failedAttempt = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "continue_implementation",
            startedAt: Date(timeIntervalSince1970: 300),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedAttempt.canonicalOutcome = .timedOutBeforeOutput
        failedAttempt.supervisionClassification = .idleHangAfterFirstEdit
        failedAttempt.transportErrorKind = .timeout
        failedAttempt.outputPresence = .none
        failedAttempt.stageExecution = stage
        stage.agentExecutions.append(failedAttempt)
        container.mainContext.insert(failedAttempt)
        try container.mainContext.save()

        RunStageSnapshotLoader.resetCacheForTesting()
        let snapshots = RunStageSnapshotLoader.load(for: run, modelContext: container.mainContext)
        let snapshotAgent = try #require(
            snapshots
                .flatMap(\.agentExecutions)
                .first(where: { $0.id == failedAttempt.id })
        )

        #expect(snapshotAgent.transportErrorKind == .timeout)
        #expect(snapshotAgent.outputPresence == .none)
    }

    @Test("Projection ignores deleted stage rows that still linger in relationship memory")
    mutating func projectionIgnoresDeletedStageRows() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let runDescriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(runDescriptor).first)

        let stageDescriptor = FetchDescriptor<StageExecution>()
        let stages = try container.mainContext.fetch(stageDescriptor).filter { $0.run?.id == run.id }
        let originalCount = stages.count
        let doomedStage = try #require(stages.last)

        container.mainContext.delete(doomedStage)
        try container.mainContext.save()

        let projection = try #require(service.projection(for: run))
        let persistedStageEntries = projection.persistedTimeline.filter { $0.id.hasPrefix("stage::") }
        #expect(persistedStageEntries.count == originalCount - 1)
    }

    @Test("Projection survives stage deletion from a sibling model context")
    mutating func projectionSurvivesSiblingContextStageDeletion() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let runDescriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(runDescriptor).first)

        let siblingContext = ModelContext(container)
        let siblingStages = try siblingContext.fetch(FetchDescriptor<StageExecution>())
            .filter { $0.run?.id == run.id }
        let doomedStage = try #require(siblingStages.last)
        siblingContext.delete(doomedStage)
        try siblingContext.save()

        let projection = try #require(service.projection(for: run))
        let persistedStageEntries = projection.persistedTimeline.filter { $0.id.hasPrefix("stage::") }
        #expect(!persistedStageEntries.isEmpty)
    }

    @Test("Projection suppresses stale downstream running stages after the run rewinds to an earlier stage")
    mutating func projectionSuppressesStaleFutureRunningStagesAfterRewind() throws {
        let container = PreviewSupport.makeModelContainer(seed: { context in
            PreviewSupport.seedWorkflowMapPreviewData(context: context)
        })
        let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
        let service = WorkflowMapProjectionService(
            modelContext: container.mainContext,
            executionService: executionService
        )

        let runDescriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let run = try #require(container.mainContext.fetch(runDescriptor).first)

        let existingStages = try container.mainContext.fetch(FetchDescriptor<StageExecution>())
            .filter { $0.run?.id == run.id }
        for stage in existingStages {
            container.mainContext.delete(stage)
        }

        let now = Date()

        let firstIdeaPass = StageExecution(
            stageID: "state_1_idea_received",
            label: "Idea received",
            startedAt: now.addingTimeInterval(-600),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        firstIdeaPass.run = run
        firstIdeaPass.completedAt = now.addingTimeInterval(-540)

        let staleProposalStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: now.addingTimeInterval(-530),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        staleProposalStage.run = run

        let staleWriter = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: now.addingTimeInterval(-528),
            status: .running,
            provider: "codex",
            effort: "high"
        )
        staleWriter.stageExecution = staleProposalStage
        staleProposalStage.agentExecutions.append(staleWriter)

        let rewoundIdeaPass = StageExecution(
            stageID: "state_1_idea_received",
            label: "Idea received",
            startedAt: now.addingTimeInterval(-120),
            status: .running,
            iteration: 2,
            attemptNumber: 1
        )
        rewoundIdeaPass.run = run

        let liveLead = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "normalize_idea_and_prepare_proposal_brief",
            startedAt: now.addingTimeInterval(-118),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        liveLead.stageExecution = rewoundIdeaPass
        rewoundIdeaPass.agentExecutions.append(liveLead)

        run.stageExecutions.append(contentsOf: [firstIdeaPass, staleProposalStage, rewoundIdeaPass])
        try container.mainContext.save()

        let projection = try #require(service.projection(for: run))
        let currentStage = try #require(projection.stages.first(where: { $0.id == "state_1_idea_received" }))
        let futureStage = try #require(projection.stages.first(where: { $0.id == "state_2_proposal_drafted" }))

        #expect(projection.currentStageID == "state_1_idea_received")
        #expect(currentStage.status == .running)
        #expect(futureStage.status != .running)
        #expect(futureStage.occurrences.contains(where: { $0.state == .thinking }) == false)
    }
}
