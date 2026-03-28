import Foundation
import SwiftData

@Model final class BenchmarkExecutionRecord {
    @Attribute(.unique) var id: UUID
    var executionMode: BenchmarkExecutionMode
    var linkedRunID: UUID?
    var startedAt: Date
    var completedAt: Date?
    var timeToProposalApprovalSeconds: Double?
    var timeToImplementationApprovalSeconds: Double?
    var timeToFinalReleaseDecisionSeconds: Double?
    var totalOrchestrationTimeSeconds: Double?
    var terminalOutcome: BenchmarkExecutionOutcome
    var evidencePackExportedAt: Date?

    private(set) var artifactLinksJSON: Data
    var notesJSON: Data?

    @Relationship(inverse: \BenchmarkPair.manualRecord)
    var manualPair: BenchmarkPair?

    @Relationship(inverse: \BenchmarkPair.appDrivenRecord)
    var appDrivenPair: BenchmarkPair?

    // Computed accessor for artifactLinks
    var artifactLinks: [BenchmarkArtifactLink] {
        get {
            (try? JSONDecoder().decode([BenchmarkArtifactLink].self, from: artifactLinksJSON)) ?? []
        }
        set {
            artifactLinksJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    // Computed accessor for notes
    var notes: [String]? {
        get {
            guard let data = notesJSON else { return nil }
            return try? JSONDecoder().decode([String].self, from: data)
        }
        set {
            notesJSON = newValue.flatMap { try? JSONEncoder().encode($0) }
        }
    }

    init(
        id: UUID = UUID(),
        executionMode: BenchmarkExecutionMode,
        linkedRunID: UUID? = nil,
        startedAt: Date = Date(),
        completedAt: Date? = nil,
        terminalOutcome: BenchmarkExecutionOutcome = .pending,
        artifactLinks: [BenchmarkArtifactLink] = []
    ) {
        self.id = id
        self.executionMode = executionMode
        self.linkedRunID = linkedRunID
        self.startedAt = startedAt
        self.completedAt = completedAt
        self.terminalOutcome = terminalOutcome
        self.artifactLinksJSON = (try? JSONEncoder().encode(artifactLinks)) ?? Data()
    }
}

enum BenchmarkExecutionMode: String, Codable {
    case manualBaseline
    case appDriven
}

enum BenchmarkExecutionOutcome: String, Codable {
    case pending
    case happyPathCompleted
    case recoveredNonHappyPathCompleted
    case failedUnrecovered
}

struct BenchmarkArtifactLink: Codable, Sendable {
    let artifactID: UUID
    let name: String
    let role: String
}
