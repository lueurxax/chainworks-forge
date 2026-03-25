import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - LiveGooseConnectionProofTests (Proposal 005 — app-launched real-Goose evidence)

/// Integration test that proves the app can connect to a **real running Goose.app** instance,
/// create a session via the GooseServerTransport, submit a prompt, receive an SSE response,
/// and close the session cleanly.
///
/// Preconditions:
/// - Goose.app must be running on localhost.
/// - GOOSE_PORT and GOOSE_SERVER__SECRET_KEY must be present in the process environment
///   (inherited from the Goose.app launcher).
///
/// Evidence produced:
/// - `live_goose_connection_proof.json`: timestamped evidence pack written to a temp directory,
///   containing session ID, event log, provider response fingerprint, and wall-clock durations.
///
/// This test is intentionally **not** part of the CI fast-test target — it requires a real Goose runtime.
@MainActor
@Suite("Live Goose Connection", .tags(.live), .timeLimit(.minutes(2)))
struct LiveGooseConnectionProofTests {

    // MARK: - Environment Discovery

    /// Well-known path for the Goose runtime discovery file.
    /// Written by the proof harness (or manually) before running tests.
    /// Uses /private/tmp (not NSTemporaryDirectory) because the test runner has a different sandbox tmp.
    private static let discoveryFilePath = "/private/tmp/chainworks_goose_discovery.json"

    /// Detect the running Goose.app from multiple sources:
    /// 1. Process environment (GOOSE_PORT + GOOSE_SERVER__SECRET_KEY)
    /// 2. Chainworks-specific env vars
    /// 3. Discovery file written by test harness
    /// 4. Process scan for goosed + probe known ports
    private func discoverGooseRuntime() -> (baseURL: URL, secretKey: String)? {
        let env = ProcessInfo.processInfo.environment

        // Source 1: explicit env vars from Goose.app launcher
        if let port = env["GOOSE_PORT"],
           let secretKey = env["GOOSE_SERVER__SECRET_KEY"],
           !port.isEmpty, !secretKey.isEmpty {
            let baseURL = URL(string: "https://127.0.0.1:\(port)")!
            return (baseURL, secretKey)
        }

        // Source 2: Chainworks-specific env vars
        if let urlString = env["CHAINWORKS_GOOSE_BASE_URL"],
           let url = URL(string: urlString),
           let apiKey = env["CHAINWORKS_GOOSE_API_KEY"],
           !apiKey.isEmpty {
            return (url, apiKey)
        }

        // Source 3: Discovery file (written by `write-goose-discovery` setup step)
        if let data = FileManager.default.contents(atPath: Self.discoveryFilePath),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: String],
           let port = json["port"], let secretKey = json["secret_key"],
           !port.isEmpty, !secretKey.isEmpty {
            let baseURL = URL(string: "https://127.0.0.1:\(port)")!
            return (baseURL, secretKey)
        }

        // Source 4: Scan running goosed processes for port
        if let discovered = discoverFromRunningProcesses() {
            return discovered
        }

