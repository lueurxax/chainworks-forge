import Foundation
import SwiftData

@Model final class StewardAnalysisRunLink {
    @Attribute(.unique) var id: UUID
    var analysisID: UUID
    var runID: UUID
    var role: RunRole

    @Relationship(inverse: \StewardAnalysis.analysisRunLinks)
    var analysis: StewardAnalysis?

    init(
        id: UUID = UUID(),
        analysisID: UUID,
        runID: UUID,
        role: RunRole
    ) {
        self.id = id
        self.analysisID = analysisID
        self.runID = runID
        self.role = role
    }
}

enum RunRole: String, Codable {
    case implicated
    case baseline
    case context
}
