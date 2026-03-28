import Foundation
import SwiftData

@Model final class BenchmarkCohort {
    @Attribute(.unique) var id: UUID
    var createdAt: Date
    var label: String
    var status: BenchmarkCohortStatus

    private(set) var repositoryProfilesJSON: Data
    private(set) var ideaMembersJSON: Data

    @Relationship(deleteRule: .cascade)
    var pairs: [BenchmarkPair] = []

    // Computed accessor for repositoryProfiles
    var repositoryProfiles: [CohortRepositoryProfile] {
        get {
            (try? JSONDecoder().decode([CohortRepositoryProfile].self, from: repositoryProfilesJSON)) ?? []
        }
        set {
            repositoryProfilesJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    // Computed accessor for ideaMembers
    var ideaMembers: [CohortIdeaMember] {
        get {
            (try? JSONDecoder().decode([CohortIdeaMember].self, from: ideaMembersJSON)) ?? []
        }
        set {
            ideaMembersJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    init(
        id: UUID = UUID(),
        createdAt: Date = Date(),
        label: String,
        status: BenchmarkCohortStatus = .active,
        repositoryProfiles: [CohortRepositoryProfile] = [],
        ideaMembers: [CohortIdeaMember] = []
    ) {
        self.id = id
        self.createdAt = createdAt
        self.label = label
        self.status = status
        self.repositoryProfilesJSON = (try? JSONEncoder().encode(repositoryProfiles)) ?? Data()
        self.ideaMembersJSON = (try? JSONEncoder().encode(ideaMembers)) ?? Data()
    }
}

enum BenchmarkCohortStatus: String, Codable {
    case active
    case completed
    case superseded
}

struct CohortRepositoryProfile: Codable, Sendable {
    let repositoryID: String
    let profileName: String
    let profileType: String
    let description: String
}

struct CohortIdeaMember: Codable, Sendable {
    let ideaIdentifier: String
    let title: String
    let repositoryID: String
    let repositoryProfileType: String
}
