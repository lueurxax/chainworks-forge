import SwiftUI
import SwiftData
#if os(macOS)
import AppKit
#endif

private enum UIAutomationDiagnostics {
    private static let logURL: URL = {
        let path = ProcessInfo.processInfo.environment["CHAINWORKS_UI_AUTOMATION_LOG_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let path, !path.isEmpty {
            return URL(fileURLWithPath: path)
        }
        return URL(fileURLWithPath: "/tmp/chainworks-ui-automation.log")
    }()

    static func log(_ message: String) {
        guard Chainworks_ForgeApp.isUIAutomationHost else { return }

        let formatter = ISO8601DateFormatter()
        let line = "[\(formatter.string(from: Date()))] \(message)\n"
        guard let data = line.data(using: .utf8) else { return }

        if FileManager.default.fileExists(atPath: logURL.path) == false {
            try? data.write(to: logURL, options: .atomic)
            return
        }

        guard let handle = try? FileHandle(forWritingTo: logURL) else { return }
        defer { try? handle.close() }
        do {
            try handle.seekToEnd()
            try handle.write(contentsOf: data)
        } catch {
            // Ignore diagnostics failures in app bootstrap.
        }
    }
}

private struct UITestWindowSize {
    let width: CGFloat
    let height: CGFloat

    static let `default` = UITestWindowSize(width: 1200, height: 800)

    static var requested: UITestWindowSize? {
        guard let rawValue = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_WINDOW_SIZE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !rawValue.isEmpty
        else { return nil }

        let normalized = rawValue.lowercased().replacingOccurrences(of: "×", with: "x")
        let parts = normalized.split(separator: "x", maxSplits: 1).map(String.init)
        guard
            parts.count == 2,
            let width = Double(parts[0]),
            let height = Double(parts[1]),
            width > 0,
            height > 0
        else {
            return nil
        }

        return UITestWindowSize(width: width, height: height)
    }

    var accessibilityIdentifier: String {
        "ui-test-window-size-\(Int(width))x\(Int(height))"
    }
}

struct UITestAccessibilitySettings: Equatable {
    let differentiateWithoutColor: Bool
    let increaseContrast: Bool
    let reduceTransparency: Bool

    static let none = UITestAccessibilitySettings(
        differentiateWithoutColor: false,
        increaseContrast: false,
        reduceTransparency: false
    )

    static var requested: UITestAccessibilitySettings? {
        let environment = ProcessInfo.processInfo.environment
        let settings = UITestAccessibilitySettings(
            differentiateWithoutColor: environment["CHAINWORKS_UI_TEST_DIFFERENTIATE_WITHOUT_COLOR"] == "1",
            increaseContrast: environment["CHAINWORKS_UI_TEST_INCREASE_CONTRAST"] == "1",
            reduceTransparency: environment["CHAINWORKS_UI_TEST_REDUCE_TRANSPARENCY"] == "1"
        )
        return settings.hasOverrides ? settings : nil
    }

    var hasOverrides: Bool {
        differentiateWithoutColor || increaseContrast || reduceTransparency
    }

    var activeIdentifiers: [String] {
        var identifiers: [String] = []
        if differentiateWithoutColor {
            identifiers.append("ui-test-accessibility-differentiate-without-color")
        }
        if increaseContrast {
            identifiers.append("ui-test-accessibility-increase-contrast")
        }
        if reduceTransparency {
            identifiers.append("ui-test-accessibility-reduce-transparency")
        }
        return identifiers
    }
}

private struct UITestAccessibilitySettingsKey: EnvironmentKey {
    static let defaultValue: UITestAccessibilitySettings = .none
}

extension EnvironmentValues {
    var uiTestAccessibilitySettings: UITestAccessibilitySettings {
        get { self[UITestAccessibilitySettingsKey.self] }
        set { self[UITestAccessibilitySettingsKey.self] = newValue }
    }
}

@main
struct Chainworks_ForgeApp: App {
    static let processEnvironment = ProcessInfo.processInfo.environment
    static let isTestHost = processEnvironment["XCTestConfigurationFilePath"] != nil
    static let isUIAutomationHost = processEnvironment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }
    static let isUnitTestHost = isTestHost && !isUIAutomationHost
    static let initialForcedUISurface = forcedUISurface(from: processEnvironment)
    fileprivate static let uiWindowSize = UITestWindowSize.requested ?? .default
    fileprivate static let uiAccessibilitySettings = UITestAccessibilitySettings.requested
    static let sharedModelContainer: ModelContainer = {
        let environment = ProcessInfo.processInfo.environment
        let schema = Schema([
            Idea.self,
            Run.self,
            StageExecution.self,
            AgentExecution.self,
            Approval.self,
            Artifact.self,
            StewardAnalysis.self,
            StewardAnalysisRunLink.self,
            StewardRecommendation.self,
            StewardExperiment.self,
            StewardDecision.self,
            // Proposal 008: MVP Benchmark and Sign-Off
            BenchmarkCohort.self,
            BenchmarkExecutionRecord.self,
            BenchmarkPair.self,
            MVPSignOffDecisionSnapshot.self,
            // Proposal 018: Agent Session Lineage
            AgentSessionLineage.self,
            AgentSessionGeneration.self,
            AgentSessionEvent.self,
        ])
        let usesInMemoryStore = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" || isUIAutomationHost
        PersistentStoreRepair.repairDefaultStoreIfNeeded(
            isStoredInMemoryOnly: usesInMemoryStore
        )

        let modelConfiguration: ModelConfiguration
        if usesInMemoryStore {
            modelConfiguration = ModelConfiguration(
                schema: schema,
                isStoredInMemoryOnly: true
            )
        } else {
            modelConfiguration = ModelConfiguration(
                "Chainworks Forge",
                schema: schema,
                url: PersistentStoreRepair.canonicalStoreURL()
            )
        }

        do {
            return try ModelContainer(for: schema, configurations: [modelConfiguration])
        } catch {
            fatalError("Could not create ModelContainer: \(error)")
        }
    }()

    @NSApplicationDelegateAdaptor(AutomationFallbackAppDelegate.self) private var automationFallbackAppDelegate

    /// Disable macOS window/scene restoration when running under UI tests.
    /// Without this, `WindowGroup` restores the previous session's window,
    /// creating two overlapping windows that cause XCUITest element queries
    /// to find (and click) elements hidden behind the wrong window — leading
    /// to indefinite hangs.
    init() {
        if Self.isUIAutomationHost || ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            UserDefaults.standard.set(false, forKey: "NSQuitAlwaysKeepsWindows")
            UserDefaults.standard.set(true, forKey: "ApplePersistenceIgnoreState")
            Self.clearSavedWindowState()
        }
        if Self.isUIAutomationHost {
            ProcessInfo.processInfo.disableAutomaticTermination("Chainworks Forge UI automation session")
        }
        UIAutomationDiagnostics.log(
            "app.init uiAutomation=\(Self.isUIAutomationHost) unitTest=\(Self.isUnitTestHost) " +
            "directSurface=\(Self.processEnvironment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] ?? "nil")"
        )
    }

    var body: some Scene {
        Window("Chainworks Forge", id: "main-window") {
            RootHostView(forcedUISurface: Self.initialForcedUISurface)
                .modifier(OptionalModelContainerModifier(enabled: Self.requiresSharedModelContainer(for: Self.initialForcedUISurface)))
        }
        .defaultSize(width: Self.uiWindowSize.width, height: Self.uiWindowSize.height)
    }

}

