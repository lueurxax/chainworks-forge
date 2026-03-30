import SwiftUI
import SwiftData

extension Notification.Name {
    static let chainworksSelectTab = Notification.Name("chainworks.selectTab")
    static let chainworksOpenRunInRunsHome = Notification.Name("chainworks.openRunInRunsHome")
}

struct ContentView: View {
    @Environment(ExecutionService.self) private var executionService
    @Query(sort: \Run.startedAt, order: .reverse) private var allRuns: [Run]
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
        case gooseAssistant = "goose_assistant"
        case releaseGate = "release_gate"
        case deliveryPreflightReport = "delivery_preflight_report"
        case completedExportHub = "completed_export_hub"
        case accessibilityAudit = "accessibility_audit"
        case proposal016Proof = "proposal_016_proof"
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

        var candidates = [
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent(repoRelativePath)
        ]
        if AppConfiguration.allowsDocumentsFallbackForCurrentProcess {
            candidates.append(
                URL(fileURLWithPath: NSHomeDirectory())
                    .appendingPathComponent("Documents/Chainworks Forge")
                    .appendingPathComponent(repoRelativePath)
            )
        }

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
        VStack(spacing: 0) {
            ShellBrandHeaderView()

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

            // P005-OPS §10: render the banner as a sibling below the shell content so it
            // remains discoverable on macOS even when TabView accessibility is volatile.
            if allRuns.contains(where: { $0.status == .waitingApproval || $0.status == .blocked || $0.status == .failed }) {
                ForegroundBannerView(
                    waitingApprovalCount: allRuns.filter { $0.status == .waitingApproval }.count,
                    blockedCount: allRuns.filter { $0.status == .blocked }.count,
                    failedCount: allRuns.filter { $0.status == .failed }.count,
                    onTap: { selectedTab = .runsHome }
                )
                .padding(.vertical, 8)
            }
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
        case .gooseAssistant:
            UITestGooseAssistantSurface()
        case .releaseGate:
            UITestReleaseGateSurface()
        case .deliveryPreflightReport:
            UITestDeliveryPreflightReportSurface()
        case .completedExportHub:
            UITestCompletedExportHubSurface()
        case .accessibilityAudit:
            UITestAccessibilityAuditSurface()
        case .proposal016Proof:
            UITestProposal016ProofSurface()
        }
    }

}

private struct ShellBrandHeaderView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
            HStack(alignment: .firstTextBaseline, spacing: DesignTokens.Spacing.small) {
                ForgeIconBridge.brandHorizontalLogo()
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 150, height: 28)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Chainworks Forge")
                        .font(DesignTokens.Typography.cardTitle)
                        .accessibilityIdentifier("shell-brand-title")
                    Text("Bounded adopter shell")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("shell-brand-subtitle")
                }
                Spacer(minLength: 0)
                StatusCapsule(
                    text: "Design system",
                    color: DesignTokens.Action.primary,
                    icon: "paintbrush.fill",
                    size: .small,
                    accessibilityIdentifier: "shell-brand-pill"
                )
            }
        }
        .padding(.horizontal, DesignTokens.Spacing.section)
        .padding(.vertical, DesignTokens.Spacing.small)
        .background(DesignTokens.Action.primary.opacity(0.05))
        .overlay(Divider(), alignment: .bottom)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("shell-brand-header")
    }
}

#Preview {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)

    return ContentView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .environment(gooseServerManager)
}

#Preview("Content Shell — Seeded") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)

    return ContentView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .environment(gooseServerManager)
        .frame(width: 1280, height: 820)
}
