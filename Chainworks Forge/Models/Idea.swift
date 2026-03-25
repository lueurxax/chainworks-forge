import Foundation
import SwiftData

@Model final class Idea {
    @Attribute(.unique) var id: UUID
    var title: String
    var body: String
    var attachmentPath: String?
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
        createdAt: Date = Date(),
        archivedAt: Date? = nil,
        status: IdeaStatus = .draft
    ) {
        self.id = id
        self.title = title
        self.body = body
        self.attachmentPath = attachmentPath
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

    var archiveLifecycleStatus: String {
        if isArchived { return "Archived" }
        return status.rawValue.capitalized
    }
}

enum IdeaStatus: String, Codable {
    case draft, active, completed, failed
}
