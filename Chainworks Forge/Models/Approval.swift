import Foundation
import SwiftData

@Model final class Approval {
    @Attribute(.unique) var id: UUID
    var stageID: String
    var requestedAt: Date
    var decidedAt: Date?
    var decision: ApprovalDecision
    var comment: String?
    var expiresAt: Date?

    @Relationship(inverse: \Run.approvals)
    var run: Run?

    init(id: UUID = UUID(), stageID: String, requestedAt: Date = Date(), decision: ApprovalDecision = .pending) {
        self.id = id
        self.stageID = stageID
        self.requestedAt = requestedAt
        self.decision = decision
    }
}

enum ApprovalDecision: String, Codable {
    case pending
    case requested
    case granted
    case rejected
    case expired
}
