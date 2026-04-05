import Foundation
import OSLog

enum GooseServerTransportDiagnosticKind: String, Sendable {
    case agentStart = "agent_start"
    case updateProvider = "update_provider"
}

struct GooseServerTransportDiagnosticEvent: Sendable {
    let kind: GooseServerTransportDiagnosticKind
    let runID: String?
    let stageID: String?
    let agentID: String?
    let workingDirectory: String?
    let sessionID: String?
    let provider: String?
    let model: String?
    let httpStatus: Int?
    let responseBodySnippet: String?
}

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

    /// Runtime-only diagnostics sink for transport lifecycle logging.
    private let diagnosticsSink: @Sendable (GooseServerTransportDiagnosticEvent) -> Void
    /// Local Goose extension registry provider used for session-scoped reconciliation.
    private let gooseExtensionRegistrySnapshotProvider: @Sendable () throws -> GooseExtensionRegistrySnapshot

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
        requestTimeout: TimeInterval = 900,
        gooseExtensionRegistrySnapshotProvider: @escaping @Sendable () throws -> GooseExtensionRegistrySnapshot = { try GooseExtensionRegistryReader().snapshot() },
        diagnosticsSink: @escaping @Sendable (GooseServerTransportDiagnosticEvent) -> Void = GooseServerTransport.logDiagnostic(_:)
    ) {
        self.baseURL = baseURL
        self.secretKey = secretKey
        self.provider = provider
        self.model = model
        self.requestTimeout = requestTimeout
        self.gooseExtensionRegistrySnapshotProvider = gooseExtensionRegistrySnapshotProvider
        self.diagnosticsSink = diagnosticsSink

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
        sessionConfiguration: URLSessionConfiguration,
        gooseExtensionRegistrySnapshotProvider: @escaping @Sendable () throws -> GooseExtensionRegistrySnapshot = { try GooseExtensionRegistryReader().snapshot() },
        diagnosticsSink: @escaping @Sendable (GooseServerTransportDiagnosticEvent) -> Void = GooseServerTransport.logDiagnostic(_:)
    ) {
        self.baseURL = baseURL
        self.secretKey = secretKey
        self.provider = provider
        self.model = model
        self.requestTimeout = requestTimeout
        self.gooseExtensionRegistrySnapshotProvider = gooseExtensionRegistrySnapshotProvider
        self.diagnosticsSink = diagnosticsSink
        self.session = URLSession(configuration: sessionConfiguration)
    }

    var mcpRuntimeNamespace: String? { "goose" }

    // MARK: - GooseTransportProtocol

    /// Create a new session on goosed.
    /// Two-phase: POST /agent/start → POST /agent/update_provider.
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        let startupStartedAt = Date()
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
        let startJSON = try? JSONSerialization.jsonObject(with: startData) as? [String: Any]
        let sessionID = startJSON?["id"] as? String
        let enabledExtensionNames = extractEnabledExtensionNames(from: startJSON)
        emitDiagnostic(
            GooseServerTransportDiagnosticEvent(
                kind: .agentStart,
                runID: request.metadata?["run_id"],
                stageID: request.metadata?["stage_id"],
                agentID: request.metadata?["agent_id"],
                workingDirectory: workingDir,
                sessionID: sessionID,
                provider: nil,
                model: nil,
                httpStatus: (startResponse as? HTTPURLResponse)?.statusCode,
                responseBodySnippet: makeResponseBodySnippet(startData)
            )
        )
        try validateHTTPResponse(startResponse, data: startData)

        // Parse session response — goosed returns a full Session object
        guard let sessionID, !sessionID.isEmpty else {
            throw GooseTransportError.sessionCreationFailed(
                reason: "goosed /agent/start did not return a session ID"
            )
        }

        await systemPromptStore.set(request.systemPrompt, for: sessionID)

        // Phase 2: POST /agent/update_provider (REQUIRED — without this, /reply returns "Provider not set")
        let resolvedProvider = request.provider ?? provider
        let resolvedModel = request.model ?? model

        if let resolvedProvider {
            let transportProvider = normalizeProviderIdentifierForGoose(resolvedProvider)
            let providerURL = baseURL.appendingPathComponent("agent/update_provider")
            var providerHTTPRequest = URLRequest(url: providerURL)
            providerHTTPRequest.httpMethod = "POST"
            providerHTTPRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
            applyAuth(&providerHTTPRequest)

            let providerBody = GooseServerUpdateProvider(
                sessionID: sessionID,
                provider: transportProvider,
                model: resolvedModel ?? "default"
            )
            let providerEncoder = JSONEncoder()
            providerEncoder.keyEncodingStrategy = .convertToSnakeCase
            providerHTTPRequest.httpBody = try providerEncoder.encode(providerBody)

            let (providerData, providerResponse) = try await session.data(for: providerHTTPRequest)
            emitDiagnostic(
                GooseServerTransportDiagnosticEvent(
                    kind: .updateProvider,
                    runID: request.metadata?["run_id"],
                    stageID: request.metadata?["stage_id"],
                    agentID: request.metadata?["agent_id"],
                    workingDirectory: workingDir,
                    sessionID: sessionID,
                    provider: transportProvider,
                    model: resolvedModel ?? "default",
                    httpStatus: (providerResponse as? HTTPURLResponse)?.statusCode,
                    responseBodySnippet: makeResponseBodySnippet(providerData)
                )
            )
            try validateHTTPResponse(providerResponse, data: providerData)
        }

        try await reconcileExtensions(
            currentExtensions: enabledExtensionNames,
            desiredExtensions: Array(Set(request.requestedExtensions ?? [])).sorted(),
            sessionID: sessionID
        )
        let actualEnabledExtensions = try await readSessionRuntimeState(sessionID: sessionID)?.enabledExtensions

        // Return response in our canonical format.
        // goosed does not have policy acknowledgement — we synthesize one for compatibility.
        return GooseSessionResponse(
            sessionId: sessionID,
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true,
                capabilityToken: "goose-server-session",
                backendPolicyVersion: "goosed-v1"
            ),
            actualEnabledExtensions: actualEnabledExtensions,
            startupLatencyMilliseconds: max(0, Int(Date().timeIntervalSince(startupStartedAt) * 1000.0))
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

    func readSessionRuntimeState(sessionID: String) async throws -> GooseSessionRuntimeState? {
        let url = baseURL.appendingPathComponent("sessions/\(sessionID)")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "GET"
        applyAuth(&httpRequest)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)

        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        return GooseSessionRuntimeState(
            enabledExtensions: extractEnabledExtensionNames(from: json).sorted()
        )
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

    private func emitDiagnostic(_ event: GooseServerTransportDiagnosticEvent) {
        diagnosticsSink(event)
    }

    private func makeResponseBodySnippet(_ data: Data) -> String? {
        guard let raw = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !raw.isEmpty else {
            return nil
        }

        if raw.count <= 240 {
            return raw
        }
        let prefix = raw.prefix(240)
        return "\(prefix)…"
    }

    private func normalizeProviderIdentifierForGoose(_ provider: String) -> String {
        switch provider {
        case "claude_code":
            return "claude-code"
        case "gemini":
            return "gemini-cli"
        default:
            return provider
        }
    }

    private func extractEnabledExtensionNames(from startJSON: [String: Any]?) -> [String] {
        guard
            let startJSON,
            let extensionData = startJSON["extension_data"] as? [String: Any],
            let enabledExtensions = extensionData["enabled_extensions.v0"] as? [String: Any],
            let extensions = enabledExtensions["extensions"] as? [[String: Any]]
        else {
            return []
        }

        var seen = Set<String>()
        var names: [String] = []
        for extensionConfig in extensions {
            guard let name = extensionConfig["name"] as? String, !name.isEmpty else { continue }
            if seen.insert(name).inserted {
                names.append(name)
            }
        }
        return names
    }

    private func removeExtension(named extensionName: String, from sessionID: String) async throws {
        let url = baseURL.appendingPathComponent("agent/remove_extension")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "POST"
        httpRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        applyAuth(&httpRequest)

        let body = GooseServerRemoveExtension(sessionID: sessionID, name: extensionName)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        httpRequest.httpBody = try encoder.encode(body)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)
    }

    private func addExtension(
        _ extensionConfig: GooseExtensionDefinition,
        to sessionID: String
    ) async throws {
        let url = baseURL.appendingPathComponent("agent/add_extension")
        var httpRequest = URLRequest(url: url)
        httpRequest.httpMethod = "POST"
        httpRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        applyAuth(&httpRequest)

        let body = GooseServerAddExtension(sessionID: sessionID, config: extensionConfig)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        httpRequest.httpBody = try encoder.encode(body)

        let (data, response) = try await session.data(for: httpRequest)
        try validateHTTPResponse(response, data: data)
    }

    private func reconcileExtensions(
        currentExtensions: [String],
        desiredExtensions: [String],
        sessionID: String
    ) async throws {
        let currentSet = Set(currentExtensions)
        let desiredSet = Set(desiredExtensions)

        for extensionName in currentSet.subtracting(desiredSet).sorted() {
            try await removeExtension(named: extensionName, from: sessionID)
        }

        guard !desiredSet.subtracting(currentSet).isEmpty else { return }
        let registry = try gooseExtensionRegistrySnapshotProvider()
        for extensionName in desiredSet.subtracting(currentSet).sorted() {
            guard let config = registry.configsByRuntimeID[extensionName] else {
                throw GooseTransportError.sessionCreationFailed(
                    reason: "Requested MCP extension '\(extensionName)' is not installed in Goose."
                )
            }
            try await addExtension(config, to: sessionID)
        }
    }

    nonisolated private static func logDiagnostic(_ event: GooseServerTransportDiagnosticEvent) {
        Logger(subsystem: "xax.Chainworks-Forge", category: "goose.transport").debug(
            """
            kind=\(event.kind.rawValue, privacy: .public) \
            run_id=\(event.runID ?? "-", privacy: .public) \
            stage_id=\(event.stageID ?? "-", privacy: .public) \
            agent_id=\(event.agentID ?? "-", privacy: .public) \
            working_dir=\(event.workingDirectory ?? "-", privacy: .public) \
            session_id=\(event.sessionID ?? "-", privacy: .public) \
            provider=\(event.provider ?? "-", privacy: .public) \
            model=\(event.model ?? "-", privacy: .public) \
            http_status=\(event.httpStatus.map(String.init) ?? "-", privacy: .public) \
            response=\(event.responseBodySnippet ?? "-", privacy: .public)
            """
        )
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

private struct GooseServerRemoveExtension: Codable {
    let sessionID: String
    let name: String

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case name
    }
}

private struct GooseServerAddExtension: Codable {
    let sessionID: String
    let config: GooseExtensionDefinition

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case config
    }
}

// MARK: - LocalhostTrustDelegate

/// URLSession delegate that trusts self-signed certificates for localhost connections.
/// goosed uses a self-signed TLS certificate by default (Section 9.6).
final class LocalhostTrustDelegate: NSObject, URLSessionDelegate, URLSessionTaskDelegate, @unchecked Sendable {
    private func handleChallenge(
        _ challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        if challenge.protectionSpace.host == "127.0.0.1" || challenge.protectionSpace.host == "localhost" {
            if let serverTrust = challenge.protectionSpace.serverTrust {
                let credential = URLCredential(trust: serverTrust)
                completionHandler(.useCredential, credential)
                return
            }
        }
        completionHandler(.performDefaultHandling, nil)
    }

    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        handleChallenge(challenge, completionHandler: completionHandler)
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        handleChallenge(challenge, completionHandler: completionHandler)
    }
}
