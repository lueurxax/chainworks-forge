import SwiftUI
import SwiftData

@main
struct Chainworks_ForgeApp: App {
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
    }
}

// MARK: - AppBootstrapView (ARCH-022: app-scoped ExecutionService wiring)

struct AppBootstrapView: View {
    @Environment(\.modelContext) private var modelContext
    @State private var executionService: ExecutionService?

    var body: some View {
        if let service = executionService {
            ContentView()
                .environment(service)
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

        let catalog = Self.loadBundledCatalog()
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

        let compiler = RunPlanCompiler(modelContext: modelContext)
        service.resumeInterruptedRuns(compiler: compiler)

        // Proposal 003 — REQ-008: Check if config has changed since last analysis.
        service.checkForConfigChange()
        Self.seedIdeaIfRequested(modelContext: modelContext)
    }

    private static func loadBundledCatalog() -> AgentCatalog? {
        let candidates: [URL?] = [
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
                transportMode: .fixtureProposalLoopSuccess
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

        return LiveRuntimeConfiguration(
            baseURL: baseURL,
            apiKey: environment["CHAINWORKS_GOOSE_API_KEY"],
            override: override,
            transportMode: .network
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
}
