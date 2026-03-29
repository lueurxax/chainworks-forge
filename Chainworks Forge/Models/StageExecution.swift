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
    var lineageID: String?
    var settlementKind: StageSettlementKind?
    var settledAt: Date?
    var activeOwnerToken: String?

    // Proposal 013 — Layer N: Retry and Attempt Truth (§5.3)
    var retryMode: String?               // RetryMode.rawValue: "agent_retry" | "stage_retry" | "fresh_execution"
    var triggerReason: String?            // Why this attempt was created
    var supersedesAttemptNumber: Int?     // Which attempt this supersedes (stage retry)

    // Proposal 013 — Layer O: Failed Stage Evidence
    var validationFailureJSON: Data?     // Serialized ValidationFailureRecord
    var evidencePacketJSON: Data?        // Serialized FailedStageEvidencePacket
    var recoverySnapshotJSON: Data?      // Serialized RecoveryActionSnapshot

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
