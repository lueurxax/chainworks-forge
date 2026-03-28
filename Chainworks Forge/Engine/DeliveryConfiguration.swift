import Foundation

// MARK: - DeliveryConfiguration (Proposal 007 — ARCH-067 through ARCH-075)

enum RepositoryIdentityNormalizer {
    static func canonicalIdentifier(configuredIdentifier: String?, repoRoot: String) -> String {
        let raw = configuredIdentifier?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let raw, !raw.isEmpty {
            let canonical = canonicalIdentifier(from: raw)
            if !canonical.isEmpty {
                return canonical
            }
        }

        let fallback = URL(fileURLWithPath: repoRoot).lastPathComponent
        let canonical = canonicalIdentifier(from: fallback)
        return canonical.isEmpty ? fallback : canonical
    }

    static func canonicalIdentifier(from rawValue: String) -> String {
        let trimmed = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }

        let leaf = remoteLeafCandidate(from: trimmed)
        var normalized = leaf.lowercased()
        if normalized.hasSuffix(".git") {
            normalized.removeLast(4)
        }

        normalized = normalized.replacingOccurrences(
            of: #"[^a-z0-9]+"#,
            with: "-",
            options: .regularExpression
        )
        normalized = normalized.replacingOccurrences(
            of: #"-{2,}"#,
            with: "-",
            options: .regularExpression
        )
        normalized = normalized.trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        return normalized
    }

    private static func remoteLeafCandidate(from rawValue: String) -> String {
        let sanitized = rawValue.replacingOccurrences(of: ":", with: "/")
        if let last = sanitized.split(separator: "/").last, !last.isEmpty {
            return String(last)
        }
        return rawValue
    }
}

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

    init(
        profileID: String?,
        profileLabel: String?,
        sampleProfileID: String?,
        repoIdentifier: String,
        repoRoot: String,
        baseBranch: String,
        worktreeBasePath: String,
        targetBranch: String,
        releaseTargetID: String,
        releaseTargetLabel: String,
        releaseMode: ReleaseMode
    ) {
        self.profileID = profileID
        self.profileLabel = profileLabel
        self.sampleProfileID = sampleProfileID
        self.repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: repoIdentifier,
            repoRoot: repoRoot
        )
        self.repoRoot = repoRoot
        self.baseBranch = baseBranch
        self.worktreeBasePath = worktreeBasePath
        self.targetBranch = targetBranch
        self.releaseTargetID = releaseTargetID
        self.releaseTargetLabel = releaseTargetLabel
        self.releaseMode = releaseMode
    }
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
