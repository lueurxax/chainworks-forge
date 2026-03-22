import Foundation
import SwiftData

@Model final class Artifact {
    @Attribute(.unique) var id: UUID
    var name: String
    var contractID: String
    var format: ArtifactFormat
    var filePath: String
    var checksumSHA256: String?
    var createdAt: Date
    var sizeBytes: Int64?
    var runID: UUID
    var stageID: String
    var agentID: String
    var provider: String
    var model: String?
    var effort: String?
    var attemptNumber: Int

    @Relationship(inverse: \AgentExecution.artifacts)
    var agentExecution: AgentExecution?

    init(id: UUID = UUID(), name: String, contractID: String, format: ArtifactFormat, filePath: String, createdAt: Date = Date(), runID: UUID, stageID: String, agentID: String, provider: String, attemptNumber: Int = 1) {
        self.id = id
        self.name = name
        self.contractID = contractID
        self.format = format
        self.filePath = filePath
        self.createdAt = createdAt
        self.runID = runID
        self.stageID = stageID
        self.agentID = agentID
        self.provider = provider
        self.attemptNumber = attemptNumber
    }
}

enum ArtifactFormat: String, Codable {
    case json, markdown, diff, report
}
