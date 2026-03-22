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
            ContentView()
        }
        .modelContainer(sharedModelContainer)
    }
}
