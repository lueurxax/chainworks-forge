import Foundation

// MARK: - GooseTransportProtocol (Proposal 005 — LOCKED-001: transport protocol extraction)

/// Common interface for all Goose transport implementations.
/// Both `GooseTransport` (bespoke) and `GooseServerTransport` (real goosed)
/// conform to this protocol. `FixtureGooseTransport` also conforms directly.
///
/// Proposal 005: LOCKED-001 — transport protocol extraction is mandatory before
/// adding the new adapter. Keeps both transports interchangeable without if/else branching.
protocol GooseTransportProtocol: Sendable {
    /// Create a new isolated Goose session.
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse

    /// Submit a prompt to an existing session and stream SSE events.
    func submitPrompt(sessionID: String, prompt: GoosePromptRequest) -> AsyncThrowingStream<GooseStreamEvent, Error>

    /// Close a Goose session explicitly.
    func closeSession(sessionID: String) async throws
}

// MARK: - GooseTransport (HTTP/SSE client for bespoke Goose API — ARCH-028)

/// Low-level transport layer for communicating with a Goose backend over the
/// original bespoke HTTP/SSE contract (`/api/sessions`).
/// Proposal 004: locked decision — use HTTP/SSE, not ACP, for the first live slice.
/// Proposal 005: conforms to `GooseTransportProtocol`; bespoke contract retained for
/// backward compatibility. Real goosed communication uses `GooseServerTransport`.
///
/// Responsibilities:
/// - Create sessions via POST /api/sessions
/// - Submit prompts via POST /api/sessions/{id}/messages
/// - Stream SSE events
/// - Close sessions via DELETE /api/sessions/{id}
///
/// Thread-safety: `GooseTransport` is injected as an immutable dependency.
/// The class remains subclassable for test doubles.
class GooseTransport: GooseTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    /// Base URL for the Goose backend (e.g., http://localhost:3000).
    let baseURL: URL

    /// Optional API key for authentication.
    let apiKey: String?

    /// URLSession configured for SSE streaming.
    private let session: URLSession

    /// Request timeout in seconds.
    let requestTimeout: TimeInterval

    // MARK: - Init

    nonisolated init(
        baseURL: URL,
        apiKey: String? = nil,
        requestTimeout: TimeInterval = 300
    ) {
        self.baseURL = baseURL
        self.apiKey = apiKey
        self.requestTimeout = requestTimeout

        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = requestTimeout
        config.timeoutIntervalForResource = requestTimeout * 2
        self.session = URLSession(configuration: config)
    }

    // MARK: - Session Lifecycle

    /// Create a new isolated Goose session.
    /// Returns the session ID for subsequent requests.
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        let url = baseURL.appendingPathComponent("/api/sessions")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "POST"
        httpRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        applyAuth(&httpRequest)

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        httpRequest.httpBody = try encoder.encode(request)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(GooseSessionResponse.self, from: data)
    }

    /// Submit a prompt to an existing session and stream SSE events.
    /// Returns an `AsyncThrowingStream` of `GooseStreamEvent`.
    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    let url = baseURL.appendingPathComponent("/api/sessions/\(sessionID)/messages")
                    var httpRequest = URLRequest(url: url)
                    httpRequest.httpMethod = "POST"
                    httpRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
                    httpRequest.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                    self.applyAuth(&httpRequest)

                    let encoder = JSONEncoder()
                    encoder.keyEncodingStrategy = .convertToSnakeCase
                    httpRequest.httpBody = try encoder.encode(prompt)

                    let (bytes, response) = try await self.session.bytes(for: httpRequest)
                    try self.validateHTTPResponse(response, data: nil)

                    var lineBuffer = ""
                    var eventType = ""
                    var eventData = ""

                    for try await byte in bytes {
                        let char = Character(UnicodeScalar(byte))

                        if char == "\n" {
                            let line = lineBuffer
                            lineBuffer = ""

                            if line.isEmpty {
                                // Empty line = end of event
                                if !eventData.isEmpty {
                                    let event = self.parseSSEEvent(type: eventType, data: eventData)
                                    continuation.yield(event)

                                    if case .sessionClosed = event {
                                        continuation.finish()
                                        return
                                    }
                                    if case .error = event {
                                        continuation.finish()
                                        return
                                    }
                                }
                                eventType = ""
                                eventData = ""
                            } else if line.hasPrefix("event:") {
                                eventType = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
                            } else if line.hasPrefix("data:") {
                                if !eventData.isEmpty { eventData += "\n" }
                                eventData += String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                            }
                        } else {
                            lineBuffer.append(char)
                        }
                    }

                    // Stream ended
                    if !eventData.isEmpty {
                        let event = self.parseSSEEvent(type: eventType, data: eventData)
                        continuation.yield(event)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    /// Close a Goose session explicitly.
    func closeSession(sessionID: String) async throws {
        let url = baseURL.appendingPathComponent("/api/sessions/\(sessionID)")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "DELETE"
        applyAuth(&httpRequest)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)
    }

    // MARK: - Private: Auth

    private func applyAuth(_ request: inout URLRequest) {
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }
    }

    // MARK: - Private: Response Validation

    private func validateHTTPResponse(_ response: URLResponse, data: Data?) throws {
        guard let httpResponse = response as? HTTPURLResponse else {
            throw GooseTransportError.invalidResponse
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            let body = data.flatMap { String(data: $0, encoding: .utf8) }
            throw GooseTransportError.httpError(
                statusCode: httpResponse.statusCode,
                body: body
            )
        }
    }

    // MARK: - Private: SSE Parsing

    private func parseSSEEvent(type: String, data: String) -> GooseStreamEvent {
        switch type {
        case "session_started":
            return .sessionStarted(raw: data)
        case "prompt_submitted":
            return .promptSubmitted(raw: data)
        case "tool_call_started":
            if let parsed = parseToolCallData(data), let name = parsed.toolName {
                return .toolCallStarted(toolName: name, raw: data)
            }
            return .toolCallStarted(toolName: "unknown", raw: data)
        case "tool_call_finished":
            if let parsed = parseToolCallData(data), let name = parsed.toolName {
                return .toolCallFinished(toolName: name, raw: data)
            }
            return .toolCallFinished(toolName: "unknown", raw: data)
        case "text_chunk":
            return .textChunk(text: data)
        case "final_output":
            return .finalOutput(content: data)
        case "execution_failed":
            return .error(message: data)
        case "session_closed":
            return .sessionClosed(raw: data)
        default:
            return .unknown(type: type, data: data)
        }
    }

    private struct ToolCallInfo: Codable {
        let toolName: String?

        enum CodingKeys: String, CodingKey {
            case toolName = "tool_name"
        }
    }

    private func parseToolCallData(_ data: String) -> ToolCallInfo? {
        guard let jsonData = data.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(ToolCallInfo.self, from: jsonData)
    }
}

