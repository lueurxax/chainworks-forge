import Foundation

extension ArtifactFormat: Hashable, Sendable {}

enum RunArtifactBucketKind: String, Codable, Sendable, Hashable, CaseIterable, Identifiable {
    case summary
    case receipt
    case transcript
    case approvalContext = "approval_context"
    case review
    case delivery
    case test
    case diff
    case report
    case release
    case diagnostic
    case other

    var id: String { rawValue }

    var title: String {
        switch self {
        case .summary:
            return "Summary"
        case .receipt:
            return "Receipt"
        case .transcript:
            return "Transcript"
        case .approvalContext:
            return "Approval Context"
        case .review:
            return "Review"
        case .delivery:
            return "Delivery"
        case .test:
            return "Test"
        case .diff:
            return "Diff"
        case .report:
            return "Report"
        case .release:
            return "Release"
        case .diagnostic:
            return "Diagnostic"
        case .other:
            return "Other"
        }
    }
}

struct RunArtifactHierarchy: Codable, Sendable, Hashable {
    let runID: UUID
    let latestSummaryArtifactID: UUID?
    let latestImmutableReportArtifactID: UUID?
    let latestReportVersion: Int
    let promotedArtifacts: [RunArtifactLeaf]
    let stageGroups: [RunArtifactStageGroup]

    var allArtifacts: [RunArtifactLeaf] {
        let nested = stageGroups.flatMap(\.allArtifacts)
        return deduplicatedArtifacts(promotedArtifacts + nested)
    }

    private func deduplicatedArtifacts(_ artifacts: [RunArtifactLeaf]) -> [RunArtifactLeaf] {
        var seen = Set<UUID>()
        return artifacts.filter { seen.insert($0.artifactID).inserted }
    }
}

struct RunArtifactStageGroup: Codable, Sendable, Hashable, Identifiable {
    let stageExecutionID: UUID?
    let stageID: String
    let stageLabel: String
    let iteration: Int
    let attemptNumber: Int
    let stageBuckets: [RunArtifactSemanticBucket]
    let agentGroups: [RunArtifactAgentGroup]

    var id: String {
        if let stageExecutionID {
            return stageExecutionID.uuidString
        }
        return "\(stageID)::\(iteration)::\(attemptNumber)"
    }

    var allArtifacts: [RunArtifactLeaf] {
        let stageArtifacts = stageBuckets.flatMap { $0.artifacts }
        let agentArtifacts = agentGroups.flatMap { $0.allArtifacts }
        return stageArtifacts + agentArtifacts
    }
}

struct RunArtifactAgentGroup: Codable, Sendable, Hashable, Identifiable {
    let agentExecutionID: UUID?
    let agentID: String
    let agentTitle: String
    let semanticBuckets: [RunArtifactSemanticBucket]

    var id: String {
        if let agentExecutionID {
            return agentExecutionID.uuidString
        }
        return agentID
    }

    var allArtifacts: [RunArtifactLeaf] {
        semanticBuckets.flatMap { $0.artifacts }
    }
}

struct RunArtifactSemanticBucket: Codable, Sendable, Hashable, Identifiable {
    let bucket: RunArtifactBucketKind
    let artifacts: [RunArtifactLeaf]

    var id: String { bucket.rawValue }
}

struct RunArtifactLeaf: Codable, Sendable, Hashable, Identifiable {
    let artifactID: UUID
    let name: String
    let contractID: String
    let format: ArtifactFormat
    let createdAt: Date
    let fileURL: URL?
    let sizeBytes: Int64?
    let stageID: String
    let stageExecutionID: UUID?
    let stageLabel: String
    let iteration: Int
    let attemptNumber: Int
    let agentID: String
    let agentExecutionID: UUID?
    let agentTitle: String
    let provider: String
    let model: String?
    let effort: String?
    let agentAttemptNumber: Int?
    let artifactLineageKind: String?
    let supersedesArtifactID: UUID?
    let supersedesAgentArtifactID: UUID?
    let reportKind: String?
    let reportVersion: Int?
    let isPinned: Bool
    let isPromoted: Bool
    let isLatestSummaryReport: Bool
    let isLatestImmutableReport: Bool

    var id: UUID { artifactID }
}
