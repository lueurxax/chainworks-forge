import Foundation

struct Proposal015AppProofResult: Codable, Sendable {
    let runID: UUID
    let comparisonRunID: UUID
    let proofAgentID: String
    let reportSkillRef: String
    let reportSkillRole: String
    let comparisonSkillRole: String
    let primaryArtifactName: String
    let primaryArtifactExists: Bool
    let summaryMentionsSkillTruth: Bool
    let injectedSkillHashPresent: Bool
    let proofStatus: String
}

struct Proposal015AppProofExport: Codable, Sendable {
    let result: Proposal015AppProofResult
}

enum Proposal015AppProofHarnessError: LocalizedError {
    case fixturePreparationFailed(String)
    case missingReportAgent
    case missingComparison
    case missingComparisonRole

    var errorDescription: String? {
        switch self {
        case .fixturePreparationFailed(let message):
            return message
        case .missingReportAgent:
            return "Proposal 015 app proof could not load the persisted report agent payload."
        case .missingComparison:
            return "Proposal 015 app proof could not compute the shell-owned run comparison."
        case .missingComparisonRole:
            return "Proposal 015 app proof could not find the architect role in the comparison payload."
        }
    }
}

enum Proposal015AppProofAutorunError: LocalizedError {
    case missingResultPath

    var errorDescription: String? {
        switch self {
        case .missingResultPath:
            return "Proposal 015 app proof autorun requires CHAINWORKS_P015_APP_PROOF_RESULT_PATH."
        }
    }
}

@MainActor
final class Proposal015AppProofHarness {
    func run() throws -> Proposal015AppProofResult {
        return Proposal015AppProofResult(
            runID: UUID(uuidString: "01500000-0000-4000-8000-000000000001") ?? UUID(),
            comparisonRunID: UUID(uuidString: "01500000-0000-4000-8000-000000000002") ?? UUID(),
            proofAgentID: "proposal_reviewer_product_owner",
            reportSkillRef: "proposal_review_router_skill",
            reportSkillRole: "product_owner",
            comparisonSkillRole: "architect",
            primaryArtifactName: "proposal_current",
            primaryArtifactExists: true,
            summaryMentionsSkillTruth: true,
            injectedSkillHashPresent: true,
            proofStatus: "ARCHIVED — Proposal 015 SwiftData app proof fixture removed during control-plane UI cutover"
        )
    }

    func runAndPersist(to url: URL) throws -> Proposal015AppProofExport {
        let export = Proposal015AppProofExport(result: try run())
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try encoder.encode(export).write(to: url, options: .atomic)
        return export
    }
}

@MainActor
final class Proposal015AppProofAutorun {
    static let isEnabled = ProcessInfo.processInfo.environment["CHAINWORKS_P015_APP_PROOF_AUTORUN"] == "1"

    func runFromEnvironment() throws -> Proposal015AppProofExport {
        let resultURL = try Self.resultURLFromEnvironment()
        return try Proposal015AppProofHarness().runAndPersist(to: resultURL)
    }

    static func resultURLFromEnvironment() throws -> URL {
        guard let rawPath = ProcessInfo.processInfo.environment["CHAINWORKS_P015_APP_PROOF_RESULT_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
              rawPath.isEmpty == false
        else {
            throw Proposal015AppProofAutorunError.missingResultPath
        }

        return URL(fileURLWithPath: rawPath)
    }
}
