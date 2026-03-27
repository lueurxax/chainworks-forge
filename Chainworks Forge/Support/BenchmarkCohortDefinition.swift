import Foundation

// MARK: - BenchmarkCohortDefinition (Proposal 008 — ARCH-082)

/// Defines the fixed idea/repository set used for manual-vs-app comparison.
///
/// Per §5.1: One fixed benchmark cohort:
/// - Repository A: controlled sample repo from Proposal 007
/// - Repository B: messy real-world repo
/// - 6 total ideas (3 per repo), each executed twice (manual + app-driven)
struct BenchmarkCohortDefinition: Codable, Sendable {

    /// Repository profile for one benchmark repository.
    struct RepositoryProfile: Codable, Sendable, Identifiable {
        let id: String                          // unique repo identifier
        let label: String                       // human-readable name
        let repoRoot: String                    // absolute path to repo root
        let profileType: RepositoryProfileType  // controlled vs real-world
        let description: String
    }

    /// Repository classification per §5.1.
    enum RepositoryProfileType: String, Codable, Sendable {
        case controlledSample = "controlled_sample"
        case realWorld = "real_world"
    }

    /// Idea definition within the cohort.
    struct IdeaSpec: Codable, Sendable, Identifiable {
        let id: String                          // stable idea identifier within cohort
        let title: String
        let body: String
        let repositoryID: String                // which repo this idea targets
        let attachmentPath: String?             // optional reference attachment
        let benchmarkRole: String               // e.g. "feature_addition", "bug_fix", "refactor"
    }

    // MARK: - Properties

    let cohortID: UUID
    let label: String
    let createdAt: Date
    let repositories: [RepositoryProfile]
    let ideas: [IdeaSpec]

    // MARK: - Validation

    /// Validate the cohort matches the §5.1 contract.
    var validationErrors: [String] {
        var errors: [String] = []
        if repositories.count != MVPBoundaryPolicy.repositoryCount {
            errors.append("Expected \(MVPBoundaryPolicy.repositoryCount) repositories, got \(repositories.count)")
        }
        if ideas.count != MVPBoundaryPolicy.benchmarkCohortSize {
            errors.append("Expected \(MVPBoundaryPolicy.benchmarkCohortSize) ideas, got \(ideas.count)")
        }
        let controlledSample = repositories.filter { $0.profileType == .controlledSample }
        if controlledSample.isEmpty {
            errors.append("At least one controlled-sample repository is required")
        }
        let realWorld = repositories.filter { $0.profileType == .realWorld }
        if realWorld.isEmpty {
            errors.append("At least one real-world repository is required")
        }
        for repo in repositories {
            let repoIdeas = ideas.filter { $0.repositoryID == repo.id }
            if repoIdeas.count != MVPBoundaryPolicy.ideasPerRepository {
                errors.append("Repository '\(repo.label)' has \(repoIdeas.count) ideas, expected \(MVPBoundaryPolicy.ideasPerRepository)")
            }
        }
        return errors
    }

    var isValid: Bool { validationErrors.isEmpty }
}
