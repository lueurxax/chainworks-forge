import Foundation
import SwiftData

/// V3 placeholder — defined but unused until Steward V3 (Experimenter).
@Model final class StewardExperiment {
    @Attribute(.unique) var id: UUID
    var createdAt: Date
    var startedAt: Date?
    var completedAt: Date?
    var experimentType: ExperimentType
    var controlConfigHash: String
    var treatmentConfigHash: String
    var minimumSampleSize: Int
    var actualSampleSize: Int
    var rollbackCondition: String
    var status: ExperimentStatus
    var evaluationArtifactPath: String?

    @Relationship(inverse: \StewardRecommendation.experiment)
    var recommendation: StewardRecommendation?

    @Relationship(deleteRule: .cascade)
    var decision: StewardDecision?

    init(
        id: UUID = UUID(),
        createdAt: Date = Date(),
        experimentType: ExperimentType,
        controlConfigHash: String,
        treatmentConfigHash: String,
        minimumSampleSize: Int,
        actualSampleSize: Int = 0,
        rollbackCondition: String,
        status: ExperimentStatus = .planned
    ) {
        self.id = id
        self.createdAt = createdAt
        self.experimentType = experimentType
        self.controlConfigHash = controlConfigHash
        self.treatmentConfigHash = treatmentConfigHash
        self.minimumSampleSize = minimumSampleSize
        self.actualSampleSize = actualSampleSize
        self.rollbackCondition = rollbackCondition
        self.status = status
    }
}

enum ExperimentType: String, Codable {
    case championChallenger
    case limitedRollout
    case abTest
}

enum ExperimentStatus: String, Codable {
    case planned
    case running
    case completed
    case rolledBack
    case cancelled
}
