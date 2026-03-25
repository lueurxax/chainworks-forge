import Foundation
import SwiftData

enum IdeaArchiveEligibility: Equatable, Sendable {
    case archivable
    case blocked(reason: String)

    var canArchive: Bool {
        if case .archivable = self { return true }
        return false
    }

    var reason: String? {
        if case .blocked(let reason) = self { return reason }
        return nil
    }

    var explanation: String {
        switch self {
        case .archivable:
            return "Eligible"
        case .blocked(let reason):
            return reason
        }
    }
}

struct IdeaArchivePolicy {
    static func eligibility(for idea: Idea) -> IdeaArchiveEligibility {
        if idea.isArchived {
            return .blocked(reason: "Idea is already archived.")
        }

        if idea.runs.contains(where: { isActiveRun($0) }) {
            return .blocked(reason: "Wait until the active run reaches a terminal state before archiving.")
        }

        if idea.status == .draft {
            return .archivable
        }

        if idea.runs.isEmpty {
            return .blocked(reason: "Archive only after the idea reaches a terminal run or remains in draft.")
        }

        if let latestStatus = latestRun(for: idea)?.status, isTerminalStatus(latestStatus) {
            return .archivable
        }

        return .blocked(reason: "Archive only after the latest run is terminal.")
    }

    static func archiveEligibility(for idea: Idea) -> IdeaArchiveEligibility {
        eligibility(for: idea)
    }

    static func restoreEligibility(for idea: Idea) -> IdeaArchiveEligibility {
        idea.isArchived ? .archivable : .blocked(reason: "Idea is not archived.")
    }

    static func canRestore(_ idea: Idea) -> Bool {
        idea.isArchived
    }

    private static func latestRun(for idea: Idea) -> Run? {
        idea.runs.sorted { $0.startedAt > $1.startedAt }.first
    }

    private static func isActiveRun(_ run: Run) -> Bool {
        [.pending, .ready, .running, .waitingApproval, .blocked].contains(run.status)
    }

    private static func isTerminalStatus(_ status: RunStatus) -> Bool {
        [.completed, .failed, .cancelled].contains(status)
    }
}

@MainActor
final class IdeaArchiveService {
    let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    func archive(_ idea: Idea) throws {
        let eligibility = IdeaArchivePolicy.eligibility(for: idea)
        guard eligibility.canArchive else {
            throw IdeaArchiveServiceError.notEligible(eligibility.reason ?? "Idea cannot be archived right now.")
        }
        idea.archivedAt = Date()
        try modelContext.save()
    }

    func restore(_ idea: Idea) throws {
        guard IdeaArchivePolicy.canRestore(idea) else {
            throw IdeaArchiveServiceError.notArchived
        }
        idea.archivedAt = nil
        try modelContext.save()
    }
}

enum IdeaArchiveServiceError: LocalizedError, Equatable {
    case notEligible(String)
    case notArchived

    var errorDescription: String? {
        switch self {
        case .notEligible(let reason):
            return reason
        case .notArchived:
            return "Idea is not archived."
        }
    }
}
