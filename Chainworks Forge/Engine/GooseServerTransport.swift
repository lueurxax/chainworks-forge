import Foundation

// MARK: - GooseServerTransport (Proposal 005, Section 5.3)

/// Transport adapter for the real `goosed agent` HTTP server.
///
/// Speaks the actual goosed API:
/// - `POST /agent/start` — create session with working directory
/// - `POST /agent/update_provider` — set provider/model on session (REQUIRED before first prompt)
/// - `POST /reply` — submit chat message, stream SSE
/// - `DELETE /sessions/{id}` — delete session
///
/// Key differences from the bespoke `GooseTransport`:
/// - Auth header is `X-Secret-Key`, not `Authorization: Bearer`
/// - Session creation is two HTTP calls (start + update_provider)
/// - Message format uses `ChatRequest` with `metadata.userVisible/agentVisible` (required)
/// - SSE events are `MessageEvent` JSON objects, not named event types
/// - 300-second timeout to handle cold-start latency
///
/// LOCKED-002: Single-turn execution per session.
/// LOCKED-003: System prompt is embedded in the user message.
final class GooseServerTransport: GooseTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    /// Base URL for the goosed server (e.g., https://127.0.0.1:51200).
    let baseURL: URL

    /// Secret key for X-Secret-Key header authentication.
    let secretKey: String?

    /// Provider name to configure on the session (e.g., "claude-code").
    let provider: String?

    /// Model name to configure on the session (e.g., "default").
    let model: String?

    /// URLSession configured for goosed communication.
    /// Trusts self-signed certificates for localhost (development).
    private let session: URLSession

    /// Per-session system prompts.
    /// goosed has no separate system-prompt field on /reply, so we persist the
    /// prompt at session creation time and prepend it to the user message later.
    private let systemPromptStore = GooseSystemPromptStore()

    /// Request timeout in seconds (300s for cold-start tolerance).
    let requestTimeout: TimeInterval

    // MARK: - Lifecycle

    deinit {
        session.invalidateAndCancel()
    }

    // MARK: - Init

    nonisolated init(
        baseURL: URL,
        secretKey: String? = nil,
        provider: String? = nil,
        model: String? = nil,
        requestTimeout: TimeInterval = 900
    ) {
        self.baseURL = baseURL
        self.secretKey = secretKey
        self.provider = provider
        self.model = model
        self.requestTimeout = requestTimeout

        let config = URLSessionConfiguration.default
        // Proposal 013: Increased from 300s to 900s to handle parallel agent execution
        // where the Goose server processes sessions sequentially. With 4 parallel review
        // agents each taking ~250s, sequential processing needs ~1000s total window.
        // 900s request timeout + 1800s resource timeout provides adequate margin.
        config.timeoutIntervalForRequest = requestTimeout
        config.timeoutIntervalForResource = requestTimeout * 2

        // Use a delegate that trusts self-signed certs for localhost
        let delegate = LocalhostTrustDelegate()
        self.session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
    }

    /// Testing-only initializer that accepts a custom URLSessionConfiguration.
    /// Allows URLProtocol-based mock HTTP interception for integration tests.
    nonisolated init(
        baseURL: URL,
        secretKey: String? = nil,
        provider: String? = nil,
        model: String? = nil,
        requestTimeout: TimeInterval = 300,
        sessionConfiguration: URLSessionConfiguration
    ) {
        self.baseURL = baseURL
        self.secretKey = secretKey
        self.provider = provider
        self.model = model
        self.requestTimeout = requestTimeout
        self.session = URLSession(configuration: sessionConfiguration)
    }

    // MARK: - GooseTransportProtocol

    /// Create a new session on goosed.
    /// Two-phase: POST /agent/start → POST /agent/update_provider.
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        // Phase 1: POST /agent/start
        let startURL = baseURL.appendingPathComponent("agent/start")
        var startHTTPRequest = URLRequest(url: startURL)
        startHTTPRequest.httpMethod = "POST"
        startHTTPRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        applyAuth(&startHTTPRequest)

        let workingDir = request.workingDirectory ?? FileManager.default.temporaryDirectory.path
        let startBody = GooseServerStartRequest(workingDir: workingDir)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        startHTTPRequest.httpBody = try encoder.encode(startBody)

        let (startData, startResponse) = try await session.data(for: startHTTPRequest)
        try validateHTTPResponse(startResponse, data: startData)

        // Parse session response — goosed returns a full Session object
        let startJSON = try JSONSerialization.jsonObject(with: startData) as? [String: Any]
        guard let sessionID = startJSON?["id"] as? String, !sessionID.isEmpty else {
            throw GooseTransportError.sessionCreationFailed(
                reason: "goosed /agent/start did not return a session ID"
            )
        }

        await systemPromptStore.set(request.systemPrompt, for: sessionID)

        // Phase 2: POST /agent/update_provider (REQUIRED — without this, /reply returns "Provider not set")
        let resolvedProvider = request.provider ?? provider
        let resolvedModel = request.model ?? model

        if let resolvedProvider {
            let providerURL = baseURL.appendingPathComponent("agent/update_provider")
            var providerHTTPRequest = URLRequest(url: providerURL)
            providerHTTPRequest.httpMethod = "POST"
            providerHTTPRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
            applyAuth(&providerHTTPRequest)

            let providerBody = GooseServerUpdateProvider(
                sessionID: sessionID,
                provider: resolvedProvider,
                model: resolvedModel ?? "default"
            )
            let providerEncoder = JSONEncoder()
            providerEncoder.keyEncodingStrategy = .convertToSnakeCase
            providerHTTPRequest.httpBody = try providerEncoder.encode(providerBody)

            let (providerData, providerResponse) = try await session.data(for: providerHTTPRequest)
            try validateHTTPResponse(providerResponse, data: providerData)
        }

        // Return response in our canonical format.
        // goosed does not have policy acknowledgement — we synthesize one for compatibility.
        return GooseSessionResponse(
            sessionId: sessionID,
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true,
                capabilityToken: "goose-server-session",
                backendPolicyVersion: "goosed-v1"
            )
        )
    }

    /// Submit a prompt to goosed via POST /reply and stream SSE events.
    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let url = self.baseURL.appendingPathComponent("reply")
                    var httpRequest = URLRequest(url: url)
                    httpRequest.httpMethod = "POST"
                    httpRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
                    httpRequest.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                    self.applyAuth(&httpRequest)

                    // Build the ChatRequest in goosed format
                    let sessionSystemPrompt = await self.systemPromptStore.get(for: sessionID)
                    let chatRequest = self.buildChatRequest(
                        sessionID: sessionID,
                        prompt: prompt,
                        systemPrompt: sessionSystemPrompt
                    )
                    httpRequest.httpBody = chatRequest

                    let (bytes, response) = try await self.session.bytes(for: httpRequest)
                    try self.validateHTTPResponse(response, data: nil)

                    // Emit synthetic session started event
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    var lineBuffer = ""
                    var isFirstMessage = true

                    for try await byte in bytes {
                        // Cooperative cancellation: exit cleanly if stream consumer is done
                        try Task.checkCancellation()

                        let char = Character(UnicodeScalar(byte))

                        if char == "\n" {
                            let line = lineBuffer
                            lineBuffer = ""

                            // goosed SSE: each data line is `data: {json}`
                            if line.hasPrefix("data:") {
                                let jsonStr = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                                if !jsonStr.isEmpty {
                                    if let event = GooseStreamEventMapper.map(jsonStr) {
                                        // Track first message for synthetic events
                                        if isFirstMessage {
                                            if case .textChunk = event {
                                                isFirstMessage = false
                                            } else if case .toolCallStarted = event {
                                                isFirstMessage = false
                                            }
                                        }

                                        // Check for terminal events
                                        if case .finalOutput = event {
                                            continuation.yield(event)
                                            continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                                            continuation.finish()
                                            return
                                        }
                                        if case .finish = event {
                                            continuation.yield(event)
                                            continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                                            continuation.finish()
                                            return
                                        }
                                        if case .error = event {
                                            continuation.yield(event)
                                            continuation.finish()
                                            return
                                        }

                                        continuation.yield(event)
                                    }
                                    // nil return from mapper = silently ignored (e.g., Ping)
                                }
                            }
                            // Ignore non-data lines (event:, id:, empty lines, comments)
                        } else {
                            lineBuffer.append(char)
                        }
                    }

                    // Stream ended without explicit Finish — synthesize session closed
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            // When the stream consumer finishes or is dropped, cancel the
            // underlying Task so the byte iteration and URLSession data task
            // are torn down cooperatively — preventing double-free on cleanup.
            continuation.onTermination = { @Sendable _ in
                task.cancel()
            }
        }
    }

    /// Close a session on goosed via DELETE /sessions/{id}.
    func closeSession(sessionID: String) async throws {
        let url = baseURL.appendingPathComponent("sessions/\(sessionID)")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "DELETE"
        applyAuth(&httpRequest)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)
        await systemPromptStore.remove(sessionID)
    }

    // MARK: - Private: Auth

    /// goosed uses X-Secret-Key header, NOT Authorization: Bearer.
    private func applyAuth(_ request: inout URLRequest) {
        if let secretKey {
            request.setValue(secretKey, forHTTPHeaderField: "X-Secret-Key")
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

    // MARK: - Private: ChatRequest Construction

    /// Build a goosed `ChatRequest` JSON body.
    /// IMPORTANT: `metadata.userVisible` and `metadata.agentVisible` are REQUIRED (422 without them).
    private func buildChatRequest(
        sessionID: String,
        prompt: GoosePromptRequest,
        systemPrompt: String?
    ) -> Data {
        // Combine prompt content with context attachments into the message text.
        // LOCKED-003: System prompt is embedded in the user message.
        var fullContent = ""
        if let systemPrompt, !systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            fullContent += """
            ## System Instructions
            \(systemPrompt)

            ---

            """
        }
        fullContent += prompt.content
        if let attachments = prompt.context, !attachments.isEmpty {
            fullContent += "\n\n---\n\n"
            for attachment in attachments {
                fullContent += "### \(attachment.name)\n"
                if let content = attachment.content {
                    fullContent += content
                }
                if let path = attachment.path {
                    fullContent += "Path: \(path)"
                }
                fullContent += "\n\n"
            }
        }

        // Build the ChatRequest using explicit JSON construction
        // to ensure exact camelCase field names match goosed expectations.
        let chatRequest: [String: Any] = [
            "session_id": sessionID,
            "user_message": [
                "role": "user",
                "created": Int(Date().timeIntervalSince1970),
                "content": [
                    ["type": "text", "text": fullContent]
                ],
                "metadata": [
                    "userVisible": true,
                    "agentVisible": true
                ]
            ] as [String: Any],
            "override_conversation": NSNull(),
            "recipe_name": NSNull(),
            "recipe_version": NSNull()
        ]

        return (try? JSONSerialization.data(withJSONObject: chatRequest)) ?? Data()
    }
}

