import Foundation

// MARK: - GeminiCLIACPTransport (Proposal 026, Phase 3 — Step 3.7)

/// ACP transport adapter for Gemini CLI (`gemini --acp` subprocess).
/// Communicates via subprocess stdin/stdout using ACP JSON-RPC protocol over ndjson framing.
///
/// Live-probed evidence (2026-04-04):
/// - Binary: `/opt/homebrew/bin/gemini`
/// - ACP launch flag: `--acp`
/// - Protocol version: 1
/// - ndjson over stdio
/// - Supports: `initialize`, `session/new`, `session/load`, `session/prompt`,
///   `session/set_mode`, `session/set_model`
/// - `session/close` is not treated as a supported Gemini ACP request; Forge terminates
///   the subprocess directly during shutdown
/// - Real `session/update` streaming with `agent_message_chunk`, `agent_thought_chunk`,
///   `tool_call`, `tool_call_update`, `session/request_permission`
/// - Real MCP server injection via `mcpServers` in `session/new`
///
/// Key differences from Claude Agent ACP:
/// - Executable: `gemini` (not `claude-agent-acp`)
/// - Requires `--acp` flag to enter ACP mode
/// - Mode catalog: `default`, `autoEdit`, `yolo`, `plan`
/// - Model catalog: `auto-gemini-3`, `gemini-3.1-pro-preview`, `gemini-2.5-pro`,
///   `gemini-2.5-flash`, `gemini-2.5-flash-lite`, etc.
/// - Authentication: Google OAuth / Gemini API key / Vertex AI / gateway
/// - Persisted session config truth is weaker than Claude (known from research)
/// - `fs/read_text_file` callback is live-proven; `fs/write_text_file` is not yet proven
/// - Usage telemetry under `_meta.quota` rather than top-level `usage`
nonisolated final class GeminiCLIACPTransport: RuntimeTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    let executablePath: String

    var mcpRuntimeNamespace: String? { "gemini_cli" }

    // MARK: - Internal State

    private var activeSessions: [String: ACPSubprocessManager] = [:]
    private var requestCounters: [String: Int] = [:]
    private var sessionSystemPrompts: [String: String] = [:]
    private var sessionEnabledExtensions: [String: [String]] = [:]
    private var sessionDiagnostics: [String: [RuntimeProviderDiagnostic]] = [:]
    private let lock = NSLock()

    // MARK: - Init

    init(executablePath: String = "gemini") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        let startTime = Date()
        let subprocess = ACPSubprocessManager(
            executablePath: executablePath,
            arguments: ["--acp"],
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
            throw RuntimeTransportError.sessionCreationFailed(reason: "No response to ACP initialize request from Gemini CLI")
        }

        // Step 2: Send `session/new` with session configuration
        var sessionParams: [String: Any] = [:]
        if let workingDirectory = request.workingDirectory {
            sessionParams["cwd"] = workingDirectory
        }
        if let model = request.model {
            sessionParams["model"] = model
        }
        sessionParams["mcpServers"] = request.mcpServers?.map { $0.acpJSONObject() } ?? ([] as [[String: Any]])

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
            throw RuntimeTransportError.sessionCreationFailed(reason: "Invalid session/new response from Gemini CLI ACP")
        }

        // Register the active session
        withLock {
            activeSessions[sessionId] = subprocess
            requestCounters[sessionId] = 2 // initialize=1, session/new=2
            if !request.systemPrompt.isEmpty {
                sessionSystemPrompts[sessionId] = request.systemPrompt
            }
            sessionDiagnostics[sessionId] = []
        }
        self.startStderrLogging(for: subprocess, prefix: "GeminiCLIACP", sessionID: sessionId)

        let startupLatency = Int(Date().timeIntervalSince(startTime) * 1000)

        // Extract enabled extensions from the session result
        var enabledExtensions: [String]?
        if let extensions = sessionResult["enabledExtensions"] as? [String] {
            enabledExtensions = extensions
        }

        withLock {
            sessionEnabledExtensions[sessionId] = enabledExtensions ?? []
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

                let subprocess = self.withLock {
                    self.activeSessions[sessionID]
                }

                guard let subprocess else {
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "No active Gemini CLI session for ID: \(sessionID)"))
                    return
                }

                // Synthesize lifecycle events (matching RuntimeTransportProtocol pattern)
                continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))

                do {
                    // LOCKED-003: System prompt embedded in prompt content
                    let systemPrompt = self.withLock {
                        self.sessionSystemPrompts[sessionID]
                    }

                    var fullContent = ""
                    if let systemPrompt, !systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        fullContent += "## System Instructions\n\(systemPrompt)\n\n---\n\n"
                    }
                    fullContent += prompt.content

                    // Append context attachments as text
                    if let attachments = prompt.context, !attachments.isEmpty {
                        fullContent += "\n\n---\n\n"
                        for attachment in attachments {
                            fullContent += "### \(attachment.name)\n"
                            if let content = attachment.content { fullContent += content }
                            if let path = attachment.path { fullContent += "Path: \(path)" }
                            fullContent += "\n\n"
                        }
                    }

                    // ACP session/prompt: prompt as array of content items
                    let promptItems: [[String: Any]] = [
                        ["type": "text", "text": fullContent]
                    ]

                    let promptRequest = self.makeJSONRPCRequest(
                        method: "session/prompt",
                        params: [
                            "sessionId": sessionID,
                            "prompt": promptItems
                        ] as [String: Any],
                        sessionID: sessionID
                    )
                    try subprocess.sendJSON(promptRequest)
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

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
                                let message = ACPProtocolSupport.formatJSONRPCError(
                                    error,
                                    fallback: "Unknown Gemini CLI ACP error"
                                )
                                continuation.yield(.error(message: message))
                            }
                            continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                            continuation.finish()
                            return
                        }

                        // Check if this is a JSON-RPC notification (has "method", no "id")
                        if let method = json["method"] as? String {
                            let params = json["params"] as? [String: Any]

                            // Handle permission requests by auto-granting
                            if method == "session/request_permission" {
                                self.autoGrantPermission(
                                    subprocess: subprocess,
                                    requestID: json["id"],
                                    params: params,
                                    sessionID: sessionID
                                )
                            }

                            // Handle file-system proxy requests
                            // Gemini CLI ACP can send fs/read_text_file requests
                            if method == "fs/read_text_file" {
                                self.handleFileRead(
                                    subprocess: subprocess,
                                    json: json,
                                    params: params
                                )
                            }

                            for event in ACPStreamEventMapper.mapNotificationEvents(method: method, params: params) {
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
        let subprocess = withLock {
            let subprocess = activeSessions.removeValue(forKey: sessionID)
            requestCounters.removeValue(forKey: sessionID)
            sessionSystemPrompts.removeValue(forKey: sessionID)
            sessionEnabledExtensions.removeValue(forKey: sessionID)
            sessionDiagnostics.removeValue(forKey: sessionID)
            return subprocess
        }

        guard let subprocess else {
            throw RuntimeTransportError.sessionCloseFailed(reason: "No active Gemini CLI session for ID: \(sessionID)")
        }

        subprocess.terminate()
    }

    func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
        let (subprocess, enabledExtensions, diagnostics) = withLock {
            (
                activeSessions[sessionID],
                sessionEnabledExtensions[sessionID] ?? [],
                sessionDiagnostics[sessionID] ?? []
            )
        }

        guard subprocess != nil else {
            throw RuntimeTransportError.streamingFailed(reason: "No active Gemini CLI session for ID: \(sessionID)")
        }
        return RuntimeSessionRuntimeState(
            enabledExtensions: enabledExtensions,
            providerDiagnostics: diagnostics
        )
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
        return withLock {
            let current = (requestCounters[key] ?? 0) + 1
            requestCounters[key] = current
            return current
        }
    }

    private func currentRequestID(for sessionID: String) -> Int {
        return withLock {
            requestCounters[sessionID] ?? 0
        }
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
                    let message = ACPProtocolSupport.formatJSONRPCError(
                        error,
                        fallback: "Unknown Gemini CLI ACP error"
                    )
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
    /// Gemini CLI ACP permission flow mirrors Claude: `session/request_permission`
    /// with options `allow_always`, `allow_once`, `reject_once`.
    private func autoGrantPermission(
        subprocess: ACPSubprocessManager,
        requestID: Any?,
        params: [String: Any]?,
        sessionID: String
    ) {
        guard let response = ACPProtocolSupport.permissionSelectionResponse(
            requestID: requestID,
            params: params
        ) else {
            ForgeLogger.execution.error("Failed to auto-grant permission for session \(sessionID)")
            return
        }
        try? subprocess.sendJSON(response)
    }

    private func startStderrLogging(for subprocess: ACPSubprocessManager, prefix: String, sessionID: String) {
        Task.detached {
            do {
                for try await line in subprocess.readStderrLines() {
                    let sanitized = ACPProtocolSupport.stripANSIEscapeCodes(from: line).trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !sanitized.isEmpty else { continue }
                    if let diagnostic = ACPProtocolSupport.geminiProviderDiagnostic(fromStderrLine: sanitized) {
                        if ACPProtocolSupport.shouldPersistProviderDiagnostic(diagnostic) {
                            self.appendDiagnostic(diagnostic, to: sessionID)
                        }
                        switch diagnostic.normalizedReason {
                        case "session_close_unsupported":
                            ForgeLogger.execution.info("\(prefix) provider warning: session/close unsupported; skipping explicit close")
                            continue
                        case "model_capacity_exhausted":
                            ForgeLogger.execution.error("\(prefix) provider error: model capacity exhausted")
                            continue
                        default:
                            break
                        }
                    }
                    if sanitized.localizedCaseInsensitiveContains("error")
                        || sanitized.localizedCaseInsensitiveContains("failed")
                        || sanitized.localizedCaseInsensitiveContains("panic") {
                        ForgeLogger.execution.error("\(prefix) stderr: \(sanitized)")
                    } else {
                        ForgeLogger.execution.info("\(prefix) stderr: \(sanitized)")
                    }
                }
            } catch {
                ForgeLogger.execution.error("\(prefix) stderr reader failed: \(error.localizedDescription)")
            }
        }
    }

    private func appendDiagnostic(_ diagnostic: RuntimeProviderDiagnostic, to sessionID: String) {
        withLock {
            var diagnostics = sessionDiagnostics[sessionID] ?? []
            diagnostics.append(diagnostic)
            if diagnostics.count > 32 {
                diagnostics.removeFirst(diagnostics.count - 32)
            }
            sessionDiagnostics[sessionID] = diagnostics
        }
    }

    // MARK: - Private: File-System Proxy

    /// Handle `fs/read_text_file` client callback requests from Gemini CLI.
    /// This is a live-proven ACP callback: Gemini CLI asks the client to read a file
    /// before edit operations.
    private func handleFileRead(
        subprocess: ACPSubprocessManager,
        json: [String: Any],
        params: [String: Any]?
    ) {
        guard let requestID = json["id"] as? Int,
              let params,
              let filePath = params["path"] as? String else {
            return
        }

        var result: [String: Any]

        if let contents = try? String(contentsOfFile: filePath, encoding: .utf8) {
            result = ["content": contents]
        } else {
            // File does not exist or is not readable — return error
            result = ["error": "ENOENT: file not found: \(filePath)"]
        }

        let response: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestID,
            "result": result
        ]

        try? subprocess.sendJSON(response)
    }

    private func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }
}

extension GeminiCLIACPTransport: RuntimeTransportTerminationControlling {
    func terminateActiveSessionsForAppShutdown() {
        let subprocesses = withLock {
            let subprocesses = Array(activeSessions.values)
            activeSessions.removeAll()
            requestCounters.removeAll()
            sessionSystemPrompts.removeAll()
            sessionEnabledExtensions.removeAll()
            sessionDiagnostics.removeAll()
            return subprocesses
        }

        for subprocess in subprocesses {
            subprocess.terminate()
        }
    }
}