extension Chainworks_ForgeApp {
    static func forcedUISurface(from environment: [String: String]) -> ContentView.UISurface? {
        environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
            .flatMap(ContentView.UISurface.init(rawValue:))
    }

    static func usesStandaloneUISurface(_ surface: ContentView.UISurface?) -> Bool {
        surface == .proposal015Proof
    }

    static func requiresSharedModelContainer(for surface: ContentView.UISurface?) -> Bool {
        !usesStandaloneUISurface(surface)
    }

    static func shouldCreateFallbackWindow(for surface: ContentView.UISurface?) -> Bool {
        !usesStandaloneUISurface(surface)
    }

    fileprivate static func clearSavedWindowState() {
        guard let bundleIdentifier = Bundle.main.bundleIdentifier else { return }
        let savedStateURL = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
            .appendingPathComponent("Library/Saved Application State", isDirectory: true)
            .appendingPathComponent("\(bundleIdentifier).savedState", isDirectory: true)
        if FileManager.default.fileExists(atPath: savedStateURL.path) {
            try? FileManager.default.removeItem(at: savedStateURL)
        }
    }
}

final class AutomationFallbackAppDelegate: AppTerminationCoordinator {
    private var fallbackWindow: NSWindow?

    private func hasVisibleWindow() -> Bool {
        NSApp.windows.contains { window in
            window.isVisible && !window.isMiniaturized
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard Chainworks_ForgeApp.isUIAutomationHost else { return }
        let forcedUISurface = Chainworks_ForgeApp.forcedUISurface(from: Chainworks_ForgeApp.processEnvironment)
        let directSurfaceRequested = forcedUISurface != nil
        UIAutomationDiagnostics.log(
            "applicationDidFinishLaunching windows=\(NSApp.windows.count) visible=\(hasVisibleWindow())"
        )

        if directSurfaceRequested {
            UIAutomationDiagnostics.log("directSurfaceRequested fallbackWindowWillBeCreatedOnlyIfNoNativeWindowAppears")
        }

        if !Chainworks_ForgeApp.shouldCreateFallbackWindow(for: forcedUISurface) {
            UIAutomationDiagnostics.log("fallbackWindowSuppressed directSurface=\(forcedUISurface?.rawValue ?? "nil")")
            return
        }

        Task { @MainActor in
            let retryDelays: [Duration] = directSurfaceRequested
                ? [.zero, .milliseconds(300), .milliseconds(900)]
                : [.zero, .milliseconds(100)]

            for (attempt, delay) in retryDelays.enumerated() {
                if delay != .zero {
                    try? await Task.sleep(for: delay)
                }

                if hasVisibleWindow() {
                    UIAutomationDiagnostics.log(
                        "nativeWindowDetected attempt=\(attempt) count=\(NSApp.windows.count) visible=\(hasVisibleWindow())"
                    )
                    return
                }
            }

            UIAutomationDiagnostics.log(
                "creatingFallbackWindow count=\(NSApp.windows.count) visible=\(hasVisibleWindow())"
            )

            let hostingController = NSHostingController(
                rootView: fallbackRootView(for: forcedUISurface)
            )
            let window = NSWindow(contentViewController: hostingController)
            window.title = "Chainworks Forge"
            window.identifier = NSUserInterfaceItemIdentifier("chainworks-fallback-window")
            window.setContentSize(NSSize(width: Chainworks_ForgeApp.uiWindowSize.width, height: Chainworks_ForgeApp.uiWindowSize.height))
            window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
            window.center()
            window.makeKeyAndOrderFront(nil)
            window.orderFrontRegardless()

            NSApp.setActivationPolicy(.regular)
            if #available(macOS 14.0, *) {
                NSApp.activate()
            } else {
                NSRunningApplication.current.activate(options: [.activateIgnoringOtherApps])
                NSApp.activate(ignoringOtherApps: true)
            }

            fallbackWindow = window
            UIAutomationDiagnostics.log("fallbackWindowCreated windows=\(NSApp.windows.count) isVisible=\(window.isVisible)")
        }
    }

    @MainActor
    private func fallbackRootView(for forcedUISurface: ContentView.UISurface?) -> some View {
        RootHostView(forcedUISurface: forcedUISurface)
            .modifier(OptionalModelContainerModifier(enabled: Chainworks_ForgeApp.requiresSharedModelContainer(for: forcedUISurface)))
    }
}

private struct UnitTestHostView: View {
    var body: some View {
        Color.clear
            .accessibilityIdentifier("unit-test-host")
    }
}

private struct OptionalModelContainerModifier: ViewModifier {
    let enabled: Bool

    @ViewBuilder
    func body(content: Content) -> some View {
        if enabled {
            content.modelContainer(Chainworks_ForgeApp.sharedModelContainer)
        } else {
            content
        }
    }
}

private struct RootHostView: View {
    let forcedUISurface: ContentView.UISurface?

