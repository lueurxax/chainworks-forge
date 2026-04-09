import Foundation

// MARK: - RuntimeTransportProtocol (Proposal 026 — ACP-shaped canonical runtime transport)

/// Canonical runtime transport contract. ACP-shaped vocabulary for session lifecycle,
/// prompt submission, and stream events. All runtime adapters (Goose, Claude Agent ACP,
/// Gemini CLI ACP) implement this protocol.
protocol RuntimeTransportProtocol: Sendable {
    /// Transport-owned runtime namespace used to resolve session-scoped MCP mappings.
    /// Keeps MCP policy independent from frozen provider bindings in fixture/proof flows.
    var mcpRuntimeNamespace: String? { get }

    /// Create a new isolated runtime session.
    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse

    /// Submit a prompt to an existing session and stream events.
    func submitPrompt(sessionID: String, prompt: RuntimePromptRequest) -> AsyncThrowingStream<RuntimeStreamEvent, Error>

    /// Close a runtime session explicitly.
    func closeSession(sessionID: String) async throws

    /// Read settled runtime state for an existing session when the transport supports it.
    func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState?
}

extension RuntimeTransportProtocol {
    var mcpRuntimeNamespace: String? { nil }

    func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
        nil
    }
}

// MARK: - RuntimeSessionRequest

/// Request to create a new runtime session.
struct RuntimeSessionRequest: Codable, Sendable {
    /// System prompt for the session.
    let systemPrompt: String
    /// Working directory for the session (explicit workspace).
    let workingDirectory: String?
    /// Model to use for this session.
    let model: String?
    /// Provider to use for this session.
    let provider: String?
    /// Explicit execution policy that the backend must acknowledge before live execution starts.
    let executionPolicy: RuntimeExecutionPolicy?
    /// Additional session metadata.
    let metadata: [String: String]?
    /// Requested session-scoped MCP / extension set for runtimes that support reconciliation.
    let requestedExtensions: [String]?
    /// Concrete ACP-native MCP server definitions for runtimes that accept per-session server injection.
    let mcpServers: [RuntimeMCPServerDefinition]?

    init(
        systemPrompt: String,
        workingDirectory: String?,
        model: String?,
        provider: String?,
        executionPolicy: RuntimeExecutionPolicy?,
        metadata: [String: String]?,
        requestedExtensions: [String]? = nil,
        mcpServers: [RuntimeMCPServerDefinition]? = nil
    ) {
        self.systemPrompt = systemPrompt
        self.workingDirectory = workingDirectory
        self.model = model
        self.provider = provider
        self.executionPolicy = executionPolicy
        self.metadata = metadata
        self.requestedExtensions = requestedExtensions
        self.mcpServers = mcpServers
    }
}

// MARK: - RuntimeMCPServerDefinition

/// Machine-local MCP server definition materialized for ACP `session/new`.
/// Repo YAML never owns these launch details; the bridge derives them from the
/// local extension registry after MCP policy resolution selects logical server IDs.
struct RuntimeMCPServerDefinition: Codable, Equatable, Sendable {
    let name: String
    let type: String?
    let command: String?
    let args: [String]
    let env: [RuntimeNameValue]
    let url: String?
    let headers: [RuntimeNameValue]

    init(
        name: String,
        type: String? = nil,
        command: String? = nil,
        args: [String] = [],
        env: [RuntimeNameValue] = [],
        url: String? = nil,
        headers: [RuntimeNameValue] = []
    ) {
        self.name = name
        self.type = type
        self.command = command
        self.args = args
        self.env = env
        self.url = url
        self.headers = headers
    }

    func acpJSONObject() -> [String: Any] {
        var object: [String: Any] = [
            "name": name
        ]
        if let type, !type.isEmpty {
            object["type"] = type
        }
        if let command, !command.isEmpty {
            object["command"] = command
            object["args"] = args
            object["env"] = env.map(\.jsonObject)
        }
        if let url, !url.isEmpty {
            object["url"] = url
            if !headers.isEmpty {
                object["headers"] = headers.map(\.jsonObject)
            }
        }
        return object
    }
}

struct RuntimeNameValue: Codable, Equatable, Sendable {
    let name: String
    let value: String

    var jsonObject: [String: String] {
        [
            "name": name,
            "value": value
        ]
    }
}

// MARK: - RuntimeSessionResponse

/// Response from session creation.
struct RuntimeSessionResponse: Codable, Sendable {
    let sessionId: String
    let status: String?
    let policyAcknowledgement: RuntimePolicyAcknowledgement?
    let actualEnabledExtensions: [String]?
    let startupLatencyMilliseconds: Int?

    init(
        sessionId: String,
        status: String?,
        policyAcknowledgement: RuntimePolicyAcknowledgement?,
        actualEnabledExtensions: [String]? = nil,
        startupLatencyMilliseconds: Int? = nil
    ) {
        self.sessionId = sessionId
        self.status = status
        self.policyAcknowledgement = policyAcknowledgement
        self.actualEnabledExtensions = actualEnabledExtensions
        self.startupLatencyMilliseconds = startupLatencyMilliseconds
    }
}

