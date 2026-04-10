import Foundation

struct ConfiguredProvider: Identifiable, Codable, Equatable, Sendable {
    let id: UUID
    var family: ProviderFamily
    var displayName: String
    var transport: ProviderTransport
    var endpoint: String?
    var authMode: ProviderAuthMode
    var defaultModel: String?
    var capabilities: ProviderCapabilities
    var adapterVersion: String
    var isEnabled: Bool

    init(
        id: UUID = UUID(),
        family: ProviderFamily,
        displayName: String,
        transport: ProviderTransport,
        endpoint: String? = nil,
        authMode: ProviderAuthMode,
        defaultModel: String? = nil,
        capabilities: ProviderCapabilities? = nil,
        adapterVersion: String = "v1",
        isEnabled: Bool = true
    ) {
        self.id = id
        self.family = family
        self.displayName = displayName
        self.transport = transport
        self.endpoint = endpoint
        self.authMode = authMode
        self.defaultModel = defaultModel
        self.capabilities = capabilities ?? .default(for: family)
        self.adapterVersion = adapterVersion
        self.isEnabled = isEnabled
    }
}

enum ProviderDefaults {
    static func defaultModel(for family: ProviderFamily) -> String {
        switch family {
        case .codexACP:
            return "gpt-5"
        case .claudeACP:
            return "sonnet"
        case .geminiACP:
            return "gemini-2.5-pro"
        case .auggie:
            return "auggie-default"
        case .junie:
            return "junie-default"
        }
    }

    static func canonicalModel(
        _ model: String?,
        for family: ProviderFamily,
        transport: ProviderTransport? = nil
    ) -> String? {
        guard let model else { return nil }
        let trimmed = model.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        let lower = trimmed.lowercased()
        switch family {
        case .claudeACP:
            if lower.hasPrefix("claude-opus") || lower.hasPrefix("anthropic/claude-opus") || lower.hasPrefix("anthropic.claude-opus") {
                return "opus"
            }
            if lower.hasPrefix("claude-sonnet") || lower.hasPrefix("anthropic/claude-sonnet") || lower.hasPrefix("anthropic.claude-sonnet") {
                return "sonnet"
            }
        case .codexACP, .geminiACP, .auggie, .junie:
            break
        }

        return trimmed
    }

    static func generatedDisplayName(for family: ProviderFamily, transport: ProviderTransport) -> String {
        "\(family.displayName) \(transport.displayName)"
    }

    static func model(_ model: String, isCompatibleWith family: ProviderFamily) -> Bool {
        let lower = model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !lower.isEmpty else { return true }

        let expectedPrefixes: [String]
        switch family {
        case .codexACP:
            expectedPrefixes = ["gpt", "codex", "o1", "o3", "o4"]
        case .claudeACP:
            expectedPrefixes = ["claude", "anthropic", "sonnet", "opus", "default"]
        case .geminiACP:
            expectedPrefixes = ["gemini", "palm"]
        case .auggie:
            expectedPrefixes = ["auggie"]
        case .junie:
            expectedPrefixes = ["junie"]
        }

        return expectedPrefixes.contains { lower.hasPrefix($0) }
    }
}

enum ProviderFamily: String, Codable, CaseIterable, Sendable {
    case codexACP
    case claudeACP
    case geminiACP
    case auggie
    case junie

    var runtimeProviderIdentifier: String {
        switch self {
        case .codexACP:
            return "codex_acp"
        case .claudeACP:
            return "claude_acp"
        case .geminiACP:
            return "gemini_acp"
        case .auggie:
            return "auggie"
        case .junie:
            return "junie"
        }
    }

    var displayName: String {
        switch self {
        case .codexACP:
            return "Codex ACP"
        case .claudeACP:
            return "Claude ACP"
        case .geminiACP:
            return "Gemini ACP"
        case .auggie:
            return "Auggie"
        case .junie:
            return "Junie"
        }
    }

    static func from(runtimeIdentifier: String) -> ProviderFamily? {
        switch runtimeIdentifier {
        case "codex_acp", "codex-acp", "codex":
            return .codexACP
        case "claude_acp", "claude-acp", "claude", "claude_code", "claude-code":
            return .claudeACP
        case "gemini_acp", "gemini-acp", "gemini":
            return .geminiACP
        case "auggie":
            return .auggie
        case "junie":
            return .junie
        default:
            return nil
        }
    }
}

enum ProviderTransport: String, Codable, CaseIterable, Sendable {
    case cli
    case localBridge
    case httpAPI

    var displayName: String {
        switch self {
        case .cli:
            return "CLI"
        case .localBridge:
            return "Local Bridge"
        case .httpAPI:
            return "HTTP API"
        }
    }
}

enum ProviderAuthMode: String, Codable, CaseIterable, Sendable {
    case none
    case apiKey
    case sessionToken

    var displayName: String {
        switch self {
        case .none:
            return "None"
        case .apiKey:
            return "API Key"
        case .sessionToken:
            return "Session Token"
        }
    }
}

enum ProviderStatus: String, Codable, CaseIterable, Sendable {
    case unknown
    case healthy
    case degraded
    case unavailable
}

struct ProviderHealthSnapshot: Codable, Equatable, Sendable {
    let providerID: UUID
    let status: ProviderStatus
    let checkedAt: Date
    let summary: String
    let blockingIssues: [String]
}

struct ProviderCapabilities: Codable, Equatable, Sendable {
    var supportsStreaming: Bool
    var supportsTools: Bool
    var supportsStructuredOutput: Bool
    var supportsEffortControl: Bool
    var supportsSessionResume: Bool
    var supportsFileEditing: Bool
    var supportsSandboxHints: Bool
    var supportsMCPReconciliation: Bool

    static func `default`(for family: ProviderFamily) -> ProviderCapabilities {
        switch family {
        case .codexACP:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: false,
                supportsEffortControl: false,
                supportsSessionResume: true,
                supportsFileEditing: true,
                supportsSandboxHints: false,
                supportsMCPReconciliation: false
            )
        case .claudeACP:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: true,
                supportsEffortControl: true,
                supportsSessionResume: true,
                supportsFileEditing: true,
                supportsSandboxHints: true,
                supportsMCPReconciliation: true
            )
        case .geminiACP:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: true,
                supportsEffortControl: true,
                supportsSessionResume: false,
                supportsFileEditing: true,
                supportsSandboxHints: true,
                supportsMCPReconciliation: true
            )
        case .auggie, .junie:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: false,
                supportsEffortControl: false,
                supportsSessionResume: false,
                supportsFileEditing: true,
                supportsSandboxHints: false,
                supportsMCPReconciliation: false
            )
        }
    }

    /// Check whether a RuntimeProfile.requires token is satisfied by this capability set.
    func satisfies(_ token: String) -> Bool {
        switch token {
        case "streaming": return supportsStreaming
        case "tools": return supportsTools
        case "structured_output": return supportsStructuredOutput
        case "effort_control": return supportsEffortControl
        case "session_resume": return supportsSessionResume
        case "file_editing": return supportsFileEditing
        case "sandbox_hints": return supportsSandboxHints
        case "mcp_reconciliation": return supportsMCPReconciliation
        case "permission_callbacks": return supportsTools  // permission callbacks are available wherever tools are
        default: return false
        }
    }
}