    var body: some View {
        Group {
            if Chainworks_ForgeApp.isUnitTestHost {
                UnitTestHostView()
            } else if Chainworks_ForgeApp.usesStandaloneUISurface(forcedUISurface) {
                StandaloneProposal015HostView()
            } else {
                AppBootstrapView()
            }
        }
        .task {
            guard Chainworks_ForgeApp.isUIAutomationHost else { return }
            guard !Chainworks_ForgeApp.usesStandaloneUISurface(forcedUISurface) else { return }
            #if os(macOS)
            UIAutomationDiagnostics.log("rootHost.task.begin windows=\(NSApp.windows.count)")
            NSApp.setActivationPolicy(.regular)
            for attempt in 0..<3 {
                if attempt > 0 {
                    try? await Task.sleep(for: .milliseconds(200))
                }

                let shouldContinue = await MainActor.run { () -> Bool in
                    if #available(macOS 14.0, *) {
                        NSApp.activate()
                    } else {
                        NSRunningApplication.current.activate(options: [.activateIgnoringOtherApps])
                        NSApp.activate(ignoringOtherApps: true)
                    }

                    var shouldRetry = true
                    for window in NSApp.windows {
                        window.collectionBehavior.remove(.transient)
                        window.makeKeyAndOrderFront(nil)
                    }

                    if NSApp.windows.contains(where: { $0.isVisible }) {
                        shouldRetry = false
                    }

                    return shouldRetry
                }
                UIAutomationDiagnostics.log("rootHost.task.activation attempt=\(attempt) windows=\(NSApp.windows.count)")
                if !shouldContinue {
                    break
                }
            }
            UIAutomationDiagnostics.log("rootHost.task.end windows=\(NSApp.windows.count)")
            #endif
        }
    }
}

private struct StandaloneProposal015HostView: View {
    var body: some View {
        withRequestedWindowSizeMarker {
            VStack(spacing: 0) {
                Text("UI Test Surface: proposal015_proof")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .accessibilityIdentifier("ui-test-direct-surface-ready-proposal015_proof")

                if let requestedWindowSize = UITestWindowSize.requested {
                    Text("Window \(Int(requestedWindowSize.width))×\(Int(requestedWindowSize.height))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.bottom, 4)
                        .accessibilityIdentifier(requestedWindowSize.accessibilityIdentifier)
                }

                UITestProposal015ProofSurface()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 960, minHeight: 720)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("ui-test-direct-surface-container-proposal015_proof")
    }

    @ViewBuilder
    private func withRequestedWindowSizeMarker<Content: View>(
        @ViewBuilder _ content: () -> Content
    ) -> some View {
        content()
            .environment(\.uiTestAccessibilitySettings, Chainworks_ForgeApp.uiAccessibilitySettings ?? .none)
            .overlay(alignment: .topLeading) {
                VStack(alignment: .leading, spacing: 1) {
                    if let requestedWindowSize = UITestWindowSize.requested {
                        Color.clear
                            .frame(width: 1, height: 1)
                            .accessibilityIdentifier(requestedWindowSize.accessibilityIdentifier)
                    }

                    if let requestedSettings = Chainworks_ForgeApp.uiAccessibilitySettings {
                        ForEach(requestedSettings.activeIdentifiers, id: \.self) { identifier in
                            Color.clear
                                .frame(width: 1, height: 1)
                                .accessibilityIdentifier(identifier)
                        }
                    }
                }
            }
    }
}

// MARK: - Menu Bar Bootstrap (P005-OPS §10)

struct AppBootstrapMenuBarView: View {
    @Environment(\.modelContext) private var modelContext
    @State private var executionService: ExecutionService?

    var body: some View {
        if let service = executionService {
            MenuBarStatusView()
                .environment(service)
        } else {
            Text("Loading...")
                .task {
                    let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: nil)
                    executionService = ExecutionService(
                        modelContext: modelContext,
                        executor: executor
                    )
                }
        }
    }
}

// MARK: - AppBootstrapView (ARCH-022: app-scoped ExecutionService wiring)

struct AppBootstrapView: View {
    @Environment(\.modelContext) private var modelContext
    @State private var executionService: ExecutionService?
    @State private var appConfigurationStore: AppConfigurationStore?
    @State private var providerSettingsStore: ProviderSettingsStore?
    @State private var providerRegistry: ProviderRegistry?

    @State private var showFirstRunWizard = false
    @State private var dogfoodHarnessStarted = false
    @State private var proposal015AppProofStarted = false
    @State private var proposal022AppProofStarted = false
    private let forcedUISurface = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
        .flatMap(ContentView.UISurface.init(rawValue:))

    var body: some View {
        if forcedUISurface == .proposal015Proof {
            proposal015ProofRoot()
        } else if let service = executionService,
           let appConfigurationStore,
           let providerSettingsStore,
           let providerRegistry {
            bootstrappedRoot(
                service: service,
                appConfigurationStore: appConfigurationStore,
                providerSettingsStore: providerSettingsStore,
                providerRegistry: providerRegistry
            )
        } else {
            ProgressView("Starting engine...")
                .accessibilityIdentifier("bootstrap-loading")
                .task {
                    await bootstrapService()
                }
        }
    }

    @MainActor
    private func bootstrapService() async {
        guard executionService == nil else { return }
        UIAutomationDiagnostics.log("bootstrapService.begin")

        let environment = ProcessInfo.processInfo.environment
        let isTestHost = environment["XCTestConfigurationFilePath"] != nil
        let isUIAutomationHost = environment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }
        let isUnitTestHost = isTestHost && !isUIAutomationHost
        let forceLiveRuntimeUnavailable = environment["CHAINWORKS_UI_TEST_FORCE_LIVE_RUNTIME_UNAVAILABLE"] == "1"
        let hasDirectUISurface = !(environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .isEmpty ?? true)
        var bootstrapEnvironment = environment
        if isUIAutomationHost &&
            (hasDirectUISurface ||
             environment["CHAINWORKS_WORKFLOW_SOURCE_PATH"] != nil ||
             environment["CHAINWORKS_AGENT_CATALOG_SOURCE_PATH"] != nil) {
            // UI proof lanes must honor the synced-tree fixtures even when the host machine
            // has a persisted app configuration from unrelated local usage.
            bootstrapEnvironment["CHAINWORKS_ALLOW_ENV_OVERRIDE"] = "1"
        }
        let suppressProposalAutoruns = isUIAutomationHost || hasDirectUISurface
        let disableEagerUITestBootstrap = isUIAutomationHost &&
            (environment["CHAINWORKS_UI_TEST_DISABLE_EAGER_BOOTSTRAP"] == "1" || hasDirectUISurface)
        let skipBackgroundBootstrap = disableEagerUITestBootstrap || (isUIAutomationHost && hasDirectUISurface)
        let isProposal007DogfoodHarness = Proposal007DogfoodHarness.isEnabled && !suppressProposalAutoruns
        let isProposal015AppProofAutorun = Proposal015AppProofAutorun.isEnabled && !suppressProposalAutoruns
        let isProposal022AppProofAutorun = Proposal022AppProofAutorun.isEnabled && !suppressProposalAutoruns

