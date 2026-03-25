import Foundation

// MARK: - DeliveryConfiguration (Proposal 007 — ARCH-067 through ARCH-075)

/// Authoritative per-run delivery contract for repo-backed runs.
/// Frozen at createRun() time, persisted on the Run.
/// WorktreeProvisioner, RepoSafetyGuard, ReleaseOpsCoordinator, evidence export,
/// and resume all read from this frozen configuration, never from live UI state.
struct DeliveryConfiguration: Codable, Sendable {
    /// Optional repo profile ID that produced this configuration (nil for direct/manual entry).
    let profileID: String?
    let profileLabel: String?
    /// Dogfood fixture identity when applicable.
    let sampleProfileID: String?

    /// Repository identity and paths.
    let repoIdentifier: String
    let repoRoot: String
    let baseBranch: String
    let worktreeBasePath: String
    let targetBranch: String

    /// Release target identity.
    let releaseTargetID: String
    let releaseTargetLabel: String
    let releaseMode: ReleaseMode
}

// MARK: - ReleaseMode (Proposal 007 — ARCH-072)

/// Default release targets are sandbox/staging only.
/// Production is intentionally excluded from the initial dogfood slice.
enum ReleaseMode: String, Codable, Sendable {
    case sandbox
    case staging
}

// MARK: - RepositoryProfile (Proposal 007 §6.5)

/// Convenience producer of DeliveryConfiguration.
/// Resolves into the same DeliveryConfiguration contract — not a parallel truth.
struct RepositoryProfile: Codable, Sendable, Identifiable {
    let id: String
    let label: String
    let repoIdentifier: String
    let repoRoot: String
    let defaultBaseBranch: String
    let defaultWorktreeBasePath: String
    let defaultTargetBranch: String
    let defaultReleaseTargetID: String
    let sampleProfileID: String?

    /// Produce a DeliveryConfiguration draft from this profile.
    func toDeliveryConfiguration(
        releaseMode: ReleaseMode = .sandbox,
        releaseTargetLabel: String = "Sandbox"
    ) -> DeliveryConfiguration {
        DeliveryConfiguration(
            profileID: id,
            profileLabel: label,
            sampleProfileID: sampleProfileID,
            repoIdentifier: repoIdentifier,
            repoRoot: repoRoot,
            baseBranch: defaultBaseBranch,
            worktreeBasePath: defaultWorktreeBasePath,
            targetBranch: defaultTargetBranch,
            releaseTargetID: defaultReleaseTargetID,
            releaseTargetLabel: releaseTargetLabel,
            releaseMode: releaseMode
        )
    }
}
