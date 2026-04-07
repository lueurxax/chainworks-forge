import Foundation

// MARK: - ClaudeAgentACPTransport (Proposal 026, Phase 3 — Step 3.6)

/// ACP transport adapter for Claude Agent (`claude-agent-acp` subprocess).
/// Communicates via subprocess stdin/stdout using ACP JSON-RPC protocol over ndjson framing.
///
/// Live-probed evidence (2026-04-04):
/// - Binary: `/opt/homebrew/bin/claude-agent-acp`
/// - Protocol version: 1
/// - ndjson over stdio
/// - Supports: `initialize`, `session/new`, `session/load`, `session/prompt`,
///   `session/set_mode`, `session/set_model`, `session/close`
/// - Real `session/update` streaming with `agent_message_chunk`, `tool_call`,
///   `tool_call_update`, `usage_update`, `session/request_permission`
/// - Real MCP server injection via `mcpServers` in `session/new`
///
/// Mode catalog: `auto`, `default`, `acceptEdits`, `plan`, `dontAsk`, `bypassPermissions`
/// Model catalog: `default`, `sonnet`, `haiku`
final class ClaudeAgentACPTransport: RuntimeTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    let executablePath: String

    var mcpRuntimeNamespace: String? { "claude_agent" }

    // MARK: - Internal State

    private var activeSessions: [String: ACPSubprocessManager] = [:]
    private var requestCounters: [String: Int] = [:]
    private let lock = NSLock()

    // MARK: - Init

    init(executablePath: String = "claude-agent-acp") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        let startTime = Date()
        let subprocess = ACPSubprocessManager(
            executablePath: executablePath,
            arguments: [],
            environment: [:],
            workingDirectory: request.workingDirectory
        )

        try subprocess.launch()

        // Step 1: Send `initialize` JSON-RPC request
        let initializeRequest = makeJSONRPCRequest(
            method: "initialize",
            params: [
                "protocolVersion": 1,
                "clientInfo": [
                    "name": "chainworks-forge",
                    "version": "1.0"
                ] as [String: Any]
            ] as [String: Any],
            sessionID: nil
        )
        try subprocess.sendJSON(initializeRequest)

        // Read initialize response
        let initResponse = try await readNextResult(from: subprocess)
        guard initResponse != nil else {
            subprocess.terminate()
            throw RuntimeTransportError.sessionCreationFailed(reason: "No response to ACP initialize request")
        }

        // Step 2: Send `session/new` with session configuration
        var sessionParams: [String: Any] = [:]
        if let workingDirectory = request.workingDirectory {
            sessionParams["cwd"] = workingDirectory
        }
        if let model = request.model {
            sessionParams["model"] = model
        }
        if let extensions = request.requestedExtensions, !extensions.isEmpty {
            // Map extensions to ACP mcpServers format if needed
            sessionParams["requestedExtensions"] = extensions
        }
        if let systemPrompt = request.systemPrompt as String?, !systemPrompt.isEmpty {
            sessionParams["systemPrompt"] = systemPrompt
        }

        let sessionNewRequest = makeJSONRPCRequest(
            method: "session/new",
            params: sessionParams,
            sessionID: nil
        )
        try subprocess.sendJSON(sessionNewRequest)

        // Read session/new response
        guard let sessionResult = try await readNextResult(from: subprocess),
              let sessionId = sessionResult["sessionId"] as? String else {
            subprocess.terminate()
            throw RuntimeTransportError.sessionCreationFailed(reason: "Invalid session/new response from Claude Agent ACP")
        }

        // Register the active session
        lock.lock()
        activeSessions[sessionId] = subprocess
        requestCounters[sessionId] = 2 // initialize=1, session/new=2
        lock.unlock()

        let startupLatency = Int(Date().timeIntervalSince(startTime) * 1000)

        // Extract enabled extensions from the session result
        var enabledExtensions: [String]?
        if let extensions = sessionResult["enabledExtensions"] as? [String] {
            enabledExtensions = extensions
        }

        return RuntimeSessionResponse(
            sessionId: sessionId,
            status: "active",
            policyAcknowledgement: RuntimePolicyAcknowledgement(
                accepted: true,
                capabilityToken: nil,
                backendPolicyVersion: nil
            ),
            actualEnabledExtensions: enabledExtensions,
            startupLatencyMilliseconds: startupLatency
        )
    }

    func submitPrompt(
        sessionID: String,
        prompt: RuntimePromptRequest
    ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task { [weak self] in
                guard let self else {
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "Transport deallocated"))
                    return
                }

                self.lock.lock()
                let subprocess = self.activeSessions[sessionID]
                self.lock.unlock()

                guard let subprocess else {
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "No active session for ID: \(sessionID)"))
                    return
                }

                do {
                    // Build prompt content in ACP format
                    var promptContent: [[String: Any]] = [
                        ["type": "text", "text": prompt.content]
                    ]

                    // Append context attachments if present
                    if let attachments = prompt.context {
                        for attachment in attachments {
                            var item: [String: Any] = ["type": attachment.type, "name": attachment.name]
                            if let content = attachment.content {
                                item["content"] = content
                            }
                            if let path = attachment.path {
                                item["path"] = path
                            }
                            promptContent.append(item)
                        }
                    }

                    let promptRequest = self.makeJSONRPCRequest(
                        method: "session/prompt",
                        params: [
                            "sessionId": sessionID,
                            "prompt": promptContent
                        ] as [String: Any],
                        sessionID: sessionID
                    )
                    try subprocess.sendJSON(promptRequest)

                    let promptRequestID = self.currentRequestID(for: sessionID)

                    // Read streaming events from stdout
                    for try await line in subprocess.readLines() {
                        try Task.checkCancellation()

                        guard let lineData = line.data(using: .utf8),
                              let json = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any] else {
                            continue
                        }

                        // Check if this is a JSON-RPC response (has "id" and "result")
                        if let responseID = json["id"] as? Int, responseID == promptRequestID {
                            // This is the final prompt result
                            if let result = json["result"] as? [String: Any] {
                                let finishEvent = ACPStreamEventMapper.mapPromptResult(result)
                                continuation.yield(finishEvent)
                            } else if let error = json["error"] as? [String: Any] {
                                let message = error["message"] as? String ?? "Unknown ACP error"
                                continuation.yield(.error(message: message))
                            }
                            continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                            continuation.finish()
                            return
                        }

                        // Check if this is a JSON-RPC notification (has "method", no "id")
                        if let method = json["method"] as? String {
                            let params = json["params"] as? [String: Any]

                            // Handle permission requests by auto-granting (for now)
                            if method == "session/request_permission" {
                                self.autoGrantPermission(
                                    subprocess: subprocess,
                                    params: params,
                                    sessionID: sessionID
                                )
                            }

                            if let event = ACPStreamEventMapper.mapNotification(method: method, params: params) {
                                continuation.yield(event)
                            }
                        }
                    }

                    // Stream ended without explicit finish
                    continuation.finish()

                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { @Sendable _ in
                task.cancel()
            }
        }
    }

    func closeSession(sessionID: String) async throws {
        lock.lock()
        let subprocess = activeSessions.removeValue(forKey: sessionID)
        requestCounters.removeValue(forKey: sessionID)
        lock.unlock()

        guard let subprocess else {
            throw RuntimeTransportError.sessionCloseFailed(reason: "No active session for ID: \(sessionID)")
        }

        // Send session/close request before terminating
        let closeRequest = makeJSONRPCRequest(
            method: "session/close",
            params: ["sessionId": sessionID],
            sessionID: nil
        )
        try? subprocess.sendJSON(closeRequest)

        // Brief wait for clean shutdown, then terminate
        try? await Task.sleep(for: .milliseconds(200))
        subprocess.terminate()
    }

    // MARK: - Private: JSON-RPC Request Construction

    /// Build a JSON-RPC 2.0 request dictionary with an auto-incrementing ID per session.
    private func makeJSONRPCRequest(
        method: String,
        params: [String: Any],
        sessionID: String?
    ) -> [String: Any] {
        let id = nextRequestID(for: sessionID)
        var request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": id,
            "method": method
        ]
        if !params.isEmpty {
            request["params"] = params
        }
        return request
    }

    private func nextRequestID(for sessionID: String?) -> Int {
        let key = sessionID ?? "__global__"
        lock.lock()
        let current = (requestCounters[key] ?? 0) + 1
        requestCounters[key] = current
        lock.unlock()
        return current
    }

    private func currentRequestID(for sessionID: String) -> Int {
        lock.lock()
        let current = requestCounters[sessionID] ?? 0
        lock.unlock()
        return current
    }

    // MARK: - Private: Read Next JSON-RPC Result

    /// Read lines from the subprocess until a JSON-RPC response (with "result") is found.
    /// Notifications encountered along the way are silently discarded during handshake.
    private func readNextResult(from subprocess: ACPSubprocessManager) async throws -> [String: Any]? {
        for try await line in subprocess.readLines() {
            guard let lineData = line.data(using: .utf8),
                  let json = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any] else {
                continue
            }

            // JSON-RPC response has "id" field
            if json["id"] != nil {
                if let result = json["result"] as? [String: Any] {
                    return result
                }
                if let error = json["error"] as? [String: Any] {
                    let message = error["message"] as? String ?? "Unknown ACP error"
                    throw RuntimeTransportError.sessionCreationFailed(reason: message)
                }
                return nil
            }

            // Skip notifications during handshake
        }
        return nil
    }

    // MARK: - Private: Permission Handling

    /// Auto-grant permission requests during execution.
    /// Based on observed ACP permission flow: the adapter sends `session/request_permission`
    /// with options like `allow_always`, `allow_once`, `reject_once`.
    /// For now, Forge auto-grants with `allow_once` based on the execution policy.
    private func autoGrantPermission(
        subprocess: ACPSubprocessManager,
        params: [String: Any]?,
        sessionID: String
    ) {
        guard let params,
              let requestId = params["id"] as? String ?? params["requestId"] as? String else {
            return
        }

        let response: [String: Any] = [
            "jsonrpc": "2.0",
            "method": "session/permission_response",
            "params": [
                "sessionId": sessionID,
                "requestId": requestId,
                "response": "allow_once"
            ] as [String: Any]
        ]

        try? subprocess.sendJSON(response)
    }
}
