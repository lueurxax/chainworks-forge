import SwiftUI
import SwiftData

extension Notification.Name {
    static let chainworksSelectTab = Notification.Name("chainworks.selectTab")
    static let chainworksOpenRunInRunsHome = Notification.Name("chainworks.openRunInRunsHome")
}

struct ContentView: View {
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
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
        case ideaArchive = "idea_archive"
        case workflowMap = "workflow_map"
        case runtimeAssistant = "runtime_assistant"
        case releaseGate = "release_gate"
        case deliveryPreflightReport = "delivery_preflight_report"
        case completedExportHub = "completed_export_hub"
        case waitingApprovalRunProgress = "waiting_approval_run_progress"
        case accessibilityAudit = "accessibility_audit"
        case proposal015Proof = "proposal015_proof"
        case proposal013Proof = "proposal013_proof"
        case proposal022Proof = "proposal022_proof"
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

    private func exampleFileURL(
        configuredPath: String? = nil,
        bundleName: String,
        bundledExtension: String = "yaml",
        repoRelativePath: String
    ) -> URL? {
        let configuredURL = configuredPath
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0) }

        return AppConfiguration.preferredExampleURL(
            configuredURL: configuredURL,
            repoRelativePath: repoRelativePath,
            bundledURL: Bundle.main.url(forResource: bundleName, withExtension: bundledExtension)
        )
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
        VStack(spacing: 0) {
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
                    catalogURL: exampleFileURL(
                        configuredPath: appConfigurationStore.configuration.agentCatalogSourcePath,
                        bundleName: "agents",
                        repoRelativePath: "examples/agents/agents.yaml"
                    )
                )
                .tabItem { Label("Agent Catalog", systemImage: "person.3") }
                .tag(Tab.agentCatalog)
                .accessibilityIdentifier("tab-agent-catalog")

                WorkflowInspectorView(
                    workflowURL: exampleFileURL(
                        configuredPath: appConfigurationStore.configuration.workflowSourcePath,
                        bundleName: "workflow",
                        repoRelativePath: "examples/workflows/workflow.yaml"
                    ),
                    compactWorkflowURL: exampleFileURL(bundleName: "proposal-to-release", repoRelativePath: "examples/workflows/proposal-to-release.yaml"),
                    catalogURL: exampleFileURL(
                        configuredPath: appConfigurationStore.configuration.agentCatalogSourcePath,
                        bundleName: "agents",
                        repoRelativePath: "examples/agents/agents.yaml"
                    )
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
            .onReceive(NotificationCenter.default.publisher(for: .chainworksSelectTab)) { notification in
                guard
                    let rawValue = notification.userInfo?["tab"] as? String,
                    let tab = Tab(rawValue: rawValue)
                else { return }
                selectedTab = tab
            }
            .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenRunInRunsHome)) { _ in
                selectedTab = .runsHome
            }
            // Approval badge on Ideas tab when approvals are pending
            .badge(executionService.pendingApprovalCount > 0 ? executionService.pendingApprovalCount : 0)
        }
    }

    @ViewBuilder
    private func directSurfaceView(for surface: UISurface) -> some View {
        switch surface {
        case .providerSettings:
            ProviderSettingsView()
        case .pilotReadiness:
            PilotReadinessView()
        case .firstRunSetup:
            FirstRunSetupWizard(isPresented: .constant(true))
        case .ideaArchive:
            UITestIdeaArchiveSurface()
        case .workflowMap:
            UITestWorkflowMapSurface()
        case .runtimeAssistant:
            UITestRuntimeAssistantSurface()
        case .releaseGate:
            UITestReleaseGateSurface()
        case .deliveryPreflightReport:
            UITestDeliveryPreflightReportSurface()
        case .completedExportHub:
            UITestCompletedExportHubSurface()
        case .waitingApprovalRunProgress:
            UITestWaitingApprovalRunProgressSurface()
        case .accessibilityAudit:
            UITestAccessibilityAuditSurface()
        case .proposal015Proof:
            UITestProposal015ProofSurface()
        case .proposal013Proof:
            UITestProposal013EvidenceSurface()
        case .proposal022Proof:
            UITestProposal022EvidenceSurface()
        }
    }
}

#Preview {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return ContentView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
}

#Preview("Content Shell — Seeded") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return ContentView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .frame(width: 1280, height: 820)
}
