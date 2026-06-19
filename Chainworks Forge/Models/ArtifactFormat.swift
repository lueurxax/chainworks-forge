import Foundation

/// Logical artifact rendering format. Decoupled from any persistence model — the live app
/// renders artifacts read from disk / the daemon, so this is a pure value type.
enum ArtifactFormat: String, Codable {
    case json, markdown, diff, report
}

// MARK: - ArtifactFormat.detect (§7.3 — strict priority order)

extension ArtifactFormat {
    /// Detect format. Priority: explicit extension > contract.format > fallback.
    /// `contract` is the resolved ArtifactContract from the agent catalog (if the agent has one).
    static func detect(from name: String, contract: ArtifactContract?) -> ArtifactFormat {
        // 1. File extension takes precedence
        if name.hasSuffix(".json") { return .json }
        if name.hasSuffix(".md") { return .markdown }
        if name.hasSuffix(".diff") || name.hasSuffix(".patch") { return .diff }

        // 2. If an output contract exists, use its declared format
        if let contract {
            return ArtifactFormat(rawValue: contract.machineFormat ?? contract.format) ?? .json
        }

        // 3. Fallback: treat as report (generic structured output)
        return .report
    }
}
