import Foundation
import SwiftData

@Model final class AgentExecution {
    @Attribute(.unique) var id: UUID
    var agentID: String
    var agentTitle: String
    var taskName: String
    var startedAt: Date
    var completedAt: Date?
    var status: AgentStatus
    var provider: String
    var effort: String
    var costCents: Int64?
    var logSnippet: String?
    var gooseSessionID: String?

    // Proposal 004: Live provider fields (Section 11.1)
    var providerSessionID: String?
    var providerRequestID: String?
    var transcriptArtifactPath: String?
    var resolvedBackendProfileID: String?
    var consumedInputArtifactNamesJSON: Data?

    // Steward data model additions (Proposal 003 — optional, lightweight migration)
    var agentConfigHash: String?
    var skillSnapshotHash: String?
    var transcriptPath: String?
    var toolTracePath: String?
    var retryReason: String?

    @Relationship(inverse: \StageExecution.agentExecutions)
    var stageExecution: StageExecution?

    @Relationship(deleteRule: .cascade)
    var artifacts: [Artifact] = []

    init(id: UUID = UUID(), agentID: String, agentTitle: String, taskName: String, startedAt: Date = Date(), status: AgentStatus = .pending, provider: String, effort: String) {
        self.id = id
        self.agentID = agentID
        self.agentTitle = agentTitle
        self.taskName = taskName
        self.startedAt = startedAt
        self.status = status
        self.provider = provider
        self.effort = effort
    }
}

enum AgentStatus: String, Codable {
    case pending
    case ready
    case running
    case completed
    case failed
    case cancelled
    case skipped
}