        return nil
    }

    /// Scan running processes for goosed and extract connection info.
    private func discoverFromRunningProcesses() -> (baseURL: URL, secretKey: String)? {
        // Use `lsof` to find what port goosed is listening on
        let lsofTask = Process()
        lsofTask.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
        lsofTask.arguments = ["-i", "TCP", "-sTCP:LISTEN", "-P", "-n"]

        let pipe = Pipe()
        lsofTask.standardOutput = pipe
        lsofTask.standardError = FileHandle.nullDevice

        do {
            try lsofTask.run()
            lsofTask.waitUntilExit()
        } catch {
            return nil
        }

        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        guard let output = String(data: data, encoding: .utf8) else { return nil }

        // Find goosed listening ports
        var goosedPort: String?
        for line in output.components(separatedBy: .newlines) {
            if line.contains("goosed") && line.contains("LISTEN") {
                // Extract port: "goosed ... TCP 127.0.0.1:63575 (LISTEN)"
                if let portMatch = line.range(of: #"127\.0\.0\.1:(\d+)"#, options: .regularExpression) {
                    let portStr = line[portMatch]
                    if let colonIdx = portStr.lastIndex(of: ":") {
                        goosedPort = String(portStr[portStr.index(after: colonIdx)...])
                    }
                } else if let portMatch = line.range(of: #"\*:(\d+)"#, options: .regularExpression) {
                    let portStr = line[portMatch]
                    if let colonIdx = portStr.lastIndex(of: ":") {
                        goosedPort = String(portStr[portStr.index(after: colonIdx)...])
                    }
                }
            }
        }

        guard let port = goosedPort else { return nil }

        // Try to read the secret key from Goose.app's renderer process environment
        // which is passed as GOOSE_SERVER__SECRET_KEY
        let psTask = Process()
        psTask.executableURL = URL(fileURLWithPath: "/bin/ps")
        psTask.arguments = ["eww", "-o", "command"]

        let psPipe = Pipe()
        psTask.standardOutput = psPipe
        psTask.standardError = FileHandle.nullDevice

        do {
            try psTask.run()
            psTask.waitUntilExit()
        } catch {
            return nil
        }

        let psData = psPipe.fileHandleForReading.readDataToEndOfFile()
        guard let psOutput = String(data: psData, encoding: .utf8) else { return nil }

        // Look for Goose Helper process which carries the secret key in its JSON env arg
        for line in psOutput.components(separatedBy: .newlines) {
            if line.contains("Goose Helper") && line.contains("GOOSE_API_HOST") {
                // The renderer process has a JSON argument containing the key
                // Extract from the JSON blob
                if let jsonStart = line.range(of: "{\"GOOSE_API_HOST") {
                    let jsonCandidate = line[jsonStart.lowerBound...]
                    if let jsonEnd = jsonCandidate.range(of: "}") {
                        let jsonStr = String(jsonCandidate[...jsonEnd.lowerBound])
                        if let jsonData = jsonStr.data(using: .utf8),
                           let _ = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any] {
                            // The secret key isn't directly in this JSON — it's in the server env
                            // But we can verify the port matches
                            break
                        }
                    }
                }
            }
        }

        // Without the secret key from process scan, we can't authenticate.
        // Write a discovery file prompt for the user.
        return nil
    }

    /// Write a discovery file so the Xcode test runner can find the Goose runtime.
    /// Call this from the terminal before running tests:
    /// ```
    /// echo '{"port":"'$GOOSE_PORT'","secret_key":"'$GOOSE_SERVER__SECRET_KEY'"}' > /tmp/chainworks_goose_discovery.json
    /// ```
    static func writeDiscoveryFile(port: String, secretKey: String) {
        let json: [String: String] = ["port": port, "secret_key": secretKey]
        if let data = try? JSONSerialization.data(withJSONObject: json) {
            FileManager.default.createFile(atPath: discoveryFilePath, contents: data)
        }
    }

    // MARK: - Evidence Structures

    struct ConnectionProofEvidence: Codable {
        let testName: String
        let timestamp: String
        let gooseBaseURL: String
        let sessionID: String
        let sessionCreatedAt: String
        let providerConfigured: Bool
        let promptSubmitted: Bool
        let sseEventsReceived: Int
        let eventLog: [EventLogEntry]
        let sessionClosedCleanly: Bool
        let totalDurationSeconds: Double
        let sessionCreateDurationSeconds: Double
        let promptRoundTripDurationSeconds: Double
        let verdict: String
        let appBundleID: String
        let hostName: String
        let osVersion: String
    }

    struct EventLogEntry: Codable {
        let index: Int
        let type: String
        let snippet: String
        let receivedAtOffset: Double
    }

    // MARK: - Tests

    /// PROOF: App can create a session on real Goose, send a prompt, receive SSE response, close session.
    @Test("App-launched real Goose connection proof",
          .disabled("Requires running Goose server; enable for manual validation"))
    func appLaunchedRealGooseConnection() async throws {
        // Step 0: Discover Goose runtime
        guard let runtime = discoverGooseRuntime() else {
            Issue.record("Goose.app is not running — GOOSE_PORT or GOOSE_SERVER__SECRET_KEY not set. This test requires a live Goose instance.")
            return
        }

        let testStart = Date()
        let transport = GooseServerTransport(
            baseURL: runtime.baseURL,
            secretKey: runtime.secretKey,
            provider: "claude-code",
            model: "default"
        )

        // Step 1: Create session via POST /agent/start + POST /agent/update_provider
        let sessionCreateStart = Date()
        let sessionRequest = GooseSessionRequest(
            systemPrompt: "You are a test agent for Chainworks Forge integration proof. Respond with exactly: CHAINWORKS_PROOF_OK",
            workingDirectory: FileManager.default.temporaryDirectory.path,
            model: "default",
            provider: "claude-code",
            executionPolicy: GooseExecutionPolicy(
                permissionProfileID: "SAFE_READONLY",
                workspaceMode: "read_only",
                gitOperationsAllowed: false,
                releaseOperationsAllowed: false,
                repoWritesAllowed: false
            ),
            metadata: [
                "test_name": "testAppLaunchedRealGooseConnection",
                "timestamp": ISO8601DateFormatter().string(from: testStart)
            ]
        )

        let sessionResponse: GooseSessionResponse
        do {
            sessionResponse = try await transport.createSession(request: sessionRequest)
        } catch {
            Issue.record("Session creation failed on real Goose: \(error.localizedDescription)")
            return
        }
        let sessionCreateDuration = Date().timeIntervalSince(sessionCreateStart)

        #expect(!sessionResponse.sessionId.isEmpty, "Session ID must not be empty")
        #expect(sessionResponse.policyAcknowledgement?.accepted == true, "Policy must be acknowledged")

        // Step 2: Submit a prompt and collect SSE events
        let promptStart = Date()
        let promptRequest = GoosePromptRequest(
            content: """
            This is a Chainworks Forge integration proof test.
            Reply with a single line: CHAINWORKS_PROOF_OK
            Do not use any tools. Just reply with text.
            """,
            context: [
                GooseContextAttachment(
                    type: "text",
                    name: "test_context",
                    content: "Integration proof — \(ISO8601DateFormatter().string(from: testStart))",
                    path: nil
                )
            ]
        )

        let eventStream = transport.submitPrompt(
            sessionID: sessionResponse.sessionId,
            prompt: promptRequest
        )

        var eventLog: [EventLogEntry] = []
        var eventIndex = 0
        var receivedTextChunks = false
        var receivedFinalOutput = false
        var receivedSessionClosed = false

        do {
            for try await event in eventStream {
                let offset = Date().timeIntervalSince(promptStart)
                let entry: EventLogEntry

                switch event {
                case .sessionStarted(let raw):
                    entry = EventLogEntry(index: eventIndex, type: "session_started", snippet: String(raw.prefix(200)), receivedAtOffset: offset)
                case .promptSubmitted(let raw):
                    entry = EventLogEntry(index: eventIndex, type: "prompt_submitted", snippet: String(raw.prefix(200)), receivedAtOffset: offset)
                case .toolCallStarted(let toolName, _):
                    entry = EventLogEntry(index: eventIndex, type: "tool_call_started", snippet: toolName, receivedAtOffset: offset)
                case .toolCallFinished(let toolName, _):
                    entry = EventLogEntry(index: eventIndex, type: "tool_call_finished", snippet: toolName, receivedAtOffset: offset)
                case .textChunk(let text):
                    receivedTextChunks = true
                    entry = EventLogEntry(index: eventIndex, type: "text_chunk", snippet: String(text.prefix(500)), receivedAtOffset: offset)
                case .finalOutput(let content):
                    receivedFinalOutput = true
                    entry = EventLogEntry(index: eventIndex, type: "final_output", snippet: String(content.prefix(500)), receivedAtOffset: offset)
                case .error(let message):
                    entry = EventLogEntry(index: eventIndex, type: "error", snippet: message, receivedAtOffset: offset)
                case .sessionClosed(let raw):
                    receivedSessionClosed = true
                    entry = EventLogEntry(index: eventIndex, type: "session_closed", snippet: String(raw.prefix(200)), receivedAtOffset: offset)
                case .unknown(let type, let data):
                    entry = EventLogEntry(index: eventIndex, type: "unknown:\(type)", snippet: String(data.prefix(200)), receivedAtOffset: offset)
                }

                eventLog.append(entry)
                eventIndex += 1
            }
        } catch {
            // Stream may throw on disconnect — still record what we got
            eventLog.append(EventLogEntry(
                index: eventIndex,
                type: "stream_error",
                snippet: error.localizedDescription,
                receivedAtOffset: Date().timeIntervalSince(promptStart)
            ))
        }
        let promptRoundTripDuration = Date().timeIntervalSince(promptStart)

        // Step 3: Close session
        var sessionClosedCleanly = false
        do {
            try await transport.closeSession(sessionID: sessionResponse.sessionId)
            sessionClosedCleanly = true
        } catch {
            // Session may already be closed by goosed after Finish event — that's acceptable
            let errorStr = error.localizedDescription
            if errorStr.contains("404") || errorStr.contains("410") {
                sessionClosedCleanly = true // Already cleaned up server-side
            }
        }

        let totalDuration = Date().timeIntervalSince(testStart)

        // Step 4: Build evidence pack
        let receivedMeaningfulResponse = receivedTextChunks || receivedFinalOutput
        let verdict: String
        if receivedMeaningfulResponse && !sessionResponse.sessionId.isEmpty {
            verdict = "PASS — real Goose session created, prompt submitted, SSE response received"
        } else if !sessionResponse.sessionId.isEmpty {
            verdict = "PARTIAL — session created but no text/final output received (\(eventLog.count) events)"
        } else {
            verdict = "FAIL — could not establish session"
        }

        let evidence = ConnectionProofEvidence(
            testName: "testAppLaunchedRealGooseConnection",
            timestamp: ISO8601DateFormatter().string(from: testStart),
            gooseBaseURL: runtime.baseURL.absoluteString,
            sessionID: sessionResponse.sessionId,
            sessionCreatedAt: ISO8601DateFormatter().string(from: Date(timeInterval: sessionCreateDuration, since: testStart)),
            providerConfigured: true,
            promptSubmitted: true,
            sseEventsReceived: eventLog.count,
            eventLog: eventLog,
            sessionClosedCleanly: sessionClosedCleanly,
            totalDurationSeconds: totalDuration,
            sessionCreateDurationSeconds: sessionCreateDuration,
            promptRoundTripDurationSeconds: promptRoundTripDuration,
            verdict: verdict,
            appBundleID: Bundle.main.bundleIdentifier ?? "com.chainworks.forge.tests",
            hostName: ProcessInfo.processInfo.hostName,
            osVersion: ProcessInfo.processInfo.operatingSystemVersionString
        )

        // Step 5: Write evidence to disk
        let evidenceDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksForge-LiveProof-\(UUID().uuidString.prefix(8))", isDirectory: true)
        try FileManager.default.createDirectory(at: evidenceDir, withIntermediateDirectories: true)

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let evidenceData = try encoder.encode(evidence)
        let evidencePath = evidenceDir.appendingPathComponent("live_goose_connection_proof.json")
        try evidenceData.write(to: evidencePath)

        // Log evidence path for manual inspection
        print("======================================================================")
        print("  LIVE GOOSE CONNECTION PROOF")
        print("======================================================================")
        print("  Verdict: \(verdict)")
        print("  Session ID: \(sessionResponse.sessionId)")
        print("  Goose URL: \(runtime.baseURL.absoluteString)")
        print("  SSE events: \(eventLog.count)")
        print("  Session create: \(String(format: "%.2f", sessionCreateDuration))s")
        print("  Prompt round-trip: \(String(format: "%.2f", promptRoundTripDuration))s")
        print("  Total duration: \(String(format: "%.2f", totalDuration))s")
        print("  Session closed cleanly: \(sessionClosedCleanly)")
        print("  Evidence: \(evidencePath.path)")
        print("======================================================================")

        // Assertions
        #expect(!sessionResponse.sessionId.isEmpty, "Must have a valid session ID from real Goose")
        #expect(eventLog.count > 0, "Must receive at least one SSE event from real Goose")
        #expect(receivedSessionClosed || receivedFinalOutput,
                "Stream should end with session_closed or final_output")
        #expect(
            receivedMeaningfulResponse,
            "Must receive text_chunk or final_output from real Goose. Events: \(eventLog.map(\.type).joined(separator: ", "))"
        )
    }

    /// PROOF: GooseServerTransport correctly uses X-Secret-Key auth (not Bearer token).
    @Test("Transport auth header uses X-Secret-Key",
          .disabled("Requires running Goose server; enable for manual validation"))
    func transportAuthHeaderIsSecretKey() async throws {
        guard let runtime = discoverGooseRuntime() else {
            Issue.record("Goose.app is not running")
            return
        }

        // Create a transport with the real secret key — session creation proves auth works
        let transport = GooseServerTransport(
            baseURL: runtime.baseURL,
            secretKey: runtime.secretKey,
            provider: "claude-code",
            model: "default"
        )

        let sessionRequest = GooseSessionRequest(
            systemPrompt: "Auth verification test",
            workingDirectory: FileManager.default.temporaryDirectory.path,
            model: "default",
            provider: "claude-code",
            executionPolicy: nil,
            metadata: ["test": "auth_header_proof"]
        )

        // If this succeeds, the X-Secret-Key header is correct
        let response = try await transport.createSession(request: sessionRequest)
        #expect(!response.sessionId.isEmpty, "Auth with X-Secret-Key must produce a valid session")

        // Clean up
        try? await transport.closeSession(sessionID: response.sessionId)
    }

    /// PROOF: App environment correctly discovers Goose.app runtime parameters.
    @Test("Goose runtime discovery from environment",
          .disabled("Requires running Goose server; enable for manual validation"))
    func gooseRuntimeDiscovery() throws {
        let env = ProcessInfo.processInfo.environment

        // When running inside Goose.app, these must be present
        let goosePort = env["GOOSE_PORT"]
        let gooseSecretKey = env["GOOSE_SERVER__SECRET_KEY"]

        if goosePort == nil && gooseSecretKey == nil {
            Issue.record("Not running inside Goose.app — GOOSE_PORT not set")
            return
        }

        #expect(goosePort != nil, "GOOSE_PORT must be set when running inside Goose.app")
        #expect(gooseSecretKey != nil, "GOOSE_SERVER__SECRET_KEY must be set when running inside Goose.app")

        if let port = goosePort {
            let portInt = Int(port)
            #expect(portInt != nil, "GOOSE_PORT must be a valid integer")
            #expect((portInt ?? 0) > 0, "GOOSE_PORT must be > 0")
            #expect((portInt ?? 70000) < 65536, "GOOSE_PORT must be < 65536")
        }

        if let key = gooseSecretKey {
            #expect(key.count > 16, "Secret key should be reasonably long")
        }

        print("[GooseRuntimeDiscovery] GOOSE_PORT=\(goosePort ?? "nil"), key length=\(gooseSecretKey?.count ?? 0)")
    }

    /// PROOF: Full GooseAgentExecutor pipeline works against real Goose (session -> prompt -> events -> result).
    @Test("Full agent executor pipeline with real Goose",
          .disabled("Requires running Goose server; enable for manual validation"))
    func fullAgentExecutorPipelineWithRealGoose() async throws {
        guard let runtime = discoverGooseRuntime() else {
            Issue.record("Goose.app is not running")
            return
        }

        let transport = GooseServerTransport(
            baseURL: runtime.baseURL,
            secretKey: runtime.secretKey,
            provider: "claude-code",
            model: "default"
        )

        let executor = GooseAgentExecutor(transport: transport)

        // Collect live execution events (uses SharedEventCollector per TEST-004)
        let collector = SharedEventCollector()
        executor.onExecutionEvent = { _, event in
            collector.append(event)
        }

        // Build minimal agent + task + context
        let agent = ResolvedAgent(
            id: "proof_agent",
            title: "Connection Proof Agent",
            mode: "autonomous",
            provider: "claude-code",
            model: "default",
            effort: "high",
            maxTurns: 3,
            temperature: 0.0,
            permissionProfile: "SAFE_READONLY",
            skillRef: "proof_skill",
            skillRole: nil,
            prompt: "You are a minimal proof-of-connectivity agent. Just reply with: PROOF_OK",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proof_output"]
        )

        let task = AgentTask(
            agent: "proof_agent",
            task: "Reply with PROOF_OK. Do not use tools.",
            inputs: nil,
            outputs: ["proof_output"]
        )

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("proof-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let executionContext = ExecutionContext(
            workspace: workspace,
            stageID: "proof_stage",
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Live Goose connection proof test",
            providerBinding: nil
        )

        // Execute through the full pipeline
        let startTime = Date()
        let result = try await executor.execute(task: task, agent: agent, context: executionContext)
        let duration = Date().timeIntervalSince(startTime)

        // Assertions
        #expect(result.sessionID != nil, "Must have a real session ID")
        #expect(result.durationSeconds > 0, "Duration must be positive")
        #expect(collector.events.count > 0, "Must receive live execution events")

        // The receipt must be present regardless of success
        let hasReceipt = result.outputs.keys.contains { $0.hasSuffix("_receipt.json") }
        #expect(hasReceipt, "Must produce an execution receipt")

        // If the receipt exists, verify it's valid JSON
        if let receiptData = result.outputs.first(where: { $0.key.hasSuffix("_receipt.json") })?.value {
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let receipt = try decoder.decode(ExecutionReceipt.self, from: receiptData)
            #expect(receipt.agentID == "proof_agent")
            #expect(!receipt.sessionID.isEmpty, "Receipt must capture real session ID")
        }

        print("======================================================================")
        print("  FULL PIPELINE PROOF")
        print("======================================================================")
        print("  Session ID: \(result.sessionID ?? "nil")")
        print("  Succeeded: \(result.succeeded)")
        print("  Duration: \(String(format: "%.2f", duration))s")
        print("  Events received: \(collector.events.count)")
        print("  Outputs: \(result.outputs.keys.sorted().joined(separator: ", "))")
        print("  Error: \(result.errorMessage ?? "none")")
        print("======================================================================")
    }
}
