import Foundation
import SwiftData

@MainActor
enum PersistedRunGraph {
    static func stageExecutions(for run: Run) -> [StageExecution] {
        guard let modelContext = run.modelContext else {
            return run.stageExecutions
        }

        let runID = run.id
        let descriptor = FetchDescriptor<StageExecution>(
            predicate: #Predicate<StageExecution> { stage in
                stage.run?.id == runID
            }
        )

        if let fetched = try? modelContext.fetch(descriptor) {
            return fetched
        }

        return run.stageExecutions
    }

    static func approvals(for run: Run) -> [Approval] {
        guard let modelContext = run.modelContext else {
            return run.approvals
        }

        let runID = run.id
        let descriptor = FetchDescriptor<Approval>(
            predicate: #Predicate<Approval> { approval in
                approval.run?.id == runID
            }
        )

        if let fetched = try? modelContext.fetch(descriptor) {
            return fetched
        }

        return run.approvals
    }
}
