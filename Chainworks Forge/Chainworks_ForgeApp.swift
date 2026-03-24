import SwiftUI
import SwiftData

@main
struct Chainworks_ForgeApp: App {

    /// Disable macOS window/scene restoration when running under UI tests.
    /// Without this, `WindowGroup` restores the previous session's window,
    /// creating two overlapping windows that cause XCUITest element queries
    /// to find (and click) elements hidden behind the wrong window — leading
    /// to indefinite hangs.
    init() {
        if ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            UserDefaults.standard.set(false, forKey: "NSQuitAlwaysKeepsWindows")
        }
    }

    var sharedModelContainer: ModelContainer = {
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

    var body: some Scene {
        WindowGroup {
            AppBootstrapView()
        }
        .modelContainer(sharedModelContainer)

        // P005-OPS §10: Optional menu bar extra
        MenuBarExtra("Chainworks Forge", systemImage: "hammer.circle") {
            AppBootstrapMenuBarView()
                .modelContainer(sharedModelContainer)
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

    var body: some View {
        if let service = executionService,
           let appConfigurationStore,
           let providerSettingsStore,
           let providerRegistry {
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
        } else {
            ProgressView("Starting engine...")
                .accessibilityIdentifier("bootstrap-loading")
                .task {
                    bootstrapService()
                }
        }
    }

    @MainActor
    private func bootstrapService() {
        guard executionService == nil else { return }

        let appConfigurationStore = AppConfigurationStore()
        let resolvedConfiguration = BootstrapConfigurationResolver.resolve(store: appConfigurationStore)
        let providerSettingsStore = ProviderSettingsStore()
        let providerRegistry = ProviderRegistry(settingsStore: providerSettingsStore)
        self.appConfigurationStore = appConfigurationStore
        self.providerSettingsStore = providerSettingsStore
        self.providerRegistry = providerRegistry

        let catalog = Self.loadBundledCatalog(appConfiguration: resolvedConfiguration)
        let stewardConfig = Self.loadStewardConfig()
        let liveRuntimeConfiguration = Self.loadLiveRuntimeConfiguration()
        // The simulated executor remains the safe default, but Proposal 004 live runs
        // are resolved per-plan inside ExecutionService using `liveRuntimeConfiguration`.
        let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: catalog)
        let service = ExecutionService(
            modelContext: modelContext,
            executor: executor,
            catalog: catalog,
            stewardConfig: stewardConfig,
            liveRuntimeConfiguration: liveRuntimeConfiguration
        )
        executionService = service

        Self.seedIdeaIfRequested(modelContext: modelContext)
        Self.seedWaitingApprovalRunIfRequested(modelContext: modelContext, catalog: catalog)

        let compiler = RunPlanCompiler(modelContext: modelContext)
        service.resumeInterruptedRuns(compiler: compiler)

        // Proposal 003 — REQ-008: Check if config has changed since last analysis.
        service.checkForConfigChange()

        Task { @MainActor in
            await providerRegistry.refreshHealth()
        }

        if shouldPresentFirstRunWizard(
            configuration: resolvedConfiguration,
            providerSettings: providerSettingsStore.settings
        ) {
            showFirstRunWizard = true
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

    private static func loadLiveRuntimeConfiguration() -> LiveRuntimeConfiguration? {
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

        guard let baseURLString = environment["CHAINWORKS_GOOSE_BASE_URL"],
              let baseURL = URL(string: baseURLString),
              !baseURLString.isEmpty else {
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

        // Proposal 005: Read transport API from environment.
        // Default to .gooseServer when CHAINWORKS_GOOSE_BASE_URL is set.
        let transportAPIString = environment["CHAINWORKS_GOOSE_TRANSPORT_API"]
        let transportAPI: GooseTransportAPI
        if let transportAPIString, let parsed = GooseTransportAPI(rawValue: transportAPIString) {
            transportAPI = parsed
        } else {
            // Default: gooseServer when a base URL is provided (Proposal 005 Section 5.5)
            transportAPI = .gooseServer
        }

        return LiveRuntimeConfiguration(
            baseURL: baseURL,
            apiKey: environment["CHAINWORKS_GOOSE_API_KEY"],
            override: override,
            transportMode: .network,
            transportAPI: transportAPI
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