        let appConfigurationStore = AppConfigurationStore()
        let resolvedConfiguration = BootstrapConfigurationResolver.resolve(
            store: appConfigurationStore,
            environment: bootstrapEnvironment
        )
        let providerSettingsStore = ProviderSettingsStore()
        let providerRegistry = ProviderRegistry(settingsStore: providerSettingsStore)
        self.appConfigurationStore = appConfigurationStore
        self.providerSettingsStore = providerSettingsStore
        self.providerRegistry = providerRegistry
        UIAutomationDiagnostics.log(
            "bootstrapService.config directSurface=\(environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] ?? "nil") " +
            "inMemory=\(environment["CHAINWORKS_IN_MEMORY_STORE"] ?? "nil")"
        )

        let catalog = Self.loadBundledCatalog(appConfiguration: resolvedConfiguration)
        let stewardConfig = Self.loadStewardConfig()
        let liveRuntimeConfiguration = isUnitTestHost || forceLiveRuntimeUnavailable
            ? nil
            : Self.loadLiveRuntimeConfiguration()
        // The simulated executor remains the safe default, but Proposal 004 live runs
        // are resolved per-plan inside ExecutionService using `liveRuntimeConfiguration`.
        let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: catalog)
        let service = ExecutionService(
            modelContext: modelContext,
            executor: executor,
            catalog: catalog,
            stewardConfig: stewardConfig,
            liveRuntimeConfiguration: liveRuntimeConfiguration,
            providerRegistry: providerRegistry
        )
        executionService = service
        #if os(macOS)
        (NSApp.delegate as? AppTerminationCoordinator)?.executionTerminationController = service
        #endif

        Self.seedIdeaIfRequested(modelContext: modelContext)
        Self.seedWaitingApprovalRunIfRequested(modelContext: modelContext, catalog: catalog)
        Self.seedWorkflowMapRunIfRequested(modelContext: modelContext)
        Self.seedReleaseGateRunIfRequested(modelContext: modelContext)
        service.rebuildPersistedPendingApprovals()

        if !isUnitTestHost &&
            !isUIAutomationHost &&
            !isProposal007DogfoodHarness &&
            !isProposal015AppProofAutorun &&
            !isProposal022AppProofAutorun {
            do {
                let normalizedCount = try ResumeManager(modelContext: modelContext)
                    .normalizeInterruptedRunsForManualResume()
                if normalizedCount > 0 {
                    ForgeLogger.app.info("Normalized \(normalizedCount) interrupted runs for manual resume after app launch")
                }
            } catch {
                ForgeLogger.app.error("Failed to normalize interrupted runs at startup: \(error.localizedDescription)")
            }
            service.rebuildPersistedPendingApprovals()
        }

        if !isUnitTestHost &&
            !isProposal015AppProofAutorun &&
            !isProposal022AppProofAutorun &&
            !skipBackgroundBootstrap {
            // Proposal 003 — REQ-008: Check if config has changed since last analysis.
            service.checkForConfigChange()
        }

        if !isUnitTestHost &&
            !disableEagerUITestBootstrap &&
            !isProposal015AppProofAutorun &&
            !isProposal022AppProofAutorun {
            Task { @MainActor in
                await providerRegistry.refreshHealth()
            }
        }

        if !isUIAutomationHost &&
            !isProposal015AppProofAutorun &&
            !isProposal022AppProofAutorun &&
            shouldPresentFirstRunWizard(
            configuration: resolvedConfiguration,
            providerSettings: providerSettingsStore.settings
        ) {
            showFirstRunWizard = true
        }

        if isProposal007DogfoodHarness,
           dogfoodHarnessStarted == false {
            dogfoodHarnessStarted = true
            Task { @MainActor in
                let harness = Proposal007DogfoodHarness(
                    modelContext: modelContext,
                    executionService: service,
                    appConfiguration: resolvedConfiguration,
                    providerRegistry: providerRegistry
                )

                do {
                    let result = try await harness.runFromEnvironment()
                    ForgeLogger.app.info("Proposal 007 dogfood harness completed: \(result.exportPath)")
                } catch {
                    ForgeLogger.app.error("Proposal 007 dogfood harness failed: \(error.localizedDescription)")
                }

                #if os(macOS)
                NSApp.terminate(nil)
                #endif
            }
        }

        if isProposal015AppProofAutorun,
           proposal015AppProofStarted == false {
            proposal015AppProofStarted = true
            Task { @MainActor in
                let autorun = Proposal015AppProofAutorun()

                do {
                    let export = try autorun.runFromEnvironment()
                    ForgeLogger.app.info("Proposal 015 app proof completed: \(export.result.proofStatus)")
                } catch {
                    ForgeLogger.app.error("Proposal 015 app proof failed: \(error.localizedDescription)")
                }

                #if os(macOS)
                NSApp.terminate(nil)
                #endif
            }
        }

