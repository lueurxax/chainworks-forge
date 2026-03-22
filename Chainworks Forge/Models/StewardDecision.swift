import Foundation
import SwiftData

/// V3 placeholder — defined but unused until Steward V3 (Experimenter).
@Model final class StewardDecision {
    @Attribute(.unique) var id: UUID
    var decidedAt: Date
    var outcome: DecisionOutcome
    var rationale: String
    var adoptedConfigHash: String?
    var rollbackConfigHash: String?

    @Relationship(inverse: \StewardExperiment.decision)
    var experiment: StewardExperiment?

    init(
        id: UUID = UUID(),
        decidedAt: Date = Date(),
        outcome: DecisionOutcome,
        rationale: String,
        adoptedConfigHash: String? = nil,
        rollbackConfigHash: String? = nil
    ) {
        self.id = id
        self.decidedAt = decidedAt
        self.outcome = outcome
        self.rationale = rationale
        self.adoptedConfigHash = adoptedConfigHash
        self.rollbackConfigHash = rollbackConfigHash
    }
}

enum DecisionOutcome: String, Codable {
    case adopted
    case rolledBack
    case iterateWithNewExperiment
    case deferred
}
