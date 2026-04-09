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
        case .codex:
            return "gpt-5-codex"
        case .claude:
            return "sonnet"
        case .gemini:
            return "gemini-2.5-pro"
        case .codexACP:
            return "gpt-5"
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
        case .claude:
            // Goose-backed claude-code currently responds reliably to short aliases only.
            if lower.hasPrefix("claude-opus") || lower.hasPrefix("anthropic/claude-opus") || lower.hasPrefix("anthropic.claude-opus") {
                return "opus"
            }
            if lower.hasPrefix("claude-sonnet") || lower.hasPrefix("anthropic/claude-sonnet") || lower.hasPrefix("anthropic.claude-sonnet") {
                return "sonnet"
            }
            if transport?.isGooseBacked == true, ["opus", "sonnet", "default"].contains(lower) {
                return lower
            }
        case .codex, .gemini, .codexACP, .auggie, .junie:
            break
        }

        return trimmed
    }

    static func generatedDisplayName(for family: ProviderFamily, transport: ProviderTransport) -> String {
        if family.gooseFirstPreferred && transport == .gooseServer {
            return "\(family.displayName) Goose"
        }
        return "\(family.displayName) \(transport.displayName)"
    }

    static func model(_ model: String, isCompatibleWith family: ProviderFamily) -> Bool {
        let lower = model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !lower.isEmpty else { return true }

        let expectedPrefixes: [String]
        switch family {
        case .codex:
            expectedPrefixes = ["gpt", "o1", "o3", "chatgpt"]
        case .claude:
            expectedPrefixes = ["claude", "anthropic", "sonnet", "opus", "default"]
        case .gemini:
            expectedPrefixes = ["gemini", "palm"]
        case .codexACP:
            expectedPrefixes = ["gpt", "codex", "o1", "o3", "o4"]
        case .auggie:
            expectedPrefixes = ["auggie"]
        case .junie:
            expectedPrefixes = ["junie"]
        }

        return expectedPrefixes.contains { lower.hasPrefix($0) }
    }
}

enum ProviderFamily: String, Codable, CaseIterable, Sendable {
    case codex
    case claude
    case gemini
    case codexACP
    case auggie
    case junie

    var runtimeProviderIdentifier: String {
        switch self {
        case .codex:
            return "codex"
        case .claude:
            return "claude_code"
        case .gemini:
            return "gemini"
        case .codexACP:
            return "codex_acp"
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
        case .auggie:
            return "Auggie"
        case .junie:
            return "Junie"
        default:
            return rawValue.capitalized
        }
    }

    var gooseFirstPreferred: Bool {
        switch self {
        case .codex, .claude, .codexACP, .auggie, .junie:
            return true
        case .gemini:
            return false
        }
    }

    static func from(runtimeIdentifier: String) -> ProviderFamily? {
        switch runtimeIdentifier {
        case "codex":
            return .codex
        case "claude", "claude_code", "claude-code":
            return .claude
        case "gemini":
            return .gemini
        case "codex_acp", "codex-acp":
            return .codexACP
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
    case gooseServer = "goose_server"

    var displayName: String {
        switch self {
        case .cli:
            return "CLI"
        case .localBridge:
            return "Local Bridge"
        case .httpAPI:
            return "HTTP API"
        case .gooseServer:
            return "Goose Server"
        }
    }

    var isGooseBacked: Bool {
        self == .gooseServer
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
        case .codex, .claude:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: true,
                supportsEffortControl: true,
                supportsSessionResume: true,
                supportsFileEditing: false,
                supportsSandboxHints: true,
                supportsMCPReconciliation: true
            )
        case .gemini:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: true,
                supportsEffortControl: true,
                supportsSessionResume: false,
                supportsFileEditing: false,
                supportsSandboxHints: true,
                supportsMCPReconciliation: true
            )
        case .codexACP, .auggie, .junie:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: false,
                supportsEffortControl: false,
                supportsSessionResume: family == .codexACP,
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
