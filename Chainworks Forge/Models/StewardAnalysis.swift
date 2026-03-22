import Foundation
import SwiftData

@Model final class StewardAnalysis {
    @Attribute(.unique) var id: UUID
    var createdAt: Date
    var windowStart: Date
    var windowEnd: Date
    var runCount: Int
    var cohortKeysJSON: Data // [String: String] stored as JSON
    var cohortQuality: CohortQuality
    var metricsSnapshotPath: String
    var baselineSnapshotPath: String
    var degradationsDetected: Int
    var reportArtifactPath: String
    var auditArtifactPath: String?
    var status: AnalysisStatus
    var workflowCatalogSnapshotHash: String
    var stewardConfigSnapshotHash: String

    @Relationship(deleteRule: .cascade)
    var recommendations: [StewardRecommendation] = []

    @Relationship(deleteRule: .cascade)
    var analysisRunLinks: [StewardAnalysisRunLink] = []

    // Computed accessor for cohortKeys
    var cohortKeys: [String: String] {
        get {
            (try? JSONDecoder().decode([String: String].self, from: cohortKeysJSON)) ?? [:]
        }
        set {
            cohortKeysJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    init(
        id: UUID = UUID(),
        createdAt: Date = Date(),
        windowStart: Date,
        windowEnd: Date,
        runCount: Int,
        cohortKeys: [String: String] = [:],
        cohortQuality: CohortQuality = .strong,
        metricsSnapshotPath: String,
        baselineSnapshotPath: String,
        degradationsDetected: Int = 0,
        reportArtifactPath: String,
        auditArtifactPath: String? = nil,
        status: AnalysisStatus = .completed,
        workflowCatalogSnapshotHash: String,
        stewardConfigSnapshotHash: String
    ) {
        self.id = id
        self.createdAt = createdAt
        self.windowStart = windowStart
        self.windowEnd = windowEnd
        self.runCount = runCount
        self.cohortKeysJSON = (try? JSONEncoder().encode(cohortKeys)) ?? Data()
        self.cohortQuality = cohortQuality
        self.metricsSnapshotPath = metricsSnapshotPath
        self.baselineSnapshotPath = baselineSnapshotPath
        self.degradationsDetected = degradationsDetected
        self.reportArtifactPath = reportArtifactPath
        self.auditArtifactPath = auditArtifactPath
        self.status = status
        self.workflowCatalogSnapshotHash = workflowCatalogSnapshotHash
        self.stewardConfigSnapshotHash = stewardConfigSnapshotHash
    }
}

enum AnalysisStatus: String, Codable {
    case completed
    case inconclusive
    case superseded
}

enum CohortQuality: String, Codable {
    case strong
    case acceptable
    case weak
}
