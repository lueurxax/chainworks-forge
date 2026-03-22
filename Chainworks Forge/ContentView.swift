import SwiftUI
import SwiftData

struct ContentView: View {
    @State private var selectedTab: Tab = .ideas

    enum Tab: String, CaseIterable {
        case ideas = "Ideas"
        case agentCatalog = "Agent Catalog"
        case workflowInspector = "Workflow Inspector"
    }

    private func exampleFileURL(bundleName: String, bundledExtension: String = "yaml", repoRelativePath: String) -> URL? {
        if let bundled = Bundle.main.url(forResource: bundleName, withExtension: bundledExtension) {
            return bundled
        }

        let candidates = [
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent(repoRelativePath),
            URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge")
                .appendingPathComponent(repoRelativePath)
        ]

        return candidates.first { FileManager.default.isReadableFile(atPath: $0.path) }
    }

    var body: some View {
        TabView(selection: $selectedTab) {
            IdeaListView()
                .tabItem { Label("Ideas", systemImage: "lightbulb") }
                .tag(Tab.ideas)
                .accessibilityIdentifier("tab-ideas")

            AgentCatalogView(
                catalogURL: exampleFileURL(bundleName: "agents", repoRelativePath: "examples/agents/agents.yaml")
            )
            .tabItem { Label("Agent Catalog", systemImage: "person.3") }
            .tag(Tab.agentCatalog)
            .accessibilityIdentifier("tab-agent-catalog")

            WorkflowInspectorView(
                workflowURL: exampleFileURL(bundleName: "workflow", repoRelativePath: "examples/workflows/workflow.yaml"),
                compactWorkflowURL: exampleFileURL(bundleName: "proposal-to-release", repoRelativePath: "examples/workflows/proposal-to-release.yaml"),
                catalogURL: exampleFileURL(bundleName: "agents", repoRelativePath: "examples/agents/agents.yaml")
            )
            .tabItem { Label("Workflow Inspector", systemImage: "flowchart") }
            .tag(Tab.workflowInspector)
            .accessibilityIdentifier("tab-workflow-inspector")
        }
    }
}

#Preview {
    ContentView()
        .modelContainer(
            for: [Idea.self],
            inMemory: true
        )
}
