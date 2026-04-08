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
            let runtimeProfile = profile.runtimeProfile.flatMap { catalog.runtimeProfiles[$0] }
            let transportSupportsStructured = transportSupportsStructuredOutput(
                provider: profile.provider,
                runtimeProfile: runtimeProfile
            )

            let result = StructuredOutputGateResult(
                backendProfileID: profileID,
                provider: profile.provider,
                model: profile.model,
                runtimeProfileID: profile.runtimeProfile,
                effectiveTransportKind: runtimeProfile?.transportKind,
                effectiveAdapterFamily: runtimeProfile?.adapterFamily,
                declaredRequirement: profile.structuredOutput,
                parsedRequirement: requirement,
                transportSupportsStructured: transportSupportsStructured,
                isBlocking: requirement == .required && !transportSupportsStructured,
                explanation: buildExplanation(
                    profileID: profileID,
                    requirement: requirement,
                    supported: transportSupportsStructured,
                    runtimeProfileID: profile.runtimeProfile,
                    transportKind: runtimeProfile?.transportKind
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
    private static func transportSupportsStructuredOutput(
        provider: String,
        runtimeProfile: RuntimeProfile?
    ) -> Bool {
        if let runtimeProfile {
            switch runtimeProfile.adapterFamily {
            case "claude_agent_acp", "gemini_cli_acp", "codex_acp":
                return true
            case "goose":
                return providerSupportsStructuredOutput(provider: provider)
            default:
                return runtimeProfile.transportKind == ProviderTransport.gooseServer.rawValue
                    ? providerSupportsStructuredOutput(provider: provider)
                    : false
            }
        }

        return providerSupportsStructuredOutput(provider: provider)
    }

    private static func providerSupportsStructuredOutput(provider: String) -> Bool {
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
        supported: Bool,
        runtimeProfileID: String?,
        transportKind: String?
    ) -> String {
        let runtimeSuffix: String
        if let runtimeProfileID {
            runtimeSuffix = " Effective runtime profile '\(runtimeProfileID)'" + (transportKind.map { " (\($0))" } ?? "") + "."
        } else {
            runtimeSuffix = ""
        }
        switch (requirement, supported) {
        case (.required, true):
            return "Profile '\(profileID)' requires structured output and transport supports it.\(runtimeSuffix)"
        case (.required, false):
            return "BLOCKING: Profile '\(profileID)' requires structured output but transport does not support it.\(runtimeSuffix)"
        case (.preferred, true):
            return "Profile '\(profileID)' prefers structured output and transport supports it.\(runtimeSuffix)"
        case (.preferred, false):
            return "Profile '\(profileID)' prefers structured output but transport does not support it. Will proceed without.\(runtimeSuffix)"
        case (.none, _):
            return "Profile '\(profileID)' does not request structured output.\(runtimeSuffix)"
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
    let runtimeProfileID: String?
    let effectiveTransportKind: String?
    let effectiveAdapterFamily: String?
    let declaredRequirement: String
    let parsedRequirement: StructuredOutputRequirement
    let transportSupportsStructured: Bool
    let isBlocking: Bool
    let explanation: String
}