// MARK: - GooseTransport Types

/// Request to create a new Goose session.
struct GooseSessionRequest: Codable, Sendable {
    /// System prompt for the session.
    let systemPrompt: String
    /// Working directory for the session (explicit workspace).
    let workingDirectory: String?
    /// Model to use for this session.
    let model: String?
    /// Provider to use for this session.
    let provider: String?
    /// Explicit execution policy that the backend must acknowledge before live execution starts.
    let executionPolicy: GooseExecutionPolicy?
    /// Additional session metadata.
    let metadata: [String: String]?
}

/// Response from session creation.
struct GooseSessionResponse: Codable, Sendable {
    let sessionId: String
    let status: String?
    let policyAcknowledgement: GoosePolicyAcknowledgement?
}

struct GooseExecutionPolicy: Codable, Sendable {
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

struct GoosePolicyAcknowledgement: Codable, Sendable {
    let accepted: Bool
    let capabilityToken: String?
    let backendPolicyVersion: String?

    enum CodingKeys: String, CodingKey {
        case accepted
        case capabilityToken = "capability_token"
        case backendPolicyVersion = "backend_policy_version"
    }
}

/// Request to submit a prompt to a session.
struct GoosePromptRequest: Codable, Sendable {
    /// The user/task prompt.
    let content: String
    /// Optional structured context attachments.
    let context: [GooseContextAttachment]?
}

/// Context attachment for a prompt (input artifacts, workspace info).
struct GooseContextAttachment: Codable, Sendable {
    let type: String // "file", "text", "artifact"
    let name: String
    let content: String?
    let path: String?
}

// MARK: - GooseStreamEvent

/// Events received from a Goose SSE stream.
/// Proposal 004 cares about: session started, prompt submitted, tool calls,
/// text chunks, final output, errors, and session closed.
enum GooseStreamEvent: Sendable {
    case sessionStarted(raw: String)
    case promptSubmitted(raw: String)
    case toolCallStarted(toolName: String, raw: String)
    case toolCallFinished(toolName: String, raw: String)
    case textChunk(text: String)
    case finalOutput(content: String)
    case error(message: String)
    case sessionClosed(raw: String)
    case unknown(type: String, data: String)
}

// MARK: - GooseTransportError

enum GooseTransportError: Error, LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, body: String?)
    case sessionCreationFailed(reason: String)
    case streamingFailed(reason: String)
    case sessionCloseFailed(reason: String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from Goose backend"
        case .httpError(let code, let body):
            return "HTTP \(code): \(body ?? "no body")"
        case .sessionCreationFailed(let reason):
            return "Session creation failed: \(reason)"
        case .streamingFailed(let reason):
            return "Streaming failed: \(reason)"
        case .sessionCloseFailed(let reason):
            return "Session close failed: \(reason)"
        }
    }
}
