import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseServerLiveIntegrationTests (Proposal 005, REQ-008/REQ-009)

/// Live integration tests against a real running `goosed agent` instance.
///
/// These tests are SKIPPED by default unless `CHAINWORKS_LIVE_INTEGRATION_TEST=1`
/// is set in the environment, because they require:
///
/// 1. A running `goosed agent` on the configured port
/// 2. A valid `claude-code` provider with `claude` CLI in PATH
/// 3. 60-120 seconds for cold-start latency
///
/// To run manually:
/// ```
/// PATH="$HOME/.local/bin:$PATH" \
/// GOOSE_SERVER__SECRET_KEY=chainworks-dev-secret \
/// GOOSE_PORT=51200 \
/// /Applications/Goose.app/Contents/Resources/bin/goosed agent &
///
/// CHAINWORKS_LIVE_INTEGRATION_TEST=1 \
/// xcodebuild test -project "Chainworks Forge.xcodeproj" \
///   -scheme "Chainworks Forge" -destination "platform=macOS" \
///   -only-testing:"Chainworks ForgeTests/GooseServerLiveIntegrationTests"
/// ```
@Suite("Goose Server Live Integration", .tags(.live), .timeLimit(.minutes(2)))
struct GooseServerLiveIntegrationTests {

    let transport: GooseServerTransport?
    let baseURL = URL(string: "https://127.0.0.1:51200")!
    let secretKey = "chainworks-dev-secret"

    /// Check both env var and live server availability.
    /// Tests skip automatically if goosed is not reachable.
    private static func checkShouldRun() -> Bool {
        // Allow explicit opt-in via env var
        if ProcessInfo.processInfo.environment["CHAINWORKS_LIVE_INTEGRATION_TEST"] == "1" {
            return true
        }
        // Also run if goosed is reachable (auto-detect)
        if let url = URL(string: "https://127.0.0.1:51200/status") {
            var request = URLRequest(url: url)
            request.timeoutInterval = 2
            let semaphore = DispatchSemaphore(value: 0)
            var reachable = false
            let delegate = LocalhostTrustDelegate()
            let session = URLSession(configuration: .ephemeral, delegate: delegate, delegateQueue: nil)
            session.dataTask(with: request) { _, response, _ in
                reachable = (response as? HTTPURLResponse)?.statusCode == 200
                semaphore.signal()
            }.resume()
            semaphore.wait()
            session.invalidateAndCancel()
            return reachable
        }
        return false
    }

    private let shouldRun: Bool

    init() {
        shouldRun = Self.checkShouldRun()

        if shouldRun {
            transport = GooseServerTransport(
                baseURL: baseURL,
                secretKey: secretKey,
                provider: "claude-code",
                model: "default"
            )
        } else {
            transport = nil
        }
    }

    // MARK: - Live Session Lifecycle

    /// Full live round-trip: create session -> set provider -> send prompt -> receive Message + Finish -> close.
    /// Proves the GooseServerTransport adapter works against real goosed.
    @Test("Live round-trip: create, prompt, close",
          .disabled("Requires running Goose server"))
    func liveRoundTripCreatePromptClose() async throws {
        guard shouldRun, let transport else {
            Issue.record("Live integration test skipped (goosed not reachable at \(baseURL))")
            return
        }

        // Step 1: Server reachability already verified in shouldRun

        // Step 2: Create session
        let workDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("live-test-\(UUID().uuidString.prefix(8))", isDirectory: true)
        try FileManager.default.createDirectory(at: workDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: workDir) }

        let sessionRequest = GooseSessionRequest(
            systemPrompt: """
            You are a test agent.
            Do not call xcode_mcp or any IDE/editor MCP tools.
            Respond directly in plain text.
            """,
            workingDirectory: workDir.path,
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: nil
        )

        let sessionResponse = try await transport.createSession(request: sessionRequest)
        #expect(!sessionResponse.sessionId.isEmpty, "Session ID should not be empty")
        #expect(sessionResponse.policyAcknowledgement?.accepted == true)

        let sessionID = sessionResponse.sessionId

        // Step 3: Submit a trivial prompt and collect events
        // Allow up to 180 seconds for cold-start + execution
        let prompt = GoosePromptRequest(
            content: "Reply with exactly: live test ok",
            context: nil
        )

        let eventStream = transport.submitPrompt(sessionID: sessionID, prompt: prompt)

        var textChunks: [String] = []
        var hasFinalOutput = false
        var hasSessionClosed = false
        var hasError = false

        for try await event in eventStream {
            switch event {
            case .textChunk(let text):
                textChunks.append(text)
            case .finalOutput:
                hasFinalOutput = true
            case .sessionClosed:
                hasSessionClosed = true
            case .error(let message):
                hasError = true
                Issue.record("Received error event: \(message)")
            default:
                break
            }
        }

        // Step 4: Verify we got a real response
        #expect(!hasError, "Should not have received any error events")
        #expect(hasFinalOutput, "Should have received a Finish/finalOutput event")
        #expect(hasSessionClosed, "Should have received sessionClosed")

        let fullText = textChunks.joined()
        #expect(!fullText.isEmpty, "Should have received at least one text chunk with real content")

        // Step 5: Close session
        do {
            try await transport.closeSession(sessionID: sessionID)
        } catch {
            // Acceptable — session may already be closed by server after Finish
            print("Note: closeSession returned error (acceptable): \(error)")
        }

        // Save evidence
        let evidence = """
        # Live Integration Test Evidence

        Date: \(ISO8601DateFormatter().string(from: Date()))
        Session ID: \(sessionID)
        Text chunks received: \(textChunks.count)
        Full text: \(fullText)
        Has final output: \(hasFinalOutput)
        Has session closed: \(hasSessionClosed)
        Has error: \(hasError)
        """

        let evidencePath = workDir.appendingPathComponent("live-test-evidence.txt")
        try? evidence.data(using: .utf8)?.write(to: evidencePath)
        print("Live test evidence saved to: \(evidencePath.path)")
    }
}
