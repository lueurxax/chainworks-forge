import SwiftUI
import SwiftData
#if os(macOS)
import AppKit
#endif

private enum UIAutomationDiagnostics {
    private static let logURL = URL(fileURLWithPath: "/tmp/chainworks-ui-automation.log")

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

@main
struct Chainworks_ForgeApp: App {
    static let processEnvironment = ProcessInfo.processInfo.environment
    static let isTestHost = processEnvironment["XCTestConfigurationFilePath"] != nil
    static let isUIAutomationHost = processEnvironment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }
    static let isUnitTestHost = isTestHost && !isUIAutomationHost
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
        ])
        let modelConfiguration = ModelConfiguration(
            schema: schema,
            isStoredInMemoryOnly: environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        )

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
        if ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            UserDefaults.standard.set(false, forKey: "NSQuitAlwaysKeepsWindows")
        }
        UIAutomationDiagnostics.log(
            "app.init uiAutomation=\(Self.isUIAutomationHost) unitTest=\(Self.isUnitTestHost) " +
            "directSurface=\(Self.processEnvironment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] ?? "nil")"
        )
    }

    var body: some Scene {
        Window("Chainworks Forge", id: "main-window") {
            RootHostView()
        }
        .modelContainer(Self.sharedModelContainer)
        .defaultSize(width: 1200, height: 800)
    }

}

final class AutomationFallbackAppDelegate: NSObject, NSApplicationDelegate {
    private var fallbackWindow: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard Chainworks_ForgeApp.isUIAutomationHost else { return }
        UIAutomationDiagnostics.log("applicationDidFinishLaunching windows=\(NSApp.windows.count)")

        Task { @MainActor in
            if !NSApp.windows.isEmpty {
                UIAutomationDiagnostics.log("nativeWindowDetected attempt=0 count=\(NSApp.windows.count)")
                return
            }

            try? await Task.sleep(for: .milliseconds(100))

            if !NSApp.windows.isEmpty {
                UIAutomationDiagnostics.log("nativeWindowDetected attempt=1 count=\(NSApp.windows.count)")
                return
            }

            UIAutomationDiagnostics.log("creatingFallbackWindow")

            let hostingController = NSHostingController(
                rootView: RootHostView()
                    .modelContainer(Chainworks_ForgeApp.sharedModelContainer)
            )
            let window = NSWindow(contentViewController: hostingController)
            window.title = "Chainworks Forge"
            window.identifier = NSUserInterfaceItemIdentifier("chainworks-fallback-window")
            window.setContentSize(NSSize(width: 1200, height: 800))
            window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
            window.center()
            window.makeKeyAndOrderFront(nil)
            window.orderFrontRegardless()

            NSApp.setActivationPolicy(.regular)
            NSRunningApplication.current.activate(options: [.activateIgnoringOtherApps])
            NSApp.activate(ignoringOtherApps: true)

            fallbackWindow = window
            UIAutomationDiagnostics.log("fallbackWindowCreated windows=\(NSApp.windows.count) isVisible=\(window.isVisible)")
        }
    }
}

private struct UnitTestHostView: View {
    var body: some View {
        Color.clear
            .accessibilityIdentifier("unit-test-host")
    }
}

