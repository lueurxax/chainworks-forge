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
