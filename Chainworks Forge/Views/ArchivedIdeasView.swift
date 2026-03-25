import SwiftUI
import SwiftData

struct ArchivedIdeasView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.modelContext) private var modelContext
    @Query(sort: \Idea.createdAt, order: .reverse) private var ideas: [Idea]

    @State private var restoreMessage: String?
    @State private var searchText = ""

    private var archivedIdeas: [Idea] {
        ideas.filter(\.isArchived)
    }

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 10) {
                TextField("Search archived ideas", text: $searchText)
                    .textFieldStyle(.roundedBorder)
                    .padding(.horizontal)
                    .padding(.top, 8)
                    .accessibilityIdentifier("ideas-archive-search")

                IdeasArchiveView(
                    ideas: archivedIdeas,
                    searchText: searchText,
                    onRestore: restore
                )
                if let restoreMessage {
                    Text(restoreMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal)
                }
            }
            .navigationSplitViewColumnWidth(min: 260, ideal: 320)
            .navigationTitle("Archived Ideas")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        } detail: {
            ContentUnavailableView(
                "Select an archived idea",
                systemImage: "archivebox",
                description: Text("Review archived ideas and restore them when they become relevant again.")
            )
        }
        .frame(minWidth: 920, minHeight: 640)
    }

    @MainActor
    private func restore(_ idea: Idea) {
        let service = IdeaArchiveService(modelContext: modelContext)
        do {
            try service.restore(idea)
            restoreMessage = "Restored \(idea.title)"
        } catch {
            restoreMessage = error.localizedDescription
        }
    }
}

#Preview("Archived Ideas — Seeded") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return ArchivedIdeasView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .frame(width: 1080, height: 720)
}
