import Foundation
import SwiftData
@testable import Chainworks_Forge

extension PreviewSupport {
    @MainActor
    static func makeExecutionService(modelContext: ModelContext? = nil) -> ExecutionService {
        let context = modelContext ?? makeModelContainer().mainContext
        return ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor()
        )
    }

    static func workflowExampleURL(_ relativePath: String) -> URL {
        repoExampleURL("workflows/\(relativePath)")
    }
}
