import Foundation
import SwiftData

enum Proposal022AppProofAutorunError: LocalizedError {
    case missingResultPath

    var errorDescription: String? {
        switch self {
        case .missingResultPath:
            return "Proposal 022 app proof autorun requires CHAINWORKS_P022_APP_PROOF_RESULT_PATH."
        }
    }
}

@MainActor
final class Proposal022AppProofAutorun {
    static let isEnabled = ProcessInfo.processInfo.environment["CHAINWORKS_P022_APP_PROOF_AUTORUN"] == "1"

    private let modelContext: ModelContext
    private let executionService: ExecutionService

    init(modelContext: ModelContext, executionService: ExecutionService) {
        self.modelContext = modelContext
        self.executionService = executionService
    }

    func runFromEnvironment() async throws -> Proposal022AppProofExport {
        let resultURL = try Self.resultURLFromEnvironment()
        let harness = Proposal022AppProofHarness(
            modelContext: modelContext,
            executionService: executionService
        )
        return try await harness.runAndPersist(to: resultURL)
    }

    static func resultURLFromEnvironment() throws -> URL {
        guard let rawPath = ProcessInfo.processInfo.environment["CHAINWORKS_P022_APP_PROOF_RESULT_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
              rawPath.isEmpty == false
        else {
            throw Proposal022AppProofAutorunError.missingResultPath
        }

        return URL(fileURLWithPath: rawPath)
    }
}
