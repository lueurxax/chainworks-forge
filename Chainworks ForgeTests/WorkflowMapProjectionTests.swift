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
}
