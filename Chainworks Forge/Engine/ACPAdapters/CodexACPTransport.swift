import Foundation

// MARK: - CodexACPTransport (Proposal 029 — Full Implementation)

/// ACP transport adapter for OpenAI Codex CLI (`codex` subprocess).
/// Communicates via subprocess stdin/stdout using ACP JSON-RPC protocol over ndjson framing.
///
/// Executable: `codex`
/// Model catalog: `gpt-5`, `gpt-5-codex`, `o4-mini` pass through; others default to `gpt-5`.
/// Mode catalog: `full-access` (repoWritesAllowed), `read-only` (default).
/// Session params: ACP JSON-RPC with `session/new` using `cwd`, `model`, `mode`. No `_meta` block.
nonisolated final class CodexACPTransport: RuntimeTransportProtocol, @unchecked Sendable {

    private struct SessionHandle {
        let subprocess: ACPSubprocessManager
        let runtimeHomeURL: URL?
    }

    // MARK: - Configuration

    let executablePath: String

    var mcpRuntimeNamespace: String? { "codex" }

    // MARK: - Internal State

    private var activeSessions: [String: SessionHandle] = [:]
    private var requestCounters: [String: Int] = [:]
    private var sessionSystemPrompts: [String: String] = [:]
    private var sessionEnabledExtensions: [String: [String]] = [:]
    private var sessionDiagnostics: [String: [RuntimeProviderDiagnostic]] = [:]
    private let lock = NSLock()

    // MARK: - Init

    init(executablePath: String = "codex-acp") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        ForgeLogger.execution.debug("CodexACPTransport.createSession called, executablePath=\(executablePath), workingDir=\(request.workingDirectory ?? "nil")")
        let startTime = Date()
        let runtimeHomeURL = try Self.prepareRuntimeHome(workingDirectory: request.workingDirectory)
        let subprocess = ACPSubprocessManager(
            executablePath: executablePath,
            arguments: [],
            environment: Self.makeSessionEnvironment(runtimeHomeURL: runtimeHomeURL),
            workingDirectory: request.workingDirectory
        )

        do {
            try subprocess.launch()
        } catch {
            Self.cleanupRuntimeHomeIfPresent(runtimeHomeURL)
            throw error
        }
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
            Self.cleanupRuntimeHomeIfPresent(runtimeHomeURL)
            throw RuntimeTransportError.sessionCreationFailed(reason: "No response to ACP initialize request from Codex")
        }

        // Step 2: Send `session/new` with session configuration
        var sessionParams: [String: Any] = [
            "mcpServers": request.mcpServers?.map { $0.acpJSONObject() } ?? ([] as [[String: Any]])
        ]
        if let workingDirectory = request.workingDirectory {
            sessionParams["cwd"] = workingDirectory
        }
        // Map model name to Codex catalog
        if let model = request.model {
            sessionParams["model"] = Self.mapModelForCodexCatalog(model)
        }
        // Map execution policy to the current Codex ACP mode catalog.
        sessionParams["mode"] = Self.mapModeForCodexCatalog(request.executionPolicy)

        // No _meta block — Codex does not use Claude-specific plugin/MCP options.

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
            Self.cleanupRuntimeHomeIfPresent(runtimeHomeURL)
            throw RuntimeTransportError.sessionCreationFailed(reason: "Invalid session/new response from Codex ACP")
        }

        // Extract enabled extensions from the session result
        var enabledExtensions: [String]?
        if let extensions = sessionResult["enabledExtensions"] as? [String] {
            enabledExtensions = extensions
        }

        // Register the active session and store systemPrompt for later prepending
        withLock {
            activeSessions[sessionId] = SessionHandle(subprocess: subprocess, runtimeHomeURL: runtimeHomeURL)
            requestCounters[sessionId] = 2 // initialize=1, session/new=2
            sessionEnabledExtensions[sessionId] = enabledExtensions ?? []
            sessionDiagnostics[sessionId] = []
            if !request.systemPrompt.isEmpty {
                sessionSystemPrompts[sessionId] = request.systemPrompt
            }
        }
        self.startStderrLogging(for: subprocess, prefix: "CodexACP", sessionID: sessionId)

        let startupLatency = Int(Date().timeIntervalSince(startTime) * 1000)

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
                    self.activeSessions[sessionID]?.subprocess
                }

                guard let subprocess else {
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "No active Codex session for ID: \(sessionID)"))
                    return
                }

                // Synthesize session lifecycle events (matches RuntimeTransportProtocol pattern)
                continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))

                do {
                    // ACP session/prompt expects prompt as an array of content items:
                    // [{"type": "text", "text": "..."}]
                    // System prompt is embedded in the prompt content.
                    let systemPrompt = self.withLock {
                        self.sessionSystemPrompts[sessionID]
                    }

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
                            ForgeLogger.execution.info("CodexACP: Non-JSON line from subprocess: \(line.prefix(200))")
                            continue
                        }

                        ForgeLogger.execution.debug("CodexACP: Received JSON-RPC: method=\(json["method"] ?? "nil"), id=\(json["id"] ?? "nil"), hasResult=\(json["result"] != nil)")

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
                                    fallback: "Unknown Codex ACP error"
                                )
                                continuation.yield(.error(message: message))
                            }
                            continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                            continuation.finish()
                            return
                        }

                        // Provider-initiated requests also carry an "id". If we ignore an
                        // unsupported request here, Codex may wait indefinitely for a client
                        // response and the executor only sees a later stall/guardrail timeout.
                        if let method = json["method"] as? String {
                            let params = json["params"] as? [String: Any]
                            if responseID != nil {
                                if method == "session/request_permission" {
                                    let permissionTool = ACPStreamEventMapper.extractPermissionToolNameForDiagnostics(from: params)
                                    ForgeLogger.execution.info("CodexACP permission request: session=\(sessionID) tool=\(permissionTool)")
                                    self.autoGrantPermission(
                                        subprocess: subprocess,
                                        requestID: json["id"],
                                        params: params,
                                        sessionID: sessionID
                                    )
                                } else {
                                    let diagnostic = RuntimeProviderDiagnostic(
                                        source: "codex_request",
                                        severity: .error,
                                        message: "Unsupported Codex ACP client request '\(method)' during prompt execution",
                                        normalizedReason: "unsupported_client_request"
                                    )
                                    self.appendDiagnostic(diagnostic, to: sessionID)
                                    ForgeLogger.execution.error("CodexACP unsupported provider request: session=\(sessionID) method=\(method)")
                                    if let response = ACPProtocolSupport.unsupportedRequestResponse(
                                        requestID: json["id"],
                                        method: method
                                    ) {
                                        do {
                                            try subprocess.sendJSON(response)
                                        } catch {
                                            ForgeLogger.execution.error("CodexACP unsupported-request response send failed for session \(sessionID): \(error.localizedDescription)")
                                        }
                                    }
                                    self.invalidateSession(sessionID: sessionID)
                                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(
                                        reason: "Unsupported Codex ACP client request during prompt execution: \(method)"
                                    ))
                                    return
                                }
                            }
                            // Log first few events to diagnose structure
                            if method == "session/update" {
                                let updateKeys = (params?["update"] as? [String: Any])?.keys.sorted().joined(separator: ",") ?? "no-update"
                                let sessionUpdate = (params?["update"] as? [String: Any])?["sessionUpdate"] as? String ?? "no-sessionUpdate"
                                ForgeLogger.execution.debug("CodexACP session/update: sessionUpdate=\(sessionUpdate) updateKeys=\(updateKeys)")
                            }

                            for event in ACPStreamEventMapper.mapNotificationEvents(method: method, params: params) {
                                continuation.yield(event)
                            }
                        }
                    }

                    self.invalidateSession(sessionID: sessionID)
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(
                        reason: "Codex ACP stream ended before final result was received"
                    ))

                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    self.invalidateSession(sessionID: sessionID)
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { @Sendable _ in
                task.cancel()
            }
        }
    }

    func closeSession(sessionID: String) async throws {
        let handle = withLock {
            let handle = activeSessions.removeValue(forKey: sessionID)
            requestCounters.removeValue(forKey: sessionID)
            sessionSystemPrompts.removeValue(forKey: sessionID)
            sessionEnabledExtensions.removeValue(forKey: sessionID)
            sessionDiagnostics.removeValue(forKey: sessionID)
            return handle
        }

        guard let handle else {
            throw RuntimeTransportError.sessionCloseFailed(reason: "No active Codex session for ID: \(sessionID)")
        }
        let subprocess = handle.subprocess

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
        Self.cleanupRuntimeHomeIfPresent(handle.runtimeHomeURL)
    }

    func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
        let (handle, enabledExtensions) = withLock {
            (
                activeSessions[sessionID],
                sessionEnabledExtensions[sessionID] ?? []
            )
        }

        guard handle != nil else {
            throw RuntimeTransportError.streamingFailed(reason: "No active Codex session for ID: \(sessionID)")
        }
        return RuntimeSessionRuntimeState(
            enabledExtensions: enabledExtensions,
            runtimeHomePath: handle?.runtimeHomeURL?.path,
            providerDiagnostics: currentDiagnostics(for: sessionID)
        )
    }

    // MARK: - Private: Model Mapping

    /// Map Forge model identifiers to the Codex CLI model catalog.
    /// Known Codex models pass through directly; others are mapped to the
    /// closest known model.
    private static func mapModelForCodexCatalog(_ model: String) -> String {
        let lowered = model.lowercased()
        switch lowered {
        case "gpt-5", "gpt-5.4", "gpt-5-codex", "o4-mini":
            return lowered
        default:
            if lowered.contains("gpt-5") { return "gpt-5" }
            if lowered.contains("codex") { return "gpt-5-codex" }
            if lowered.contains("o4-mini") { return "o4-mini" }
            return "gpt-5"
        }
    }

    private static func mapModeForCodexCatalog(_ policy: RuntimeExecutionPolicy?) -> String {
        guard let policy else { return "read-only" }
        return policy.repoWritesAllowed ? "full-access" : "read-only"
    }

    private static func makeSessionEnvironment(runtimeHomeURL: URL?) -> [String: String] {
        guard let runtimeHomeURL else { return [:] }
        let cacheRoot = runtimeHomeURL.appendingPathComponent(".cache", isDirectory: true)
        let shimBinRoot = runtimeHomeURL.appendingPathComponent("bin", isDirectory: true)
        let swiftPMConfigRoot = runtimeHomeURL.appendingPathComponent("Library/org.swift.swiftpm/configuration", isDirectory: true)
        let swiftPMSecurityRoot = runtimeHomeURL.appendingPathComponent("Library/org.swift.swiftpm/security", isDirectory: true)
        let swiftPMCacheRoot = runtimeHomeURL.appendingPathComponent("Library/Caches/org.swift.swiftpm", isDirectory: true)
        let clangModuleCacheRoot = cacheRoot.appendingPathComponent("clang/ModuleCache", isDirectory: true)
        let tempRoot = runtimeHomeURL.appendingPathComponent("tmp", isDirectory: true)
        return [
            "CODEX_HOME": runtimeHomeURL.path,
            "HOME": runtimeHomeURL.path,
            "TMPDIR": tempRoot.path,
            "PATH": "\(shimBinRoot.path):" + (ProcessInfo.processInfo.environment["PATH"] ?? ""),
            "XDG_CACHE_HOME": cacheRoot.path,
            "CLANG_MODULE_CACHE_PATH": clangModuleCacheRoot.path,
            "SWIFTPM_CONFIGURATION_DIRECTORY": swiftPMConfigRoot.path,
            "SWIFTPM_SECURITY_DIRECTORY": swiftPMSecurityRoot.path,
            "SWIFTPM_CACHE_PATH": swiftPMCacheRoot.path,
            "SWIFTPM_DISABLE_SANDBOX": "1"
        ]
    }

    static func prepareRuntimeHome(
        workingDirectory: String?,
        fileManager: FileManager = .default,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        tempRootURL: URL? = nil
    ) throws -> URL {
        let runtimeHomeURL: URL
        if let workingDirectory,
           !workingDirectory.isEmpty {
            let workspaceRoot = URL(fileURLWithPath: workingDirectory, isDirectory: true)
            runtimeHomeURL = workspaceRoot
                .appendingPathComponent(".forge-codex-acp", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
        } else {
            let tempRoot = tempRootURL ?? fileManager.temporaryDirectory
            runtimeHomeURL = tempRoot
                .appendingPathComponent("forge-codex-acp", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
        }
        try fileManager.createDirectory(at: runtimeHomeURL, withIntermediateDirectories: true)

        let sourceHomeURL = sourceCodexHomeURL(environment: environment)
        let sourceAuthURL = sourceHomeURL.appendingPathComponent("auth.json", isDirectory: false)
        let runtimeAuthURL = runtimeHomeURL.appendingPathComponent("auth.json", isDirectory: false)
        let sourceConfigURL = sourceHomeURL.appendingPathComponent("config.toml", isDirectory: false)
        let runtimeConfigURL = runtimeHomeURL.appendingPathComponent("config.toml", isDirectory: false)

        if fileManager.fileExists(atPath: sourceAuthURL.path) {
            try fileManager.copyItem(at: sourceAuthURL, to: runtimeAuthURL)
        } else {
            ForgeLogger.execution.info("CodexACPTransport: auth.json not found at \(sourceAuthURL.path); starting isolated runtime home without copied auth")
        }

        if fileManager.fileExists(atPath: sourceConfigURL.path) {
            let configData = try Data(contentsOf: sourceConfigURL)
            let sanitizedConfigData = sanitizeRuntimeConfig(configData)
            try sanitizedConfigData.write(to: runtimeConfigURL)
        } else {
            ForgeLogger.execution.info("CodexACPTransport: config.toml not found at \(sourceConfigURL.path); starting isolated runtime home without copied runtime config")
        }

        let writableExecRoots = [
            runtimeHomeURL.appendingPathComponent("bin", isDirectory: true),
            runtimeHomeURL.appendingPathComponent("Library/org.swift.swiftpm/configuration", isDirectory: true),
            runtimeHomeURL.appendingPathComponent("Library/org.swift.swiftpm/security", isDirectory: true),
            runtimeHomeURL.appendingPathComponent("Library/Caches/org.swift.swiftpm", isDirectory: true),
            runtimeHomeURL.appendingPathComponent(".cache/clang/ModuleCache", isDirectory: true),
            runtimeHomeURL.appendingPathComponent("tmp", isDirectory: true)
        ]
        for root in writableExecRoots {
            try fileManager.createDirectory(at: root, withIntermediateDirectories: true)
        }

        let swiftShimURL = runtimeHomeURL.appendingPathComponent("bin/swift", isDirectory: false)
        try buildSwiftSandboxShim().write(to: swiftShimURL, atomically: true, encoding: .utf8)
        try fileManager.setAttributes(
            [.posixPermissions: NSNumber(value: 0o755)],
            ofItemAtPath: swiftShimURL.path
        )

        ForgeLogger.execution.debug("CodexACPTransport: prepared isolated CODEX_HOME at \(runtimeHomeURL.path) for workingDir=\(workingDirectory ?? "nil")")
        return runtimeHomeURL
    }

    private static func sourceCodexHomeURL(environment: [String: String]) -> URL {
        if let explicit = environment["CODEX_HOME"], !explicit.isEmpty {
            return URL(fileURLWithPath: explicit, isDirectory: true)
        }
        return URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
            .appendingPathComponent(".codex", isDirectory: true)
    }

    private static func sanitizeRuntimeConfig(_ data: Data) -> Data {
        guard let source = String(data: data, encoding: .utf8) else {
            return data
        }

        let sanitizedLines = source.split(separator: "\n", omittingEmptySubsequences: false).filter { rawLine in
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard !line.hasPrefix("[") else { return true }
            guard let key = line.split(separator: "=", maxSplits: 1).first?.trimmingCharacters(in: .whitespaces).lowercased() else {
                return true
            }
            return key != "model" && key != "model_reasoning_effort"
        }

        let sanitized = sanitizedLines.joined(separator: "\n")
        return Data(sanitized.utf8)
    }

    private static func buildSwiftSandboxShim() -> String {
        """
        #!/bin/zsh
        set -euo pipefail

        real_swift="/usr/bin/xcrun swift"

        if [[ $# -eq 0 ]]; then
          exec /usr/bin/xcrun swift
        fi

        subcommand="$1"
        shift

        case "$subcommand" in
          build|test|package|run)
            for arg in "$@"; do
              if [[ "$arg" == "--disable-sandbox" ]]; then
                exec /usr/bin/xcrun swift "$subcommand" "$@"
              fi
            done
            exec /usr/bin/xcrun swift "$subcommand" --disable-sandbox "$@"
            ;;
          *)
            exec /usr/bin/xcrun swift "$subcommand" "$@"
            ;;
        esac
        """
    }

    static func cleanupRuntimeHomeIfPresent(_ runtimeHomeURL: URL?, fileManager: FileManager = .default) {
        guard let runtimeHomeURL else { return }
        try? fileManager.removeItem(at: runtimeHomeURL)
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

    private func invalidateSession(sessionID: String) {
        let handle = withLock {
            let handle = activeSessions.removeValue(forKey: sessionID)
            requestCounters.removeValue(forKey: sessionID)
            sessionSystemPrompts.removeValue(forKey: sessionID)
            sessionEnabledExtensions.removeValue(forKey: sessionID)
            sessionDiagnostics.removeValue(forKey: sessionID)
            return handle
        }

        guard let handle else { return }
        handle.subprocess.terminate()
        Self.cleanupRuntimeHomeIfPresent(handle.runtimeHomeURL)
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
                        fallback: "Unknown Codex ACP error"
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
        let toolName = ACPStreamEventMapper.extractPermissionToolNameForDiagnostics(from: params)
        guard let response = ACPProtocolSupport.permissionSelectionResponse(
            requestID: requestID,
            params: params
        ) else {
            appendDiagnostic(
                RuntimeProviderDiagnostic(
                    source: "codex_permission",
                    severity: .error,
                    message: "Failed to build permission response for tool '\(toolName)'",
                    normalizedReason: "permission_response_build_failed"
                ),
                to: sessionID
            )
            ForgeLogger.execution.error("CodexACP permission auto-grant failed: could not build response for session \(sessionID)")
            return
        }
        do {
            try subprocess.sendJSON(response)
            ForgeLogger.execution.info("CodexACP permission granted: session=\(sessionID) tool=\(toolName)")
        } catch {
            appendDiagnostic(
                RuntimeProviderDiagnostic(
                    source: "codex_permission",
                    severity: .error,
                    message: "Failed to send permission response for tool '\(toolName)': \(error.localizedDescription)",
                    normalizedReason: "permission_response_send_failed"
                ),
                to: sessionID
            )
            ForgeLogger.execution.error("CodexACP permission auto-grant send failed for session \(sessionID): \(error.localizedDescription)")
        }
    }

    private func startStderrLogging(for subprocess: ACPSubprocessManager, prefix: String, sessionID: String) {
        Task.detached {
            do {
                for try await line in subprocess.readStderrLines() {
                    let sanitized = ACPProtocolSupport.stripANSIEscapeCodes(from: line).trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !sanitized.isEmpty else { continue }
                    if let diagnostic = ACPProtocolSupport.codexProviderDiagnostic(fromStderrLine: sanitized) {
                        self.appendDiagnostic(diagnostic, to: sessionID)
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

    private func currentDiagnostics(for sessionID: String) -> [RuntimeProviderDiagnostic] {
        withLock {
            sessionDiagnostics[sessionID] ?? []
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

    private func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }
}

extension CodexACPTransport: RuntimeTransportTerminationControlling {
    func terminateActiveSessionsForAppShutdown() {
        let handles = withLock {
            let handles = Array(activeSessions.values)
            activeSessions.removeAll()
            requestCounters.removeAll()
            sessionSystemPrompts.removeAll()
            sessionEnabledExtensions.removeAll()
            sessionDiagnostics.removeAll()
            return handles
        }

        for handle in handles {
            handle.subprocess.terminate()
            Self.cleanupRuntimeHomeIfPresent(handle.runtimeHomeURL)
        }
    }
}
