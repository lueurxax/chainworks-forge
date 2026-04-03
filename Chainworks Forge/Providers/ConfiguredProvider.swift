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

    init(
        id: UUID = UUID(),
        family: ProviderFamily,
        displayName: String,
        transport: ProviderTransport,
        endpoint: String? = nil,
        authMode: ProviderAuthMode,
        defaultModel: String? = nil,
        capabilities: ProviderCapabilities? = nil,
        adapterVersion: String = "v1"
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
        case .codex, .gemini:
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
        }

        return expectedPrefixes.contains { lower.hasPrefix($0) }
    }
}

enum ProviderFamily: String, Codable, CaseIterable, Sendable {
    case codex
    case claude
    case gemini

    var runtimeProviderIdentifier: String {
        switch self {
        case .codex:
            return "codex"
        case .claude:
            return "claude_code"
        case .gemini:
            return "gemini"
        }
    }

    var displayName: String {
        rawValue.capitalized
    }

    var gooseFirstPreferred: Bool {
        switch self {
        case .codex, .claude:
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
                supportsSandboxHints: true
            )
        case .gemini:
            return ProviderCapabilities(
                supportsStreaming: true,
                supportsTools: true,
                supportsStructuredOutput: true,
                supportsEffortControl: true,
                supportsSessionResume: false,
                supportsFileEditing: false,
                supportsSandboxHints: true
            )
        }
    }
}
