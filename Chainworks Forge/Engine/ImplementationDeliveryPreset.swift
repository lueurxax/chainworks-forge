import Foundation

// MARK: - ImplementationDeliveryPreset (Proposal 007 — §3 Layer I)

/// Compiles a repo-backed executable workflow from the current catalog and workflow fixtures.
/// Provides opinionated safe defaults for the Full MVP Live dogfood preset.
///
/// This is not a replacement for RunPlanCompiler — it produces the pre-validated
/// configuration that feeds into the compiler and the delivery configuration boundary.
struct ImplementationDeliveryPreset: Sendable {

    /// A pre-configured delivery setup ready to be compiled and started.
    struct PresetConfiguration: Sendable {
        let workflowURL: URL
        let catalogURL: URL
        let deliveryConfiguration: DeliveryConfiguration
        let presetLabel: String
        let presetDescription: String
        let safetyNotes: [String]
    }

    /// All known preset IDs.
    enum PresetID: String, CaseIterable, Identifiable, Sendable {
        case fullMVPLiveDogfood = "full_mvp_live_dogfood"
        case fullMVPLiveCustom = "full_mvp_live_custom"

        var id: String { rawValue }

        var label: String {
            switch self {
            case .fullMVPLiveDogfood: return "Full MVP Live (Dogfood)"
            case .fullMVPLiveCustom: return "Full MVP Live (Custom Repo)"
            }
        }
    }

    // MARK: - Dogfood Preset

    /// Build the dogfood preset from a sample repository profile.
    /// Uses the known-good defaults for the first full-loop session.
    static func dogfoodPreset(
        profile: RepositoryProfile,
        releaseMode: ReleaseMode = .sandbox,
        workflowBundleURL: URL?,
        catalogBundleURL: URL?
    ) -> PresetConfiguration? {
        guard let workflowURL = workflowBundleURL,
              let catalogURL = catalogBundleURL else { return nil }

        let config = profile.toDeliveryConfiguration(
            releaseMode: releaseMode,
            releaseTargetLabel: releaseMode == .sandbox ? "Sandbox" : "Staging"
        )

        return PresetConfiguration(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            deliveryConfiguration: config,
            presetLabel: "Full MVP Live (Dogfood)",
            presetDescription: "First repo-backed end-to-end dogfood workflow. Idea → proposal → implementation in dedicated worktree → review quartet → manual release gate → durable receipts.",
            safetyNotes: [
                "Dedicated worktree per run (ARCH-067)",
                "Manual release gate — no autonomous release (ARCH-069)",
                "Deterministic release services only (ARCH-069)",
                "Sandbox/staging target — not production (ARCH-072)",
                "Partial failure preserves receipts (ARCH-073)"
            ]
        )
    }

    // MARK: - Custom Repo Preset

    /// Build a custom repo preset from a manually-specified delivery configuration.
    static func customPreset(
        deliveryConfiguration: DeliveryConfiguration,
        workflowBundleURL: URL?,
        catalogBundleURL: URL?
    ) -> PresetConfiguration? {
        guard let workflowURL = workflowBundleURL,
              let catalogURL = catalogBundleURL else { return nil }

        return PresetConfiguration(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            deliveryConfiguration: deliveryConfiguration,
            presetLabel: "Full MVP Live (Custom)",
            presetDescription: "Repo-backed end-to-end workflow against a custom repository target.",
            safetyNotes: [
                "Dedicated worktree per run (ARCH-067)",
                "Manual release gate (ARCH-069)",
                "Deterministic release services (ARCH-069)",
                "\(deliveryConfiguration.releaseMode.rawValue.capitalized) release target (ARCH-072)"
            ]
        )
    }

    // MARK: - Preset Summary

    /// Generate a human-readable summary block for display in the Start Run sheet.
    static func summaryBlock(for config: PresetConfiguration) -> String {
        let dc = config.deliveryConfiguration
        return """
        Workflow: \(config.presetLabel)
        Repo: \(dc.profileLabel ?? dc.repoIdentifier) → \(dc.repoRoot)
        Branch: \(dc.baseBranch) → \(dc.targetBranch)
        Release target: \(dc.releaseTargetLabel) (\(dc.releaseMode.rawValue))
        Safety: \(config.safetyNotes.first ?? "dedicated worktree, manual release gate")
        """
    }
}

// MARK: - Sample Repository Profiles (§12.1)

extension RepositoryProfile {

    /// The first dogfood target: Chainworks Forge itself.
    /// Small enough to finish in one sitting, real enough to exercise code + docs + tests + release.
    static func chainworksForge(repoRoot: String) -> RepositoryProfile {
        RepositoryProfile(
            id: "chainworks_forge_self",
            label: "Chainworks Forge (Self)",
            repoIdentifier: "Chainworks Forge",
            repoRoot: repoRoot,
            defaultBaseBranch: "main",
            defaultWorktreeBasePath: URL(fileURLWithPath: repoRoot)
                .deletingLastPathComponent()
                .appendingPathComponent(".chainworks/worktrees").path,
            defaultTargetBranch: "dogfood/full-mvp",
            defaultReleaseTargetID: "sandbox_local",
            sampleProfileID: "chainworks_forge_self"
        )
    }

    /// A minimal sample repo for safe first runs.
    static let sampleDogfood = RepositoryProfile(
        id: "sample_dogfood",
        label: "Sample Dogfood Repo",
        repoIdentifier: "sample-dogfood-repo",
        repoRoot: "",  // Must be configured at runtime
        defaultBaseBranch: "main",
        defaultWorktreeBasePath: "",  // Must be configured at runtime
        defaultTargetBranch: "dogfood/test",
        defaultReleaseTargetID: "sandbox_local",
        sampleProfileID: "sample_dogfood"
    )
}