// MARK: - RuntimeSessionRuntimeState

struct RuntimeSessionRuntimeState: Codable, Sendable {
    let enabledExtensions: [String]
}

// MARK: - RuntimeExecutionPolicy

struct RuntimeExecutionPolicy: Codable, Sendable {
    let permissionProfileID: String
    let workspaceMode: String
    let gitOperationsAllowed: Bool
    let releaseOperationsAllowed: Bool
    let repoWritesAllowed: Bool

    enum CodingKeys: String, CodingKey {
        case workspaceMode = "workspace_mode"
        case permissionProfileID = "permission_profile_id"
        case gitOperationsAllowed = "git_operations_allowed"
        case releaseOperationsAllowed = "release_operations_allowed"
        case repoWritesAllowed = "repo_writes_allowed"
    }
}

// MARK: - RuntimePolicyAcknowledgement

struct RuntimePolicyAcknowledgement: Codable, Sendable {
    let accepted: Bool
    let capabilityToken: String?
    let backendPolicyVersion: String?

    enum CodingKeys: String, CodingKey {
        case accepted
        case capabilityToken = "capability_token"
        case backendPolicyVersion = "backend_policy_version"
    }
}

// MARK: - RuntimePromptRequest

/// Request to submit a prompt to a session.
struct RuntimePromptRequest: Codable, Sendable {
    /// The user/task prompt.
    let content: String
    /// Optional structured context attachments.
    let context: [RuntimeContextAttachment]?
}

// MARK: - RuntimeContextAttachment

/// Context attachment for a prompt (input artifacts, workspace info).
struct RuntimeContextAttachment: Codable, Sendable {
    let type: String // "file", "text", "artifact"
    let name: String
    let content: String?
    let path: String?
}

// MARK: - RuntimeStreamEvent

/// Events received from a runtime SSE stream.
/// Canonical event set for all runtime adapters: session started, prompt submitted,
/// tool calls, text chunks, final output, errors, and session closed.
enum RuntimeStreamEvent: Sendable {
    case sessionStarted(raw: String)
    case promptSubmitted(raw: String)
    case toolCallStarted(toolName: String, raw: String)
    case toolCallFinished(toolName: String, raw: String)
    case textChunk(text: String)
    case finalOutput(content: String)
    case finish(reason: String, totalTokens: Int?, raw: String)
    case error(message: String)
    case sessionClosed(raw: String)
    case unknown(type: String, data: String)
}

// MARK: - RuntimeExtensionRegistryProvider (Proposal 026 Phase 2)

/// Abstracts MCP / extension registry access so core runtime code
/// does not depend on the concrete Goose config reader.
protocol RuntimeExtensionRegistryProvider: Sendable {
    func registrySnapshot() throws -> RuntimeExtensionRegistrySnapshot
}

// MARK: - RuntimeTransportError

enum RuntimeTransportError: Error, LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, body: String?)
    case sessionCreationFailed(reason: String)
    case streamingFailed(reason: String)
    case sessionCloseFailed(reason: String)
    case unknownAdapterFamily(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from runtime backend"
        case .httpError(let code, let body):
            return "HTTP \(code): \(body ?? "no body")"
        case .sessionCreationFailed(let reason):
            return "Session creation failed: \(reason)"
        case .streamingFailed(let reason):
            return "Streaming failed: \(reason)"
        case .sessionCloseFailed(let reason):
            return "Session close failed: \(reason)"
        case .unknownAdapterFamily(let family):
            return "No registered transport adapter for runtime family '\(family)'. Register the adapter before adding its runtime profile to the catalog."
        }
    }
}

// MARK: - RuntimeTransportFactory (Proposal 026 — per-agent transport resolution)

/// Resolves the correct transport for each agent based on its runtime profile.
/// Transports are cached by adapter family — max one instance per family per run.
protocol RuntimeTransportFactory: Sendable {
    func transport(for agent: ResolvedAgent, binding: ResolvedProviderBinding?) throws -> any RuntimeTransportProtocol
}

/// Optional lifecycle hook for factories that cache live runtime transports and need
/// explicit app-termination cleanup beyond normal per-session settlement.
protocol RuntimeTransportFactoryTerminationControlling: Sendable {
    func terminateActiveTransportsForAppShutdown()
}

/// Optional lifecycle hook for transports that own live runtime sessions/processes and
/// need immediate teardown during app termination.
protocol RuntimeTransportTerminationControlling: Sendable {
    func terminateActiveSessionsForAppShutdown()
}

/// Trivial factory wrapping a single transport — backward compatibility for tests
/// and runs where all agents share one transport.
struct SingleTransportFactory: RuntimeTransportFactory {
    let transport: any RuntimeTransportProtocol
    func transport(for agent: ResolvedAgent, binding: ResolvedProviderBinding?) throws -> any RuntimeTransportProtocol {
        transport
    }
}
