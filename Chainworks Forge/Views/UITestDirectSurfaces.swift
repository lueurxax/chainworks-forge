import SwiftUI
import SwiftData

struct UITestIdeaArchiveSurface: View {
    @Environment(\.modelContext) private var modelContext

    private var seededIdeaTitle: String? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]
    }

    private var seededIdea: Idea? {
        guard let seededIdeaTitle else { return nil }
        let descriptor = FetchDescriptor<Idea>()
        return (try? modelContext.fetch(descriptor))?.first(where: { $0.title == seededIdeaTitle })
    }

    var body: some View {
        Group {
            if let seededIdea {
                IdeaDetailView(idea: seededIdea)
            } else {
                ContentUnavailableView(
                    "Seeded idea unavailable",
                    systemImage: "archivebox",
                    description: Text("The UI test archive surface requires a seeded idea.")
                )
            }
        }
        .accessibilityIdentifier("ui-test-idea-archive-surface")
    }
}

struct UITestWorkflowMapSurface: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService

    private var seededIdeaTitle: String? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]
    }

    private var targetRun: Run? {
        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let runs = (try? modelContext.fetch(descriptor)) ?? []
        if let seededIdeaTitle {
            return runs.first(where: { $0.idea?.title == seededIdeaTitle }) ?? runs.first
        }
        return runs.first
    }

    var body: some View {
        Group {
            if let targetRun {
                WorkflowRunProgressView(run: targetRun)
                    .environment(executionService)
            } else {
                ContentUnavailableView(
                    "Workflow map unavailable",
                    systemImage: "chart.xyaxis.line",
                    description: Text("The UI test workflow map surface requires a seeded run.")
                )
            }
        }
        .accessibilityIdentifier("ui-test-workflow-map-surface")
    }
}
