import Foundation
import SwiftData

@Model final class Idea {
    @Attribute(.unique) var id: UUID
    var title: String
    var body: String
    var attachmentPath: String?
    var createdAt: Date
    var status: IdeaStatus

    @Relationship(deleteRule: .cascade)
    var runs: [Run] = []

    init(id: UUID = UUID(), title: String, body: String, attachmentPath: String? = nil, createdAt: Date = Date(), status: IdeaStatus = .draft) {
        self.id = id
        self.title = title
        self.body = body
        self.attachmentPath = attachmentPath
        self.createdAt = createdAt
        self.status = status
    }
}

enum IdeaStatus: String, Codable {
    case draft, active, completed, failed
}
