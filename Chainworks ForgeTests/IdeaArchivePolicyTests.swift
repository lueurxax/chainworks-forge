import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

private func makeArchiveContext() throws -> ModelContext {
    let config = ModelConfiguration("ArchiveTests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        configurations: config
    )
    return ModelContext(container)
}

@MainActor
@Suite("Idea Archive Policy")
struct IdeaArchivePolicyTests {
    @Test func draftIdeaCanArchive() throws {
        let idea = Idea(title: "Draft", body: "Body", status: .draft)

        #expect(IdeaArchivePolicy.eligibility(for: idea).canArchive)
    }

    @Test func activeRunBlocksArchive() throws {
        let context = try makeArchiveContext()
        let idea = Idea(title: "Active", body: "Body", status: .active)
        context.insert(idea)

        let run = Run(
            startedAt: Date(),
            status: .running,
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
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)

        #expect(!IdeaArchivePolicy.eligibility(for: idea).canArchive)
    }

    @Test func archivedIdeaCanRestore() throws {
        let context = try makeArchiveContext()
        let idea = Idea(title: "Archived", body: "Body", archivedAt: Date(), status: .completed)
        context.insert(idea)

        let service = IdeaArchiveService(modelContext: context)
        try service.restore(idea)

        #expect(idea.archivedAt == nil)
        #expect(!idea.isArchived)
    }
}