private actor GooseSystemPromptStore {
    private var promptsBySessionID: [String: String] = [:]

    func set(_ prompt: String, for sessionID: String) {
        promptsBySessionID[sessionID] = prompt
    }

    func get(for sessionID: String) -> String? {
        promptsBySessionID[sessionID]
    }

    func remove(_ sessionID: String) {
        promptsBySessionID.removeValue(forKey: sessionID)
    }
}

// MARK: - GooseServerTransport Request Types

/// Request body for POST /agent/start
private struct GooseServerStartRequest: Codable {
    let workingDir: String

    enum CodingKeys: String, CodingKey {
        case workingDir = "working_dir"
    }
}

/// Request body for POST /agent/update_provider
private struct GooseServerUpdateProvider: Codable {
    let sessionID: String
    let provider: String
    let model: String

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case provider
        case model
    }
}

// MARK: - LocalhostTrustDelegate

/// URLSession delegate that trusts self-signed certificates for localhost connections.
/// goosed uses a self-signed TLS certificate by default (Section 9.6).
final class LocalhostTrustDelegate: NSObject, URLSessionDelegate, @unchecked Sendable {
    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        // Trust self-signed certs for localhost only
        if challenge.protectionSpace.host == "127.0.0.1" || challenge.protectionSpace.host == "localhost" {
            if let serverTrust = challenge.protectionSpace.serverTrust {
                let credential = URLCredential(trust: serverTrust)
                completionHandler(.useCredential, credential)
                return
            }
        }
        completionHandler(.performDefaultHandling, nil)
    }
}
