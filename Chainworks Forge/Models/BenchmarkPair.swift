import Foundation
import SwiftData

@Model final class BenchmarkPair {
    @Attribute(.unique) var id: UUID
    private(set) var ideaIdentifier: String
    private(set) var repositoryID: String
    var createdAt: Date

    @Relationship(inverse: \BenchmarkCohort.pairs)
    var cohort: BenchmarkCohort?

    @Relationship(deleteRule: .nullify)
    var manualRecord: BenchmarkExecutionRecord?

    @Relationship(deleteRule: .nullify)
    var appDrivenRecord: BenchmarkExecutionRecord?

    init(
        id: UUID = UUID(),
        ideaIdentifier: String,
        repositoryID: String,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.ideaIdentifier = ideaIdentifier
        self.repositoryID = repositoryID
        self.createdAt = createdAt
    }
}
