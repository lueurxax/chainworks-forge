import SwiftUI
import SwiftData

@main
struct Chainworks_ForgeApp: App {
    var sharedModelContainer: ModelContainer = {
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
        let modelConfiguration = ModelConfiguration(schema: schema, isStoredInMemoryOnly: false)

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

/// Bootstrap view that creates the app-scoped ExecutionService (ARCH-022)
/// and resumes interrupted runs on launch (ARCH-029).
///
/// Proposal 004: also loads Steward config and prepares live executor infrastructure.
struct AppBootstrapView: View {
    @Environment(\.modelContext) private var modelContext
    @State private var executionService: ExecutionService?

    var body: some View {
        Group {
            if let service = executionService {
                ContentView()
                    .environment(service)
            } else {
                ProgressView("Starting engine...")
            }
        }
        .task { @MainActor in
            guard executionService == nil else { return }

            // Load catalog from bundle or repo for contract-aware output generation
            let catalog = loadBundledCatalog()

            // Load steward config (Proposal 003)
            let stewardConfig = loadStewardConfig()

            // Create app-scoped ExecutionService with SimulatedAgentExecutor
            // Live mode uses GooseAgentExecutor, selected per-run in StartRunSheet (Proposal 004)
            let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: catalog)
            let service = ExecutionService(
                modelContext: modelContext,
                executor: executor,
                catalog: catalog,
                stewardConfig: stewardConfig
            )
            executionService = service

            // Resume interrupted runs on app launch (ARCH-029)
            let compiler = RunPlanCompiler(modelContext: modelContext)
            service.resumeInterruptedRuns(compiler: compiler)
        }
    }

    /// Attempt to load the bundled agents.yaml catalog.
    private func loadBundledCatalog() -> AgentCatalog? {
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

    /// Attempt to load the steward_config.yaml (Proposal 003).
    private func loadStewardConfig() -> StewardConfig? {
        let candidates: [URL?] = [
            Bundle.main.url(forResource: "steward_config", withExtension: "yaml"),
            URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/examples/steward/steward_config.yaml")
        ]
        for case let url? in candidates {
            if let config = try? YAMLParser.loadStewardConfig(from: url) {
                return config
            }
        }
        return nil
    }
}
