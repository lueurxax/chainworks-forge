import Foundation

// MARK: - MVPBoundaryPolicy (Proposal 008 — ARCH-080, ARCH-086)

/// Frozen MVP boundary policy. All MVP acceptance paths, UI copy, and tests
/// must reference this single canonical source of truth.
///
/// Locked decisions:
/// - ARCH-080: Canonical MVP provider set is codex, claude_code, gemini
/// - ARCH-086: MVP attachments are reference-only local-path artifacts
enum MVPBoundaryPolicy {

    // MARK: - Provider Boundary (§4.1)

    /// The canonical MVP provider families. No MVP sign-off path may depend
    /// on a provider family beyond this set.
    static let canonicalProviderFamilies: Set<String> = [
        "codex",
        "claude_code",
        "gemini",
    ]

    /// Human-readable labels for canonical providers (UI copy).
    static let providerLabels: [String: String] = [
        "codex": "Codex",
        "claude_code": "Claude Code",
        "gemini": "Gemini",
    ]

    /// Check if a provider family is within MVP boundary.
    static func isWithinMVPBoundary(_ providerFamily: String) -> Bool {
        canonicalProviderFamilies.contains(providerFamily.lowercased())
    }

    // MARK: - Attachment Policy (§6.1)

    /// Supported reference attachment file extensions for MVP.
    /// Attachments are reference-only — NOT injected into agent execution context.
    static let supportedAttachmentExtensions: Set<String> = [
        "md", "txt", "pdf", "png", "jpg", "jpeg",
        "json", "yaml", "yml", "swift", "diff", "patch",
    ]

    /// Attachment states per §6.1.
    enum AttachmentStatus: String, Codable, Sendable {
        case referenceOnly = "reference_only"
        case rejected = "rejected"
    }

    /// Validate an attachment path and return its status.
    /// Unsupported paths/extensions produce a deterministic rejection.
    static func validateAttachment(path: String) -> AttachmentStatus {
        let url = URL(fileURLWithPath: path)
        let ext = url.pathExtension.lowercased()
        guard !ext.isEmpty, supportedAttachmentExtensions.contains(ext) else {
            return .rejected
        }
        return .referenceOnly
    }

    // MARK: - Cost Granularity (§6.2)

    /// MVP cost display policy:
    /// - completed-run overview shows total run cost
    /// - completed-run export hub exposes per-stage and per-agent breakdown
    enum CostDisplayLevel: String, Sendable {
        case overview     // total only
        case breakdown    // per-stage, per-agent
    }

    // MARK: - Output/Report SLO (§6.4 — PERF-080)

    /// Active output/report retrieval SLO target.
    /// p95 <= 2.0 seconds from operator action to first rendered content.
    static let outputRetrievalSLO_p95Seconds: Double = 2.0

    // MARK: - Benchmark Cohort (§5.1)

    /// Fixed benchmark cohort size per §5.1.
    static let benchmarkCohortSize = 6
    static let ideasPerRepository = 3
    static let repositoryCount = 2
}