        if isProposal022AppProofAutorun,
           proposal022AppProofStarted == false {
            proposal022AppProofStarted = true
            Task { @MainActor in
                let autorun = Proposal022AppProofAutorun(
                    modelContext: modelContext,
                    executionService: service
                )

                do {
                    let export = try await autorun.runFromEnvironment()
                    ForgeLogger.app.info("Proposal 022 app proof completed: \(export.result.proofStatus)")
                } catch {
                    ForgeLogger.app.error("Proposal 022 app proof failed: \(error.localizedDescription)")
                }

                #if os(macOS)
                NSApp.terminate(nil)
                #endif
            }
        }
        UIAutomationDiagnostics.log("bootstrapService.end showFirstRunWizard=\(showFirstRunWizard)")
    }

    @ViewBuilder
    private func bootstrappedRoot(
        service: ExecutionService,
        appConfigurationStore: AppConfigurationStore,
        providerSettingsStore: ProviderSettingsStore,
        providerRegistry: ProviderRegistry
    ) -> some View {
        if let forcedUISurface {
            withRequestedWindowSizeMarker {
                VStack(spacing: 0) {
                    Text("UI Test Surface: \(forcedUISurface.rawValue)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 6)
                        .accessibilityIdentifier("ui-test-direct-surface-ready-\(forcedUISurface.rawValue)")

                    if let requestedWindowSize = UITestWindowSize.requested {
                        Text("Window \(Int(requestedWindowSize.width))×\(Int(requestedWindowSize.height))")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 8)
                            .padding(.bottom, 4)
                            .accessibilityIdentifier(requestedWindowSize.accessibilityIdentifier)
                    }

                    Group {
                        switch forcedUISurface {
                        case .providerSettings:
                            ProviderSettingsView()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .pilotReadiness:
                            PilotReadinessView()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .firstRunSetup:
                            FirstRunSetupWizard(isPresented: .constant(true))
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .ideaArchive:
                            UITestIdeaArchiveSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .workflowMap:
                            UITestWorkflowMapSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .runtimeAssistant:
                            UITestRuntimeAssistantSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .releaseGate:
                            UITestReleaseGateSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .deliveryPreflightReport:
                            UITestDeliveryPreflightReportSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .completedExportHub:
                            UITestCompletedExportHubSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .waitingApprovalRunProgress:
                            UITestWaitingApprovalRunProgressSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .accessibilityAudit:
                            UITestAccessibilityAuditSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .proposal015Proof:
                            UITestProposal015ProofSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .proposal013Proof:
                            UITestProposal013EvidenceSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        case .proposal022Proof:
                            UITestProposal022EvidenceSurface()
                                .environment(service)
                                .environment(appConfigurationStore)
                                .environment(providerSettingsStore)
                                .environment(providerRegistry)

                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(minWidth: 960, minHeight: 720)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("ui-test-direct-surface-container-\(forcedUISurface.rawValue)")
        } else {
            withRequestedWindowSizeMarker {
                ContentView()
                    .environment(service)
                    .environment(appConfigurationStore)
                    .environment(providerSettingsStore)
                    .environment(providerRegistry)
                    .sheet(isPresented: $showFirstRunWizard) {
                        FirstRunSetupWizard(isPresented: $showFirstRunWizard)
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                    }
            }
        }
    }

    @ViewBuilder
    private func proposal015ProofRoot() -> some View {
        // Proposal 015's proof surface is fully fixture-backed and does not depend on
        // runtime/provider bootstrap. Keep this path minimal so approved-host UI proof
        // does not fail behind unrelated provider initialization.
        withRequestedWindowSizeMarker {
            VStack(spacing: 0) {
                Text("UI Test Surface: proposal015_proof")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .accessibilityIdentifier("ui-test-direct-surface-ready-proposal015_proof")

                if let requestedWindowSize = UITestWindowSize.requested {
                    Text("Window \(Int(requestedWindowSize.width))×\(Int(requestedWindowSize.height))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.bottom, 4)
                        .accessibilityIdentifier(requestedWindowSize.accessibilityIdentifier)
                }

                UITestProposal015ProofSurface()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 960, minHeight: 720)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("ui-test-direct-surface-container-proposal015_proof")
    }

    @ViewBuilder
    private func withRequestedWindowSizeMarker<Content: View>(
        @ViewBuilder _ content: () -> Content
    ) -> some View {
        content()
            .environment(\.uiTestAccessibilitySettings, Chainworks_ForgeApp.uiAccessibilitySettings ?? .none)
            .overlay(alignment: .topLeading) {
                VStack(alignment: .leading, spacing: 1) {
                    if let requestedWindowSize = UITestWindowSize.requested {
                        Color.clear
                            .frame(width: 1, height: 1)
                            .accessibilityIdentifier(requestedWindowSize.accessibilityIdentifier)
                    }

                    if let requestedSettings = Chainworks_ForgeApp.uiAccessibilitySettings {
                        ForEach(requestedSettings.activeIdentifiers, id: \.self) { identifier in
                            Color.clear
                                .frame(width: 1, height: 1)
                                .accessibilityIdentifier(identifier)
                        }
                    }
                }
            }
    }

    private static func loadBundledCatalog(appConfiguration: AppConfiguration) -> AgentCatalog? {
        let candidates: [URL?] = [
            AppConfiguration.preferredExampleURL(
                configuredURL: URL(fileURLWithPath: appConfiguration.agentCatalogSourcePath),
                repoRelativePath: "examples/agents/agents.yaml",
                bundledURL: Bundle.main.url(forResource: "agents", withExtension: "yaml")
            ),
            Bundle.main.url(forResource: "agents", withExtension: "yaml")
        ]
        for case let url? in candidates {
            if let catalog = try? YAMLParser.loadAgentCatalog(from: url) {
                return catalog
            }
        }
        return nil
    }

    private func shouldPresentFirstRunWizard(
        configuration: AppConfiguration,
        providerSettings: ProviderSettings
    ) -> Bool {
        let environment = ProcessInfo.processInfo.environment
        if environment["CHAINWORKS_UI_TEST_INITIAL_TAB"] != nil
            || environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
            || Proposal007DogfoodHarness.isEnabled
            || Proposal015AppProofAutorun.isEnabled
            || Proposal022AppProofAutorun.isEnabled {
            return false
        }

        if !SecurityScopedAccess.fileExists(at: URL(fileURLWithPath: configuration.workflowSourcePath)) {
            return true
        }

        if !SecurityScopedAccess.fileExists(at: URL(fileURLWithPath: configuration.agentCatalogSourcePath)) {
            return true
        }

        return providerSettings.configuredProviders.isEmpty
    }

    private static func loadBundledWorkflow(named resourceName: String, repoRelativePath: String) -> WorkflowDefinition? {
        if let url = AppConfiguration.preferredExampleURL(
            repoRelativePath: repoRelativePath,
            bundledURL: Bundle.main.url(forResource: resourceName, withExtension: "yaml")
        ), let workflow = try? YAMLParser.loadWorkflow(from: url) {
            return workflow
        }
        return nil
    }

    private static func loadStewardConfig() -> StewardConfig? {
        if let url = AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/steward/steward_config.yaml",
            bundledURL: Bundle.main.url(forResource: "steward_config", withExtension: "yaml")
        ), let config = try? YAMLParser.loadStewardConfig(from: url) {
            // REQ-003: Enforce validation at load time.
            let issues = YAMLValidator.validateStewardConfig(config)
            let errors = issues.filter { $0.severity == .error }
            if !errors.isEmpty {
                ForgeLogger.steward.error("steward_config.yaml validation failed: \(errors.map(\.message).joined(separator: "; ")). Using defaults.")
                return StewardConfig.defaultConfig
            }
            return config
        }
        return nil
    }

    private static func loadLiveRuntimeConfiguration() -> LiveRuntimeConfiguration? {
        let environment = ProcessInfo.processInfo.environment
        guard let fixtureMode = environment["CHAINWORKS_FIXTURE_MODE"],
              !fixtureMode.isEmpty else { return nil }

        let override = LiveExecutionOverride(
            enabled: true,
            provider: environment["CHAINWORKS_LIVE_PROVIDER"] ?? "claude_code",
            model: environment["CHAINWORKS_LIVE_MODEL"] ?? "fixture-model",
            effort: environment["CHAINWORKS_LIVE_EFFORT"] ?? "high"
        )

        switch fixtureMode {
        case "proposal_loop_success":
            return LiveRuntimeConfiguration(
                override: override,
                transportMode: .fixtureProposalLoopSuccess
            )
        case "proposal022_feedback_cycle":
            return LiveRuntimeConfiguration(
                override: override,
                transportMode: .fixtureProposal022FeedbackCycle
            )
        case "proposal013_aggregate_failure":
            return LiveRuntimeConfiguration(
                override: override,
                transportMode: .fixtureProposal013AggregateFailure
            )
        case "full_mvp_success":
            return LiveRuntimeConfiguration(
                override: override,
                transportMode: .fixtureFullMVPSuccess
            )
        default:
            return nil
        }
    }

    @MainActor
    private static func seedIdeaIfRequested(modelContext: ModelContext) {
        let environment = ProcessInfo.processInfo.environment
        guard let title = environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"],
              !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return
        }

        let seededWorkspaceRoot = environment["CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)

        let descriptor = FetchDescriptor<Idea>()
        let existingIdeas = (try? modelContext.fetch(descriptor)) ?? []
        if let existingIdea = existingIdeas.first(where: { $0.title == title }) {
            if let seededWorkspaceRoot, !seededWorkspaceRoot.isEmpty {
                existingIdea.workspaceRootPath = seededWorkspaceRoot
                do {
                    try modelContext.save()
                } catch {
                    ForgeLogger.app.error("Failed to persist seeded idea workspace root: \(error.localizedDescription)")
                    UIAutomationDiagnostics.log("Failed to persist seeded idea workspace root: \(error.localizedDescription)")
                }
            }
            return
        }

        let idea = Idea(
            title: title,
            body: environment["CHAINWORKS_UI_TEST_SEED_IDEA_BODY"] ?? "Seeded UI test idea",
            attachmentPath: nil,
            workspaceRootPath: seededWorkspaceRoot?.isEmpty == false ? seededWorkspaceRoot : nil
        )
        modelContext.insert(idea)
        do {
            try modelContext.save()
        } catch {
            ForgeLogger.app.error("Failed to persist seeded UI test idea '\(title)': \(error.localizedDescription)")
            UIAutomationDiagnostics.log("Failed to persist seeded UI test idea '\(title)': \(error.localizedDescription)")
        }
    }

    @MainActor
    private static func seedWaitingApprovalRunIfRequested(
        modelContext: ModelContext,
        catalog: AgentCatalog?
    ) {
        // RunRepository-exempt: seeded UI-test host data with manual stage/artifact shaping.
        let environment = ProcessInfo.processInfo.environment
        guard environment["CHAINWORKS_UI_TEST_SEED_WAITING_APPROVAL_RUN"] == "1",
              environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != "workflow_map",
              let catalog,
              let workflow = loadBundledWorkflow(
                named: "proposal-loop-live",
                repoRelativePath: "examples/workflows/proposal-loop-live.yaml"
              ) else {
            return
        }

        let title = environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"] ?? "Seeded Waiting Approval Run"
        let body = environment["CHAINWORKS_UI_TEST_SEED_IDEA_BODY"] ?? "Seeded UI test idea"

        let ideaDescriptor = FetchDescriptor<Idea>()
        let existingIdeas = (try? modelContext.fetch(ideaDescriptor)) ?? []
        let idea = existingIdeas.first(where: { $0.title == title }) ?? {
            let newIdea = Idea(title: title, body: body, attachmentPath: nil)
            modelContext.insert(newIdea)
            return newIdea
        }()

        if idea.runs.contains(where: { $0.workflowID == "proposal_loop_live" }) {
            do {
                try modelContext.save()
            } catch {
                ForgeLogger.app.error(
                    "Failed to persist seeded waiting-approval idea state for '\(title)': \(error.localizedDescription)"
                )
                UIAutomationDiagnostics.log(
                    "Failed to persist seeded waiting-approval idea state for '\(title)': \(error.localizedDescription)"
                )
            }
            return
        }

        do {
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let plan = try compiler.previewCompile(
                workflow: workflow,
                catalog: catalog,
                catalogSourcePath: resolvedExamplePath("examples/agents/agents.yaml")
            )
            let workspace = try makeSeedWorkspace(runID: UUID(), prefix: "UITestWaitingApproval")
            let run = try RunRepository(context: modelContext).createRunFromPlan(
                for: idea,
                plan: plan,
                workspace: workspace,
                workflowSourcePath: resolvedExamplePath("examples/workflows/proposal-loop-live.yaml"),
                catalogSourcePath: resolvedExamplePath("examples/agents/agents.yaml")
            )

            run.status = .waitingApproval

            let refinedStage = StageExecution(
                stageID: "state_4_proposal_refined",
                label: "Proposal refined",
                startedAt: Date().addingTimeInterval(-120),
                status: .completed,
                iteration: 1,
                attemptNumber: 1
            )
            refinedStage.completedAt = Date().addingTimeInterval(-90)
            refinedStage.run = run
            modelContext.insert(refinedStage)

            let approvalStage = StageExecution(
                stageID: "state_5_proposal_approval",
                label: "Human approval: proposal quality",
                startedAt: Date().addingTimeInterval(-60),
                status: .waitingApproval,
                iteration: 1,
                attemptNumber: 1
            )
            approvalStage.run = run
            modelContext.insert(approvalStage)

            let writerAgent = ResolvedAgent(
                id: "proposal_writer",
                title: "Proposal Writer",
                mode: "writer",
                provider: "claude_code",
                model: "fixture-model",
                effort: "high",
                maxTurns: 12,
                temperature: 0.1,
                permissionProfile: "SAFE_READONLY",
                skillRef: "proposal_writer_core",
                skillRole: nil,
                prompt: "Seeded waiting-approval proposal output",
                outputContract: nil,
                requiresHumanApproval: false,
                inputs: [],
                outputs: ["proposal_current", "proposal_revision_summary"]
            )

            let orchestratorAgent = ResolvedAgent(
                id: "lead_orchestrator",
                title: "Lead Orchestrator",
                mode: "orchestrator",
                provider: "claude_code",
                model: "fixture-model",
                effort: "high",
                maxTurns: 12,
                temperature: 0.1,
                permissionProfile: "SAFE_READONLY",
                skillRef: "orchestrator_core",
                skillRole: nil,
                prompt: "Seeded review summary output",
                outputContract: nil,
                requiresHumanApproval: false,
                inputs: [],
                outputs: ["proposal_review_summary"]
            )

            let writerExecution = AgentExecution(
                agentID: writerAgent.id,
                agentTitle: writerAgent.title,
                taskName: "seed_proposal_artifacts",
                startedAt: Date().addingTimeInterval(-120),
                status: .completed,
                provider: writerAgent.provider,
                effort: writerAgent.effort
            )
            writerExecution.completedAt = Date().addingTimeInterval(-95)
            writerExecution.stageExecution = refinedStage
            writerExecution.providerSessionID = "fixture-seeded-session"
            writerExecution.runtimeSessionID = "fixture-seeded-session"
            writerExecution.transcriptArtifactPath = workspace.artifactRoot
                .appendingPathComponent("seed")
                .appendingPathComponent("proposal_writer_transcript.md")
                .path
            modelContext.insert(writerExecution)

            let reviewExecution = AgentExecution(
                agentID: orchestratorAgent.id,
                agentTitle: orchestratorAgent.title,
                taskName: "seed_review_summary",
                startedAt: Date().addingTimeInterval(-100),
                status: .completed,
                provider: orchestratorAgent.provider,
                effort: orchestratorAgent.effort
            )
            reviewExecution.completedAt = Date().addingTimeInterval(-90)
            reviewExecution.stageExecution = refinedStage
            reviewExecution.providerSessionID = "fixture-seeded-session"
            reviewExecution.runtimeSessionID = "fixture-seeded-session"
            modelContext.insert(reviewExecution)

            let artifactManager = ArtifactManager(modelContext: modelContext)
            let writerArtifacts = try artifactManager.persistOutputs(
                outputs: [
                    "proposal_current": Data("""
                    # Seeded Proposal

                    This run is paused at approval and is safe to resume.
                    """.utf8),
                    "proposal_revision_summary": Data("""
                    # Revision Summary

                    Review feedback has been incorporated.
                    """.utf8),
                    "proposal_writer_receipt.json": Data("""
                    {"status":"success","agent_id":"proposal_writer"}
                    """.utf8),
                    "proposal_writer_transcript.md": Data("""
                    # Transcript

                    Seeded transcript for receipt inspection.
                    """.utf8)
                ],
                agent: writerAgent,
                agentExecution: writerExecution,
                workspace: workspace,
                stageID: refinedStage.stageID,
                iteration: refinedStage.iteration,
                attemptNumber: refinedStage.attemptNumber,
                catalog: catalog
            )
            let reviewArtifacts = try artifactManager.persistOutputs(
                outputs: [
                    "proposal_review_summary": Data("""
                    {
                      "pass": true,
                      "average_score": 9.25,
                      "aggregate_score": 9.25,
                      "min_individual_score": 9.1,
                      "blocker_count": 0,
                      "summary": "Seeded approval-ready summary.",
                      "required_changes": [],
                      "recurring_themes": ["Scope is clear"],
                      "decision": "proceed"
                    }
                    """.utf8)
                ],
                agent: orchestratorAgent,
                agentExecution: reviewExecution,
                workspace: workspace,
                stageID: refinedStage.stageID,
                iteration: refinedStage.iteration,
                attemptNumber: refinedStage.attemptNumber,
                catalog: catalog
            )

            writerExecution.artifacts = writerArtifacts
            reviewExecution.artifacts = reviewArtifacts
            refinedStage.agentExecutions = [writerExecution, reviewExecution]
            if run.stageExecutions.contains(where: { $0.id == refinedStage.id }) == false {
                run.stageExecutions.append(refinedStage)
            }
            if run.stageExecutions.contains(where: { $0.id == approvalStage.id }) == false {
                run.stageExecutions.append(approvalStage)
            }

            let approval = Approval(
                stageID: approvalStage.stageID,
                requestedAt: Date().addingTimeInterval(-45),
                decision: .requested
            )
            approval.run = run
            modelContext.insert(approval)

            try modelContext.save()
        } catch {
            ForgeLogger.test.error("Failed to seed waiting approval run: \(error.localizedDescription)")
        }
    }

    @MainActor
    private static func seedWorkflowMapRunIfRequested(modelContext: ModelContext) {
        let environment = ProcessInfo.processInfo.environment
        guard environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] == "workflow_map" else {
            return
        }
        guard environment["CHAINWORKS_UI_TEST_DISABLE_WORKFLOW_MAP_SEED"] != "1" else {
            return
        }

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let existingRuns = (try? modelContext.fetch(descriptor)) ?? []
        if !existingRuns.isEmpty {
            return
        }

        PreviewSupport.seedWorkflowMapPreviewData(context: modelContext)
    }

    @MainActor
    private static func seedReleaseGateRunIfRequested(modelContext: ModelContext) {
        let environment = ProcessInfo.processInfo.environment
        guard environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] == "release_gate" else {
            return
        }

        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let existingRuns = (try? modelContext.fetch(descriptor)) ?? []
        if existingRuns.contains(where: { $0.status == .waitingApproval && $0.deliveryConfigurationJSON != nil }) {
            return
        }

        let repositoryRoot = AppConfiguration.defaultRepositoryRoot().path

        do {
            let workspace = try makeSeedWorkspace(runID: UUID(), prefix: "UITestReleaseGate")
            let worktreeRoot = workspace.workspaceRoot.appendingPathComponent("worktree", isDirectory: true)
            try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)

            let idea = Idea(title: "Release Gate Proof", body: "Seeded repo-backed release gate proof.")
            modelContext.insert(idea)

            let run = Run(
                id: workspace.runID,
                workflowID: "full_mvp_live",
                workflowTitle: "Full MVP Live",
                workflowSnapshotHash: "seeded-workflow",
                catalogSnapshotHash: "seeded-catalog",
                workflowSourcePath: resolvedExamplePath("examples/workflows/full-mvp-live.yaml"),
                catalogSourcePath: resolvedExamplePath("examples/agents/agents.yaml"),
                workflowSnapshotJSON: Data(),
                catalogSnapshotJSON: Data(),
                workspaceRoot: workspace.workspaceRoot.path,
                artifactRoot: workspace.artifactRoot.path,
                planCompilerVersion: 1
            )
            run.idea = idea
            run.status = .waitingApproval
            run.worktreeRoot = worktreeRoot.path
            run.baseRevision = "seededbase"
            run.repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
                configuredIdentifier: "Chainworks Forge",
                repoRoot: repositoryRoot
            )
            run.repoRoot = repositoryRoot
            run.baseBranch = "main"
            run.targetBranch = "dogfood/full-mvp"
            run.releaseTargetID = "sandbox_local"
            run.releaseMode = "sandbox"
            run.totalCostCents = 4200

            let config = DeliveryConfiguration(
                profileID: "chainworks_forge_self",
                profileLabel: "Chainworks Forge (Self)",
                sampleProfileID: nil,
                repoIdentifier: "Chainworks Forge",
                repoRoot: repositoryRoot,
                baseBranch: "main",
                worktreeBasePath: worktreeRoot.deletingLastPathComponent().path,
                targetBranch: "dogfood/full-mvp",
                releaseTargetID: "sandbox_local",
                releaseTargetLabel: "Local Sandbox",
                releaseMode: .sandbox
            )
            run.deliveryConfigurationJSON = try JSONEncoder().encode(config)
            modelContext.insert(run)

            let releaseStage = StageExecution(
                stageID: "state_11_manual_release",
                label: "Manual release gate",
                startedAt: Date().addingTimeInterval(-90),
                status: .waitingApproval,
                iteration: 1,
                attemptNumber: 1
            )
            releaseStage.run = run
            modelContext.insert(releaseStage)

            let releaseExecution = AgentExecution(
                agentID: "lead_orchestrator",
                agentTitle: "Lead / Orchestrator",
                taskName: "prepare_release_gate",
                startedAt: Date().addingTimeInterval(-100),
                status: .completed,
                provider: "claude_code",
                effort: "high"
            )
            releaseExecution.completedAt = Date().addingTimeInterval(-95)
            releaseExecution.stageExecution = releaseStage
            modelContext.insert(releaseExecution)

            let artifactManager = ArtifactManager(modelContext: modelContext)
            _ = try artifactManager.persistOutputs(
                outputs: [
                    "approved_proposal": Data("""
                    {"title":"Seeded proposal","decision":"approved"}
                    """.utf8),
                    "changed_files_manifest": Data("Chainworks Forge/App.swift\nChainworks Forge/Views/ReleaseGateView.swift\n".utf8),
                    "docs_delta": Data("{\"files\":[\"README.md\"],\"summary\":\"Docs updated\"}".utf8),
                    "implementation_review_summary": Data("{\"decision\":\"implemented\"}".utf8),
                    "security_report": Data("{\"status\":\"pass\"}".utf8),
                    "audit_report": Data("{\"status\":\"pass\"}".utf8),
                    "prepush_review_report": Data("{\"status\":\"pass\"}".utf8),
                    "delivery_receipt": Data("{\"status\":\"ready\",\"target\":\"sandbox_local\"}".utf8)
                ],
                agent: ResolvedAgent(
                    id: "lead_orchestrator",
                    title: "Lead / Orchestrator",
                    mode: "orchestration",
                    provider: "claude_code",
                    model: "default",
                    effort: "high",
                    maxTurns: 12,
                    temperature: 0,
                    permissionProfile: "ORCH",
                    skillRef: "orchestrator_core",
                    skillRole: nil,
                    prompt: "Seeded release gate data",
                    outputContract: nil,
                    requiresHumanApproval: false,
                    inputs: [],
                    outputs: ["approved_proposal", "changed_files_manifest", "docs_delta", "implementation_review_summary", "security_report", "audit_report", "prepush_review_report", "delivery_receipt"]
                ),
                agentExecution: releaseExecution,
                workspace: workspace,
                stageID: releaseStage.stageID,
                iteration: releaseStage.iteration,
                attemptNumber: releaseStage.attemptNumber,
                catalog: nil
            )

            let approval = Approval(
                stageID: releaseStage.stageID,
                requestedAt: Date().addingTimeInterval(-60),
                decision: .requested
            )
            approval.run = run
            modelContext.insert(approval)

            try modelContext.save()
        } catch {
            ForgeLogger.test.error("Failed to seed release gate run: \(error.localizedDescription)")
        }
    }

    private static func makeSeedWorkspace(runID: UUID, prefix: String) throws -> RunWorkspace {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = root.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        return RunWorkspace(runID: runID, workspaceRoot: root, artifactRoot: artifactRoot, worktreeRoot: nil)
    }

    private static func resolvedExamplePath(_ relativePath: String) -> String {
        AppConfiguration.preferredExampleURL(repoRelativePath: relativePath)?.path
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent(relativePath)
            .path
    }
}
