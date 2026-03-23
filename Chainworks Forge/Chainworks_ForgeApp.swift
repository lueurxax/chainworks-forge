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
        let executor = SimulatedAgentExecutor(simulatedDelay: 0.5, catalog: catalog)
        let service = ExecutionService(
            modelContext: modelContext,
            executor: executor,
            catalog: catalog
        )
        executionService = service

        // Resume interrupted runs on app launch (ARCH-029)
        let compiler = RunPlanCompiler(modelContext: modelContext)
        service.resumeInterruptedRuns(compiler: compiler)
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
}
