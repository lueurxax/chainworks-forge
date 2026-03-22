import Foundation
import SwiftData

@Model final class StageExecution {
    @Attribute(.unique) var id: UUID
    var stageID: String
    var label: String
    var startedAt: Date
    var completedAt: Date?
    var status: StageStatus
    var iteration: Int
    var attemptNumber: Int

    @Relationship(inverse: \Run.stageExecutions)
    var run: Run?

    @Relationship(deleteRule: .cascade)
    var agentExecutions: [AgentExecution] = []

    init(id: UUID = UUID(), stageID: String, label: String, startedAt: Date = Date(), status: StageStatus = .pending, iteration: Int = 1, attemptNumber: Int = 1) {
        self.id = id
        self.stageID = stageID
        self.label = label
        self.startedAt = startedAt
        self.status = status
        self.iteration = iteration
        self.attemptNumber = attemptNumber
    }
}

enum StageStatus: String, Codable {
    case pending
    case ready
    case running
    case waitingApproval
    case blocked
    case completed
    case failed
    case skipped
}
