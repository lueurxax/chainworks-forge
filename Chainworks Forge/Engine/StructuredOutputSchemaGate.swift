import Foundation

// MARK: - Proposal 013 Layer Q: Structured Output Schema Gate

/// Ensures backend_profiles.*.structured_output either reaches transport
/// or triggers preflight failure when unsupported.
///
/// Per Appendix B Tier 1: structured_output must not silently no-op.
struct StructuredOutputSchemaGate {

    /// Validate structured_output declarations against transport capabilities.
    /// Returns preflight check results.
    static func validate(catalog: AgentCatalog) -> [StructuredOutputGateResult] {
        var results: [StructuredOutputGateResult] = []

        for (profileID, profile) in catalog.backendProfiles {
            let requirement = StructuredOutputRequirement(rawValue: profile.structuredOutput)
                ?? .preferred

            let result = StructuredOutputGateResult(
                backendProfileID: profileID,
                provider: profile.provider,
                model: profile.model,
                declaredRequirement: profile.structuredOutput,
                parsedRequirement: requirement,
                transportSupportsStructured: transportSupportsStructuredOutput(provider: profile.provider),
                isBlocking: requirement == .required && !transportSupportsStructuredOutput(provider: profile.provider),
                explanation: buildExplanation(
                    profileID: profileID,
                    requirement: requirement,
                    supported: transportSupportsStructuredOutput(provider: profile.provider)
                )
            )
            results.append(result)
        }

        return results
    }

    /// Check if any backend profile has a blocking structured_output violation.
    static func hasBlockingViolations(catalog: AgentCatalog) -> Bool {
        validate(catalog: catalog).contains { $0.isBlocking }
    }

    /// Get blocking violations for preflight display.
    static func blockingViolations(catalog: AgentCatalog) -> [StructuredOutputGateResult] {
        validate(catalog: catalog).filter { $0.isBlocking }
    }

    // MARK: - Transport Support

    /// Check if a provider's transport supports structured output.
    /// This is the authoritative gate — if unsupported, "required" triggers preflight failure.
    private static func transportSupportsStructuredOutput(provider: String) -> Bool {
        switch provider {
        case "claude_code":
            // Claude Code supports structured output via tool use and response format
            return true
        case "codex":
            // Codex (OpenAI) supports structured output via response_format
            return true
        case "gemini":
            // Gemini supports structured output
            return true
        default:
            return false
        }
    }

    // MARK: - Explanation

    private static func buildExplanation(
        profileID: String,
        requirement: StructuredOutputRequirement,
        supported: Bool
    ) -> String {
        switch (requirement, supported) {
        case (.required, true):
            return "Profile '\(profileID)' requires structured output and transport supports it."
        case (.required, false):
            return "BLOCKING: Profile '\(profileID)' requires structured output but transport does not support it."
        case (.preferred, true):
            return "Profile '\(profileID)' prefers structured output and transport supports it."
        case (.preferred, false):
            return "Profile '\(profileID)' prefers structured output but transport does not support it. Will proceed without."
        case (.none, _):
            return "Profile '\(profileID)' does not request structured output."
        }
    }
}

// MARK: - Structured Output Requirement

enum StructuredOutputRequirement: String, Codable, Sendable {
    case required
    case preferred
    case none
}

// MARK: - Gate Result

struct StructuredOutputGateResult: Codable, Sendable {
    let backendProfileID: String
    let provider: String
    let model: String
    let declaredRequirement: String
    let parsedRequirement: StructuredOutputRequirement
    let transportSupportsStructured: Bool
    let isBlocking: Bool
    let explanation: String
}
