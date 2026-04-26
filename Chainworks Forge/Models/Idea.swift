import Foundation
import SwiftData

@Model final class Idea {
    @Attribute(.unique) var id: UUID
    var title: String
    var body: String
    var attachmentPath: String?
    /// Proposal 011 (REQ-006): operator-assigned project directory for this idea.
    var workspaceRootPath: String?
    /// Proposal 008 (REQ-006): Link idea to a benchmark cohort for sign-off tracking.
    var experimentCohortID: UUID?
    var createdAt: Date
    var archivedAt: Date?
    var status: IdeaStatus

    @Relationship(deleteRule: .cascade)
    var runs: [Run] = []

    init(
        id: UUID = UUID(),
        title: String,
        body: String,
        attachmentPath: String? = nil,
        workspaceRootPath: String? = nil,
        createdAt: Date = Date(),
        archivedAt: Date? = nil,
        status: IdeaStatus = .draft
    ) {
        self.id = id
        self.title = title
        self.body = body
        self.attachmentPath = attachmentPath
        self.workspaceRootPath = workspaceRootPath
        self.createdAt = createdAt
        self.archivedAt = archivedAt
        self.status = status
    }

    var isArchived: Bool {
        archivedAt != nil
    }

    var latestRun: Run? {
        runs.max(by: { $0.startedAt < $1.startedAt })
    }

    var hasActiveRun: Bool {
        runs.contains { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
    }

    @MainActor var lifecycleStatusLabel: String {
        if isArchived { return "Archived" }
        guard let latestRun else { return status.rawValue.capitalized }

        switch latestRun.presentationStatus {
        case .pending:
            return status == .draft ? "Draft" : "Pending"
        case .ready:
            return "Ready"
        case .running:
            return "Running"
        case .waitingApproval:
            return "Waiting Approval"
        case .blocked:
            return "Blocked"
        case .completed:
            return "Completed"
        case .failed:
            return "Failed"
        case .cancelled:
            return "Cancelled"
        case .cancelling:
            return "Cancelling"
        }
    }

    @MainActor var sidebarLifecycleStatusLabel: String {
        if isArchived { return "Archived" }
        guard let latestRun else { return status.rawValue.capitalized }

        switch latestRun.listPresentationStatus {
        case .pending:
            return status == .draft ? "Draft" : "Pending"
        case .ready:
            return "Ready"
        case .running:
            return "Running"
        case .waitingApproval:
            return "Waiting Approval"
        case .blocked:
            return "Blocked"
        case .completed:
            return "Completed"
        case .failed:
            return "Failed"
        case .cancelled:
            return "Cancelled"
        case .cancelling:
            return "Cancelling"
        }
    }

    @MainActor var latestRunIsTerminal: Bool {
        guard let latestRun else { return false }
        switch latestRun.presentationStatus {
        case .completed, .failed, .cancelled:
            return true
        default:
            return false
        }
    }

    @MainActor var archiveLifecycleStatus: String {
        lifecycleStatusLabel
    }

    /// Detail surfaces should prefer the newest still-active run over a stale
    /// previously selected run, so Ideas stays aligned with Runs Home.
    @MainActor var latestDetailRunCandidate: Run? {
        runs
            .filter {
                switch $0.presentationStatus {
                case .pending, .ready, .running, .waitingApproval, .blocked, .cancelling:
                    return true
                case .completed, .cancelled:
                    return false
                case .failed:
                    return true
                }
            }
            .sorted { $0.startedAt > $1.startedAt }
            .first
    }

    @MainActor func preferredDetailRun(selectedRun: Run?) -> Run? {
        if let latestDetailRunCandidate {
            return latestDetailRunCandidate
        }

        guard let selectedRun else { return nil }
        switch selectedRun.presentationStatus {
        case .pending, .ready, .running, .waitingApproval, .blocked, .failed, .cancelling:
            return selectedRun
        case .completed, .cancelled:
            return nil
        }
    }

    /// Keeps the persisted legacy idea status aligned with the latest run without
    /// forcing the UI to collapse terminal run truth into the old four-state enum.
    @MainActor func synchronizePersistedStatusFromRuns() {
        guard !isArchived else { return }

        guard let latestRun else {
            status = .draft
            return
        }

        switch latestRun.presentationStatus {
        case .completed:
            status = .completed
        case .failed, .cancelled:
            status = .failed
        case .pending, .ready, .running, .waitingApproval, .blocked, .cancelling:
            status = .active
        }
    }
}

enum IdeaStatus: String, Codable {
    case draft, active, completed, failed
}