private struct RootHostView: View {
    var body: some View {
        Group {
            if Chainworks_ForgeApp.isUnitTestHost {
                UnitTestHostView()
            } else {
                AppBootstrapView()
            }
        }
        .task {
            guard Chainworks_ForgeApp.isUIAutomationHost else { return }
            #if os(macOS)
            UIAutomationDiagnostics.log("rootHost.task.begin windows=\(NSApp.windows.count)")
            NSApp.setActivationPolicy(.regular)
            for attempt in 0..<20 {
                if attempt > 0 {
                    try? await Task.sleep(for: .milliseconds(150))
                }
                await MainActor.run {
                    NSRunningApplication.current.activate(options: [.activateIgnoringOtherApps])
                    NSApp.activate(ignoringOtherApps: true)
                    for window in NSApp.windows {
                        window.collectionBehavior.remove(.transient)
                        window.makeKeyAndOrderFront(nil)
                        window.orderFrontRegardless()
                    }
                }
                UIAutomationDiagnostics.log("rootHost.task.activation attempt=\(attempt) windows=\(NSApp.windows.count)")
            }
            UIAutomationDiagnostics.log("rootHost.task.end windows=\(NSApp.windows.count)")
            #endif
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
    @State private var gooseServerManager: GooseServerManager?
    @State private var showFirstRunWizard = false
    private let forcedUISurface = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
        .flatMap(ContentView.UISurface.init(rawValue:))

    var body: some View {
        if let service = executionService,
           let appConfigurationStore,
           let providerSettingsStore,
           let providerRegistry,
           let gooseServerManager {
            bootstrappedRoot(
                service: service,
                appConfigurationStore: appConfigurationStore,
                providerSettingsStore: providerSettingsStore,
                providerRegistry: providerRegistry,
                gooseServerManager: gooseServerManager
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

        let appConfigurationStore = AppConfigurationStore()
        let resolvedConfiguration = BootstrapConfigurationResolver.resolve(store: appConfigurationStore)
        let providerSettingsStore = ProviderSettingsStore()
        let providerRegistry = ProviderRegistry(settingsStore: providerSettingsStore)
        let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)
        self.appConfigurationStore = appConfigurationStore
        self.providerSettingsStore = providerSettingsStore
        self.providerRegistry = providerRegistry
        self.gooseServerManager = gooseServerManager
        UIAutomationDiagnostics.log(
            "bootstrapService.config directSurface=\(environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] ?? "nil") " +
            "inMemory=\(environment["CHAINWORKS_IN_MEMORY_STORE"] ?? "nil")"
        )

        let catalog = Self.loadBundledCatalog(appConfiguration: resolvedConfiguration)
        let stewardConfig = Self.loadStewardConfig()
        if !isUnitTestHost {
            await gooseServerManager.bootstrap()
        }
        let liveRuntimeConfiguration = isUnitTestHost
            ? nil
            : Self.loadLiveRuntimeConfiguration(gooseServerManager: gooseServerManager)
        // The simulated executor remains the safe default, but Proposal 004 live runs
        // are resolved per-plan inside ExecutionService using `liveRuntimeConfiguration`.
        let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: catalog)
        let service = ExecutionService(
            modelContext: modelContext,
            executor: executor,
            catalog: catalog,
            stewardConfig: stewardConfig,
            liveRuntimeConfiguration: liveRuntimeConfiguration,
            gooseServerManager: gooseServerManager
        )
        executionService = service

        Self.seedIdeaIfRequested(modelContext: modelContext)
        Self.seedWaitingApprovalRunIfRequested(modelContext: modelContext, catalog: catalog)
        Self.seedWorkflowMapRunIfRequested(modelContext: modelContext)

        if !isUnitTestHost {
            let compiler = RunPlanCompiler(modelContext: modelContext)
            service.resumeInterruptedRuns(compiler: compiler)

            // Proposal 003 — REQ-008: Check if config has changed since last analysis.
            service.checkForConfigChange()
        }

        if !isUnitTestHost {
            Task { @MainActor in
                await providerRegistry.refreshHealth()
            }
        }

        if shouldPresentFirstRunWizard(
            configuration: resolvedConfiguration,
            providerSettings: providerSettingsStore.settings
        ) {
            showFirstRunWizard = true
        }
        UIAutomationDiagnostics.log("bootstrapService.end showFirstRunWizard=\(showFirstRunWizard)")
    }

    @ViewBuilder
    private func bootstrappedRoot(
        service: ExecutionService,
        appConfigurationStore: AppConfigurationStore,
        providerSettingsStore: ProviderSettingsStore,
        providerRegistry: ProviderRegistry,
        gooseServerManager: GooseServerManager
    ) -> some View {
        if let forcedUISurface {
            VStack(spacing: 0) {
                Button("UI Test Surface: \(forcedUISurface.rawValue)") {}
                    .buttonStyle(.plain)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .accessibilityIdentifier("ui-test-direct-surface-ready-\(forcedUISurface.rawValue)")

                Group {
                    switch forcedUISurface {
                    case .providerSettings:
                        ProviderSettingsView()
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    case .pilotReadiness:
                        PilotReadinessView()
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    case .firstRunSetup:
                        FirstRunSetupWizard(isPresented: .constant(true))
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    case .ideaArchive:
                        UITestIdeaArchiveSurface()
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    case .workflowMap:
                        UITestWorkflowMapSurface()
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    case .gooseAssistant:
                        UITestGooseAssistantSurface()
                            .environment(service)
                            .environment(appConfigurationStore)
                            .environment(providerSettingsStore)
                            .environment(providerRegistry)
                            .environment(gooseServerManager)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .frame(minWidth: 960, minHeight: 720)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("ui-test-direct-surface-container-\(forcedUISurface.rawValue)")
        } else {
            ContentView()
                .environment(service)
                .environment(appConfigurationStore)
                .environment(providerSettingsStore)
                .environment(providerRegistry)
                .environment(gooseServerManager)
                .sheet(isPresented: $showFirstRunWizard) {
                    FirstRunSetupWizard(isPresented: $showFirstRunWizard)
                        .environment(service)
                        .environment(appConfigurationStore)
                        .environment(providerSettingsStore)
                        .environment(providerRegistry)
                        .environment(gooseServerManager)
                }
        }
    }

    private static func loadBundledCatalog(appConfiguration: AppConfiguration) -> AgentCatalog? {
        let candidates: [URL?] = [
            URL(fileURLWithPath: appConfiguration.agentCatalogSourcePath),
            Bundle.main.url(forResource: "agents", withExtension: "yaml"),
            URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/examples/agents/agents.yaml")
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
        if environment["CHAINWORKS_UI_TEST_INITIAL_TAB"] != nil || environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            return false
        }

        if !FileManager.default.fileExists(atPath: configuration.workflowSourcePath) {
            return true
        }

        if !FileManager.default.fileExists(atPath: configuration.agentCatalogSourcePath) {
            return true
        }

        return providerSettings.configuredProviders.isEmpty
    }

    private static func loadBundledWorkflow(named resourceName: String, repoRelativePath: String) -> WorkflowDefinition? {
        let candidates: [URL?] = [
            Bundle.main.url(forResource: resourceName, withExtension: "yaml"),
            URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/\(repoRelativePath)")
        ]
        for case let url? in candidates {
            if let workflow = try? YAMLParser.loadWorkflow(from: url) {
                return workflow
            }
        }
        return nil
    }

    private static func loadStewardConfig() -> StewardConfig? {
        let candidates: [URL?] = [
            Bundle.main.url(forResource: "steward_config", withExtension: "yaml"),
            URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/examples/steward/steward_config.yaml")
        ]
        for case let url? in candidates {
            if let config = try? YAMLParser.loadStewardConfig(from: url) {
                // REQ-003: Enforce validation at load time.
                let issues = YAMLValidator.validateStewardConfig(config)
                let errors = issues.filter { $0.severity == .error }
                if !errors.isEmpty {
                    print("[Steward] steward_config.yaml validation failed: \(errors.map(\.message).joined(separator: "; ")). Using defaults.")
                    return StewardConfig.defaultConfig
                }
                return config
            }
        }
        return nil
    }

    private static func loadLiveRuntimeConfiguration(gooseServerManager: GooseServerManager?) -> LiveRuntimeConfiguration? {
        let environment = ProcessInfo.processInfo.environment
        if environment["CHAINWORKS_GOOSE_FIXTURE_MODE"] == "proposal_loop_success" {
            let override = LiveExecutionOverride(
                enabled: true,
                provider: environment["CHAINWORKS_LIVE_PROVIDER"] ?? "claude_code",
                model: environment["CHAINWORKS_LIVE_MODEL"] ?? "fixture-model",
                effort: environment["CHAINWORKS_LIVE_EFFORT"] ?? "high"
            )

            return LiveRuntimeConfiguration(
                baseURL: URL(string: "http://fixture.local")!,
                apiKey: nil,
                override: override,
                transportMode: .fixtureProposalLoopSuccess,
                transportAPI: .bespoke
            )
        }

        guard let managed = gooseServerManager?.liveRuntimeConfiguration else {
            return nil
        }

        let provider = environment["CHAINWORKS_LIVE_PROVIDER"]
        let model = environment["CHAINWORKS_LIVE_MODEL"]
        let effort = environment["CHAINWORKS_LIVE_EFFORT"]

        let override: LiveExecutionOverride?
        if let provider, !provider.isEmpty,
           let model, !model.isEmpty,
           let effort, !effort.isEmpty {
            override = LiveExecutionOverride(
                enabled: true,
                provider: provider,
                model: model,
                effort: effort
            )
        } else {
            override = nil
        }

        return LiveRuntimeConfiguration(
            baseURL: managed.baseURL,
            apiKey: managed.apiKey,
            override: override,
            transportMode: managed.transportMode,
            transportAPI: managed.transportAPI
        )
    }

    @MainActor
    private static func seedIdeaIfRequested(modelContext: ModelContext) {
        let environment = ProcessInfo.processInfo.environment
        guard let title = environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"],
              !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return
        }

        let descriptor = FetchDescriptor<Idea>()
        let existingIdeas = (try? modelContext.fetch(descriptor)) ?? []
        if existingIdeas.contains(where: { $0.title == title }) {
            return
        }

        let idea = Idea(
            title: title,
            body: environment["CHAINWORKS_UI_TEST_SEED_IDEA_BODY"] ?? "Seeded UI test idea",
            attachmentPath: nil
        )
        modelContext.insert(idea)
        try? modelContext.save()
    }

    @MainActor
    private static func seedWaitingApprovalRunIfRequested(
        modelContext: ModelContext,
        catalog: AgentCatalog?
    ) {
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
            try? modelContext.save()
            return
        }

        do {
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
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
            writerExecution.gooseSessionID = "fixture-seeded-session"
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
            reviewExecution.gooseSessionID = "fixture-seeded-session"
            modelContext.insert(reviewExecution)

            let artifactManager = ArtifactManager(modelContext: modelContext)
            _ = try artifactManager.persistOutputs(
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
            _ = try artifactManager.persistOutputs(
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

            let approval = Approval(
                stageID: approvalStage.stageID,
                requestedAt: Date().addingTimeInterval(-45),
                decision: .requested
            )
            approval.run = run
            modelContext.insert(approval)

            try modelContext.save()
        } catch {
            print("Failed to seed waiting approval run: \(error.localizedDescription)")
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

    private static func makeSeedWorkspace(runID: UUID, prefix: String) throws -> RunWorkspace {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = root.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        return RunWorkspace(runID: runID, workspaceRoot: root, artifactRoot: artifactRoot, worktreeRoot: nil)
    }

    private static func resolvedExamplePath(_ relativePath: String) -> String {
        let candidates = [
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent(relativePath),
            URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Documents/Chainworks Forge/\(relativePath)")
        ]
        return candidates.first(where: { FileManager.default.isReadableFile(atPath: $0.path) })?.path
            ?? candidates[0].path
    }
}
