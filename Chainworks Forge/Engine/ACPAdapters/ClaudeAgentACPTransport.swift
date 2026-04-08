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
    private var sessionSystemPrompts: [String: String] = [:]
    private let lock = NSLock()

    // MARK: - Init

    init(executablePath: String = "claude-agent-acp") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        ForgeLogger.claudeACP.debug("createSession called, executablePath=\(executablePath), workingDir=\(request.workingDirectory ?? "nil")")
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
        // mcpServers is required by the ACP adapter (must be an array, even if empty)
        var sessionParams: [String: Any] = [
            "mcpServers": request.mcpServers?.map { $0.acpJSONObject() } ?? ([] as [[String: Any]])
        ]
        if let workingDirectory = request.workingDirectory {
            sessionParams["cwd"] = workingDirectory
        }
        // Map model name to ACP adapter catalog: default, sonnet, haiku.
        // Evidence: claude-agent-acp model catalog does NOT include "opus" —
        // the adapter maps "default" to the best available model.
        if let model = request.model {
            sessionParams["model"] = Self.mapModelForACPCatalog(model)
        }
        // Note: requestedExtensions from Forge are Goose extension IDs — not applicable for ACP.
        // Claude Agent ACP handles MCP through its own config; client-provided MCP servers
        // can be passed via mcpServers array if needed in the future.
        // Map execution policy to ACP mode.
        // Evidence mode catalog: auto, default, acceptEdits, plan, dontAsk, bypassPermissions.
        // Write-enabled agents → bypassPermissions for autonomous execution.
        // Read-only agents → default.
        if let policy = request.executionPolicy {
            if policy.repoWritesAllowed {
                sessionParams["mode"] = "bypassPermissions"
            }
        }

        // Proposal 026: Control MCP/tool exposure through ACP _meta.claudeCode.options.
        // Disable plugins that are not managed by Forge (e.g. swift-lsp Xcode MCP bridge)
        // to prevent token waste and unwanted Xcode connections.
        // Pass only Forge-declared MCP servers; disable built-in plugins.
        sessionParams["_meta"] = [
            "claudeCode": [
                "options": [
                    "enabledPlugins": [String: Bool](),  // empty = no plugins
                    "mcpServers": [String: Any]()  // no additional MCP servers beyond what's in mcpServers[]
                ] as [String: Any]
            ] as [String: Any]
        ] as [String: Any]

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

        // Register the active session and store systemPrompt for later prepending
        lock.lock()
        activeSessions[sessionId] = subprocess
        requestCounters[sessionId] = 2 // initialize=1, session/new=2
        if !request.systemPrompt.isEmpty {
            sessionSystemPrompts[sessionId] = request.systemPrompt
        }
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

                // Synthesize session lifecycle events (matches GooseServerTransport behavior)
                continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))

                do {
                    // ACP session/prompt expects prompt as an array of content items:
                    // [{"type": "text", "text": "..."}]
                    // LOCKED-003: System prompt is embedded in the prompt content (same as GooseServerTransport).
                    self.lock.lock()
                    let systemPrompt = self.sessionSystemPrompts[sessionID]
                    self.lock.unlock()

                    var fullContent = ""
                    if let systemPrompt, !systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        fullContent += "## System Instructions\n\(systemPrompt)\n\n---\n\n"
                    }
                    fullContent += prompt.content

                    var promptItems: [[String: Any]] = [
                        ["type": "text", "text": fullContent]
                    ]
                    if let attachments = prompt.context, !attachments.isEmpty {
                        var attachmentText = "---\n\n"
                        for attachment in attachments {
                            attachmentText += "### \(attachment.name)\n"
                            if let content = attachment.content {
                                attachmentText += content
                            }
                            if let path = attachment.path {
                                attachmentText += "Path: \(path)"
                            }
                            attachmentText += "\n\n"
                        }
                        promptItems.append(["type": "text", "text": attachmentText])
                    }

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
                            ForgeLogger.claudeACP.info("Non-JSON line from subprocess: \(line.prefix(200))")
                            continue
                        }

                        ForgeLogger.claudeACP.debug("Received JSON-RPC: method=\(json["method"] ?? "nil"), id=\(json["id"] ?? "nil"), hasResult=\(json["result"] != nil)")

                        // Check if this is a JSON-RPC response (has "id" and "result" or "error")
                        // ACP may send id as Int or String — handle both.
                        let responseID: Int?
                        if let intID = json["id"] as? Int {
                            responseID = intID
                        } else if let strID = json["id"] as? String, let parsed = Int(strID) {
                            responseID = parsed
                        } else {
                            responseID = nil
                        }

                        if let responseID, responseID == promptRequestID {
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
                            // Log first few events to diagnose structure
                            if method == "session/update" {
                                let updateKeys = (params?["update"] as? [String: Any])?.keys.sorted().joined(separator: ",") ?? "no-update"
                                let sessionUpdate = (params?["update"] as? [String: Any])?["sessionUpdate"] as? String ?? "no-sessionUpdate"
                                ForgeLogger.claudeACP.debug("session/update: sessionUpdate=\(sessionUpdate) updateKeys=\(updateKeys)")
                            }

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
        sessionSystemPrompts.removeValue(forKey: sessionID)
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

    // MARK: - Private: Model Mapping

    /// Map Forge model identifiers to the claude-agent-acp catalog.
    /// Live-verified model catalog (2026-04-07): `default`, `opus`, `sonnet`, `haiku`.
    private static func mapModelForACPCatalog(_ model: String) -> String {
        let lowered = model.lowercased()
        switch lowered {
        case "opus", "sonnet", "haiku", "default":
            return lowered
        default:
            if lowered.contains("opus") { return "opus" }
            if lowered.contains("haiku") { return "haiku" }
            if lowered.contains("sonnet") { return "sonnet" }
            return "default"
        }
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
