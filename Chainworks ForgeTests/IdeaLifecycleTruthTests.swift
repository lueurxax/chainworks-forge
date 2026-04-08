import Foundation
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Idea Lifecycle Truth")
struct IdeaLifecycleTruthTests {
    @Test("Cancelled latest run is shown as cancelled instead of active")
    func cancelledLatestRunUsesTruthfulLifecycleLabel() {
        let idea = Idea(title: "Cancelled Idea", body: "Body", status: .active)
        let run = makeRun(status: .cancelled)
        run.idea = idea
        idea.runs = [run]

        #expect(idea.lifecycleStatusLabel == "Cancelled")
        #expect(idea.archiveLifecycleStatus == "Cancelled")
        #expect(idea.latestRunIsTerminal)
    }

    @Test("Persisted legacy idea status syncs from latest terminal run")
    func synchronizePersistedStatusFromRunsUsesLatestRun() {
        let idea = Idea(title: "Terminal Idea", body: "Body", status: .active)
        let run = makeRun(status: .completed)
        run.idea = idea
        idea.runs = [run]

        idea.synchronizePersistedStatusFromRuns()

        #expect(idea.status == .completed)
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
}
