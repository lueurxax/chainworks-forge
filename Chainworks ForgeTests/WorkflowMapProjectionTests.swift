import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("WorkflowMapProjection", .tags(.fast, .provider))
struct WorkflowMapProjectionTests {
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
}
