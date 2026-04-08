import Foundation
import SwiftData

@MainActor
enum PersistedRunGraph {
    static func stageExecutions(for run: Run) -> [StageExecution] {
        guard let modelContext = run.modelContext else {
            return run.stageExecutions
        }

        let descriptor = FetchDescriptor<StageExecution>()

        if let fetched = try? modelContext.fetch(descriptor) {
            return fetched.filter { $0.run?.id == run.id }
        }

        return run.stageExecutions
    }

    static func approvals(for run: Run) -> [Approval] {
        guard let modelContext = run.modelContext else {
            return run.approvals
        }

        let descriptor = FetchDescriptor<Approval>()

        if let fetched = try? modelContext.fetch(descriptor) {
            return fetched.filter { $0.run?.id == run.id }
        }

        return run.approvals
    }
}
