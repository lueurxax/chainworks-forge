import Foundation
import SwiftData

@Model final class AggregateSettlementRecord {
    @Attribute(.unique) var id: UUID
    var runID: UUID
    var stageExecutionID: UUID
    var aggregateStepID: String
    var lineageID: String
    var canonicalOutcome: AgentCanonicalOutcome
    var inputCoverageJSON: Data?
    var outputArtifactName: String?
    var validationFailureJSON: Data?
    var evidencePacketJSON: Data?
    var settledAt: Date?

    init(
        id: UUID = UUID(),
        runID: UUID,
        stageExecutionID: UUID,
        aggregateStepID: String,
        lineageID: String,
        canonicalOutcome: AgentCanonicalOutcome,
        inputCoverageJSON: Data? = nil,
        outputArtifactName: String? = nil,
        validationFailureJSON: Data? = nil,
        evidencePacketJSON: Data? = nil,
        settledAt: Date? = nil
    ) {
        self.id = id
        self.runID = runID
        self.stageExecutionID = stageExecutionID
        self.aggregateStepID = aggregateStepID
        self.lineageID = lineageID
        self.canonicalOutcome = canonicalOutcome
        self.inputCoverageJSON = inputCoverageJSON
        self.outputArtifactName = outputArtifactName
        self.validationFailureJSON = validationFailureJSON
        self.evidencePacketJSON = evidencePacketJSON
        self.settledAt = settledAt
    }
}

struct AggregateInputCoverage: Codable, Equatable, Sendable {
    let requiredInputs: [String]
    let availableInputs: [String]
    let missingInputs: [String]
}
