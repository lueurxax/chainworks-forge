import SwiftUI
import SwiftData

struct ContentView: View {
    @Environment(ExecutionService.self) private var executionService
    @State private var selectedTab: Tab = .ideas
    private let forcedInitialTab: Tab?
    private let forcedUISurface: UISurface?

    enum Tab: String, CaseIterable {
        case runsHome = "Runs Home"
        case ideas = "Ideas"
        case approvals = "Approvals"
        case agentCatalog = "Agent Catalog"
        case workflowInspector = "Workflow Inspector"
        case pilotReadiness = "Pilot Readiness"
        case providerSettings = "Settings"
    }

    enum UISurface: String {
        case providerSettings = "provider_settings"
        case pilotReadiness = "pilot_readiness"
        case firstRunSetup = "first_run_setup"
    }

    init() {
        let environment = ProcessInfo.processInfo.environment
        let initialTab = environment["CHAINWORKS_UI_TEST_INITIAL_TAB"]
            .flatMap(Tab.init(rawValue:))
        forcedUISurface = environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
            .flatMap(UISurface.init(rawValue:))
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
        Group {
            if let forcedUISurface {
                directSurfaceView(for: forcedUISurface)
            } else {
                tabShell
            }
        }
    }

    private var tabShell: some View {
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

            PilotReadinessView()
                .tabItem { Label("Pilot Readiness", systemImage: "checkmark.shield") }
                .tag(Tab.pilotReadiness)
                .accessibilityIdentifier("tab-pilot-readiness")

            ProviderSettingsView()
                .tabItem { Label("Settings", systemImage: "slider.horizontal.3") }
                .tag(Tab.providerSettings)
                .accessibilityIdentifier("tab-provider-settings")
        }
        // P005-OPS §10: Foreground banner as bottom overlay — avoids conflicting with macOS tab bar
        .overlay(alignment: .bottom) {
            ForegroundBannerView(
                waitingApprovalCount: executionService.pendingApprovalCount,
                blockedCount: executionService.blockedRunCount,
                failedCount: executionService.failedRunCount,
                onTap: { selectedTab = .runsHome }
            )
            .padding(.bottom, 8)
        }
        .task(id: forcedInitialTab?.rawValue ?? "default") {
            guard let forcedInitialTab, selectedTab != forcedInitialTab else { return }
            // UI tests need a deterministic landing tab even when macOS restores prior scene state.
            selectedTab = forcedInitialTab
        }
        // Approval badge on Ideas tab when approvals are pending
        .badge(executionService.pendingApprovalCount > 0 ? executionService.pendingApprovalCount : 0)
    }

    @ViewBuilder
    private func directSurfaceView(for surface: UISurface) -> some View {
        switch surface {
        case .providerSettings:
            ProviderSettingsView()
        case .pilotReadiness:
            PilotReadinessView()
        case .firstRunSetup:
            EmptyView()
        }
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
