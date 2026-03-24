import SwiftUI
import SwiftData

struct ContentView: View {
    @Environment(ExecutionService.self) private var executionService
    @State private var selectedTab: Tab = .ideas
    private let forcedInitialTab: Tab?

    enum Tab: String, CaseIterable {
        case runsHome = "Runs Home"
        case ideas = "Ideas"
        case approvals = "Approvals"
        case agentCatalog = "Agent Catalog"
        case workflowInspector = "Workflow Inspector"
    }

    init() {
        let environment = ProcessInfo.processInfo.environment
        let initialTab = environment["CHAINWORKS_UI_TEST_INITIAL_TAB"]
            .flatMap(Tab.init(rawValue:))
        forcedInitialTab = initialTab
        // P005-OPS §5: RunsHomeView is the primary operator landing surface
        _selectedTab = State(initialValue: initialTab ?? .runsHome)
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
            // P005-OPS §5: Primary operator landing surface
            RunsHomeView()
                .tabItem { Label("Runs Home", systemImage: "house") }
                .tag(Tab.runsHome)
                .accessibilityIdentifier("tab-runs-home")

            IdeaListView()
                .tabItem { Label("Ideas", systemImage: "lightbulb") }
                .tag(Tab.ideas)
                .accessibilityIdentifier("tab-ideas")

            ApprovalInboxView()
                .tabItem {
                    Label("Approvals", systemImage: "checkmark.seal")
                }
                .tag(Tab.approvals)
                .badge(executionService.pendingApprovalCount)
                .accessibilityIdentifier("tab-approvals")

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
        .task(id: forcedInitialTab?.rawValue ?? "default") {
            guard let forcedInitialTab, selectedTab != forcedInitialTab else { return }
            // UI tests need a deterministic landing tab even when macOS restores prior scene state.
            selectedTab = forcedInitialTab
        }
        // Approval badge on Ideas tab when approvals are pending
        .badge(executionService.pendingApprovalCount > 0 ? executionService.pendingApprovalCount : 0)
    }
}

#Preview {
    ContentView()
        .modelContainer(
            for: [Idea.self],
            inMemory: true
        )
        .environment(ExecutionService(
            modelContext: try! ModelContainer(
                for: Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self]),
                configurations: [ModelConfiguration(isStoredInMemoryOnly: true)]
            ).mainContext,
            executor: SimulatedAgentExecutor()
        ))
}
