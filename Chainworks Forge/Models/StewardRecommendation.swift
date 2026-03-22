import Foundation
import SwiftData

@Model final class StewardRecommendation {
    @Attribute(.unique) var id: UUID
    var createdAt: Date
    var category: RecommendationCategory
    var summary: String
    var targetMetric: String
    var proposedPatchPath: String?
    var confidenceLevel: ConfidenceLevel
    var status: RecommendationStatus
    var decisionComment: String?
    var decidedAt: Date?

    @Relationship(inverse: \StewardAnalysis.recommendations)
    var analysis: StewardAnalysis?

    @Relationship
    var experiment: StewardExperiment?

    init(
        id: UUID = UUID(),
        createdAt: Date = Date(),
        category: RecommendationCategory,
        summary: String,
        targetMetric: String,
        proposedPatchPath: String? = nil,
        confidenceLevel: ConfidenceLevel = .medium,
        status: RecommendationStatus = .proposed,
        decisionComment: String? = nil,
        decidedAt: Date? = nil
    ) {
        self.id = id
        self.createdAt = createdAt
        self.category = category
        self.summary = summary
        self.targetMetric = targetMetric
        self.proposedPatchPath = proposedPatchPath
        self.confidenceLevel = confidenceLevel
        self.status = status
        self.decisionComment = decisionComment
        self.decidedAt = decidedAt
    }
}

enum RecommendationCategory: String, Codable {
    case agentTuning
    case workflowTuning
    case backendChange
    case inputContractChange
    case other
}

enum ConfidenceLevel: String, Codable {
    case high
    case medium
    case low
}

enum RecommendationStatus: String, Codable {
    case proposed
    case approved
    case rejected
    case superseded
    case adoptedAfterExperiment
    case rolledBack
}
