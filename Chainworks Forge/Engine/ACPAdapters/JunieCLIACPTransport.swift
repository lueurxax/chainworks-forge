import Foundation

// MARK: - JunieCLIACPTransport (Proposal 029 — Full Implementation)

/// ACP transport adapter for Junie CLI (`junie` subprocess).
/// Communicates via subprocess stdin/stdout using ACP JSON-RPC protocol over ndjson framing.
///
/// Executable: `junie`
/// Model catalog: pass through as-is (Junie uses its own model catalog).
/// Mode catalog: `autonomous` (repoWritesAllowed), `interactive` (default).
/// Session params: ACP JSON-RPC with `session/new` using `cwd`, `model`, `mode`. No `_meta` block.
final class JunieCLIACPTransport: RuntimeTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    let executablePath: String

    var mcpRuntimeNamespace: String? { "junie" }

    // MARK: - Internal State

    private var activeSessions: [String: ACPSubprocessManager] = [:]
    private var requestCounters: [String: Int] = [:]
    private var sessionSystemPrompts: [String: String] = [:]
    private var sessionEnabledExtensions: [String: [String]] = [:]
    private let lock = NSLock()

    // MARK: - Init

    init(executablePath: String = "junie") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        ForgeLogger.execution.debug("JunieCLIACPTransport.createSession called, executablePath=\(executablePath), workingDir=\(request.workingDirectory ?? "nil")")
        let startTime = Date()
        let subprocess = ACPSubprocessManager(
            executablePath: executablePath,
            arguments: [],
            environment: [:],
            workingDirectory: request.workingDirectory
        )

        try subprocess.launch()
        self.startStderrLogging(for: subprocess, prefix: "JunieCLIACP")

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
            throw RuntimeTransportError.sessionCreationFailed(reason: "No response to ACP initialize request from Junie")
        }

        // Step 2: Send `session/new` with session configuration
        var sessionParams: [String: Any] = [
            "mcpServers": request.mcpServers?.map { $0.acpJSONObject() } ?? ([] as [[String: Any]])
        ]
        if let workingDirectory = request.workingDirectory {
            sessionParams["cwd"] = workingDirectory
        }
        // Junie uses its own model catalog — pass through as-is.
        if let model = request.model {
            sessionParams["model"] = model
        }
        // Map execution policy to Junie mode.
        // Junie uses `autonomous` for write-enabled agents, `interactive` for read-only.
        if let policy = request.executionPolicy {
            if policy.repoWritesAllowed {
                sessionParams["mode"] = "autonomous"
            } else {
                sessionParams["mode"] = "interactive"
            }
        }

        // No _meta block — Junie does not use Claude-specific plugin/MCP options.

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
            throw RuntimeTransportError.sessionCreationFailed(reason: "Invalid session/new response from Junie ACP")
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

        lock.lock()
        sessionEnabledExtensions[sessionId] = enabledExtensions ?? []
        lock.unlock()

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
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "No active Junie session for ID: \(sessionID)"))
                    return
                }

                // Synthesize session lifecycle events (matches RuntimeTransportProtocol pattern)
                continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))

                do {
                    // ACP session/prompt expects prompt as an array of content items:
                    // [{"type": "text", "text": "..."}]
                    // System prompt is embedded in the prompt content.
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
                            ForgeLogger.execution.info("JunieACP: Non-JSON line from subprocess: \(line.prefix(200))")
                            continue
                        }

                        ForgeLogger.execution.debug("JunieACP: Received JSON-RPC: method=\(json["method"] ?? "nil"), id=\(json["id"] ?? "nil"), hasResult=\(json["result"] != nil)")

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
                                let message = ACPProtocolSupport.formatJSONRPCError(
                                    error,
                                    fallback: "Unknown Junie ACP error"
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
                            // Log first few events to diagnose structure
                            if method == "session/update" {
                                let updateKeys = (params?["update"] as? [String: Any])?.keys.sorted().joined(separator: ",") ?? "no-update"
                                let sessionUpdate = (params?["update"] as? [String: Any])?["sessionUpdate"] as? String ?? "no-sessionUpdate"
                                ForgeLogger.execution.debug("JunieACP session/update: sessionUpdate=\(sessionUpdate) updateKeys=\(updateKeys)")
                            }

                            // Handle permission requests by auto-granting
                            if method == "session/request_permission" {
                                self.autoGrantPermission(
                                    subprocess: subprocess,
                                    requestID: json["id"],
                                    params: params,
                                    sessionID: sessionID
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
        lock.lock()
        let subprocess = activeSessions.removeValue(forKey: sessionID)
        requestCounters.removeValue(forKey: sessionID)
        sessionSystemPrompts.removeValue(forKey: sessionID)
        sessionEnabledExtensions.removeValue(forKey: sessionID)
        lock.unlock()

        guard let subprocess else {
            throw RuntimeTransportError.sessionCloseFailed(reason: "No active Junie session for ID: \(sessionID)")
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

    func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
        lock.lock()
        let subprocess = activeSessions[sessionID]
        let enabledExtensions = sessionEnabledExtensions[sessionID] ?? []
        lock.unlock()

        guard subprocess != nil else {
            throw RuntimeTransportError.streamingFailed(reason: "No active Junie session for ID: \(sessionID)")
        }
        return RuntimeSessionRuntimeState(enabledExtensions: enabledExtensions)
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
                    let message = ACPProtocolSupport.formatJSONRPCError(
                        error,
                        fallback: "Unknown Junie ACP error"
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
    /// Based on observed ACP permission flow: the adapter sends `session/request_permission`
    /// with options like `allow_always`, `allow_once`, `reject_once`.
    /// Forge auto-grants with `allow_once` based on the execution policy.
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
            ForgeLogger.execution.error("Failed to auto-grant Junie permission for session \(sessionID)")
            return
        }
        try? subprocess.sendJSON(response)
    }

    private func startStderrLogging(for subprocess: ACPSubprocessManager, prefix: String) {
        Task.detached {
            do {
                for try await line in subprocess.readStderrLines() {
                    if line.localizedCaseInsensitiveContains("error")
                        || line.localizedCaseInsensitiveContains("failed")
                        || line.localizedCaseInsensitiveContains("panic") {
                        ForgeLogger.execution.error("\(prefix) stderr: \(line)")
                    } else {
                        ForgeLogger.execution.info("\(prefix) stderr: \(line)")
                    }
                }
            } catch {
                ForgeLogger.execution.error("\(prefix) stderr reader failed: \(error.localizedDescription)")
            }
        }
    }
}

extension JunieCLIACPTransport: RuntimeTransportTerminationControlling {
    func terminateActiveSessionsForAppShutdown() {
        lock.lock()
        let subprocesses = Array(activeSessions.values)
        activeSessions.removeAll()
        requestCounters.removeAll()
        sessionSystemPrompts.removeAll()
        sessionEnabledExtensions.removeAll()
        lock.unlock()

        for subprocess in subprocesses {
            subprocess.terminate()
        }
    }
}
