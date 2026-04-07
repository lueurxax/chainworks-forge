import Foundation
import SwiftData

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
        let prepared = Proposal015ProofFixtureBuilder.makeFixture()
        guard let fixture = prepared.fixture else {
            throw Proposal015AppProofHarnessError.fixturePreparationFailed(
                prepared.errorMessage ?? "Proposal 015 proof fixture builder returned no fixture."
            )
        }

        let reportPayload = RunReportBuilder(modelContext: fixture.modelContainer.mainContext)
            .buildReportPayload(for: fixture.proofRun, version: 1)
        guard let reportAgent = reportPayload.agentsUsed.first else {
            throw Proposal015AppProofHarnessError.missingReportAgent
        }

        let comparison = RunComparisonService(modelContext: fixture.modelContainer.mainContext)
            .compare(fixture.proofRun, fixture.comparisonRun)
        guard let comparison else {
            throw Proposal015AppProofHarnessError.missingComparison
        }

        let comparisonRole = comparison.bindingsB
            .first(where: { $0.skillRole == "architect" })?
            .skillRole
        guard let comparisonRole else {
            throw Proposal015AppProofHarnessError.missingComparisonRole
        }

        let summaryBody: String
        if let summaryArtifactID = fixture.proofRun.latestSummaryArtifactID,
           let summaryArtifact = try fixture.modelContainer.mainContext.fetch(FetchDescriptor<Artifact>())
            .first(where: { $0.id == summaryArtifactID }) {
            summaryBody = (try? SecurityScopedAccess.loadString(from: URL(fileURLWithPath: summaryArtifact.filePath))) ?? ""
        } else {
            summaryBody = ""
        }

        let injectedHashes = (try? JSONDecoder().decode(
            [String: String].self,
            from: fixture.proofRun.skillInjectedContentHashesJSON ?? Data()
        )) ?? [:]

        let passed =
            fixture.proofAgentID == "proposal_reviewer_product_owner" &&
            reportAgent.skillRef == "proposal_review_triad" &&
            reportAgent.skillRole == "product_owner" &&
            comparisonRole == "architect" &&
            fixture.primaryArtifact.name == "proposal_current" &&
            FileManager.default.fileExists(atPath: fixture.primaryArtifact.filePath) &&
            summaryBody.contains("Skill: proposal_review_triad") &&
            summaryBody.contains("Role: product_owner") &&
            injectedHashes["proposal_review_triad"]?.isEmpty == false

        return Proposal015AppProofResult(
            runID: fixture.proofRun.id,
            comparisonRunID: fixture.comparisonRun.id,
            proofAgentID: fixture.proofAgentID,
            reportSkillRef: reportAgent.skillRef ?? "missing",
            reportSkillRole: reportAgent.skillRole ?? "missing",
            comparisonSkillRole: comparisonRole,
            primaryArtifactName: fixture.primaryArtifact.name,
            primaryArtifactExists: FileManager.default.fileExists(atPath: fixture.primaryArtifact.filePath),
            summaryMentionsSkillTruth: summaryBody.contains("Skill: proposal_review_triad")
                && summaryBody.contains("Role: product_owner"),
            injectedSkillHashPresent: injectedHashes["proposal_review_triad"]?.isEmpty == false,
            proofStatus: passed
                ? "PASS — Proposal 015 app proof verified"
                : "FAIL — Proposal 015 app proof did not preserve shell-owned skill truth"
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
