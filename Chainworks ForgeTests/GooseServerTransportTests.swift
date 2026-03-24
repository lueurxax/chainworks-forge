import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseServerTransportTests (Proposal 005, Section 8)

/// Unit and integration tests for GooseServerTransport.
/// Covers initialization, protocol conformance, ChatRequest encoding (REQ-010),
/// and full mock-HTTP round-trip (REQ-011).
final class GooseServerTransportTests: XCTestCase {

    // MARK: - MockURLProtocol

    /// URLProtocol subclass that intercepts HTTP requests for testing.
    /// Records all requests and returns pre-configured responses.
    final class MockURLProtocol: URLProtocol, @unchecked Sendable {
        /// Recorded requests (thread-safe via queue).
        nonisolated(unsafe) static var requestLog: [(url: String, method: String, headers: [String: String], body: Data?)] = []
        private static let logQueue = DispatchQueue(label: "MockURLProtocol.log")

        /// Handler that returns (statusCode, responseHeaders, responseBody) for a given request.
        nonisolated(unsafe) static var requestHandler: ((URLRequest) -> (Int, [String: String], Data))? = nil

        static func reset() {
            logQueue.sync { requestLog = [] }
            requestHandler = nil
        }

        static func recordedRequests() -> [(url: String, method: String, headers: [String: String], body: Data?)] {
            logQueue.sync { requestLog }
        }

        override class func canInit(with request: URLRequest) -> Bool { true }
        override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

        override func startLoading() {
            let url = request.url?.absoluteString ?? "unknown"
            let method = request.httpMethod ?? "GET"
            let headers = request.allHTTPHeaderFields ?? [:]
            let body = request.httpBody ?? request.httpBodyStream.flatMap { stream in
                stream.open()
                var data = Data()
                let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: 4096)
                defer { buffer.deallocate() }
                while stream.hasBytesAvailable {
                    let count = stream.read(buffer, maxLength: 4096)
                    if count > 0 { data.append(buffer, count: count) }
                    else { break }
                }
                stream.close()
                return data
            }

            Self.logQueue.sync {
                Self.requestLog.append((url: url, method: method, headers: headers, body: body))
            }

            if let handler = Self.requestHandler {
                let (statusCode, responseHeaders, responseBody) = handler(request)
                let httpResponse = HTTPURLResponse(
                    url: request.url!,
                    statusCode: statusCode,
                    httpVersion: "HTTP/1.1",
                    headerFields: responseHeaders
                )!
                client?.urlProtocol(self, didReceive: httpResponse, cacheStoragePolicy: .notAllowed)
                client?.urlProtocol(self, didLoad: responseBody)
                client?.urlProtocolDidFinishLoading(self)
            } else {
                let error = NSError(domain: "MockURLProtocol", code: -1, userInfo: [NSLocalizedDescriptionKey: "No handler configured"])
                client?.urlProtocol(self, didFailWithError: error)
            }
        }

        override func stopLoading() {}
    }

    // MARK: - Helpers

    private func makeMockTransport(
        secretKey: String? = "test-secret",
        provider: String? = "claude-code",
        model: String? = "default"
    ) -> GooseServerTransport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 20

        return GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            secretKey: secretKey,
            provider: provider,
            model: model,
            requestTimeout: 10,
            sessionConfiguration: config
        )
    }

    override func setUp() {
        super.setUp()
        MockURLProtocol.reset()
    }

    override func tearDown() {
        MockURLProtocol.reset()
        super.tearDown()
    }

    // MARK: - Transport API Selection

    func testGooseTransportAPIRawValues() {
        XCTAssertEqual(GooseTransportAPI.bespoke.rawValue, "bespoke")
        XCTAssertEqual(GooseTransportAPI.gooseServer.rawValue, "goose_server")
    }

    func testGooseTransportAPIFromRawValue() {
        XCTAssertEqual(GooseTransportAPI(rawValue: "bespoke"), .bespoke)
        XCTAssertEqual(GooseTransportAPI(rawValue: "goose_server"), .gooseServer)
        XCTAssertNil(GooseTransportAPI(rawValue: "invalid"))
    }

    // MARK: - GooseServerTransport Initialization

    func testServerTransportInitialization() {
        let transport = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            secretKey: "test-secret",
            provider: "claude-code",
            model: "default"
        )

        XCTAssertEqual(transport.baseURL.absoluteString, "https://127.0.0.1:51200")
        XCTAssertEqual(transport.secretKey, "test-secret")
        XCTAssertEqual(transport.provider, "claude-code")
        XCTAssertEqual(transport.model, "default")
        XCTAssertEqual(transport.requestTimeout, 300)
    }

    func testServerTransportCustomTimeout() {
        let transport = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            requestTimeout: 600
        )
        XCTAssertEqual(transport.requestTimeout, 600)
    }

    // MARK: - Protocol Conformance

    func testServerTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            secretKey: "test-secret"
        )
        XCTAssertNotNil(transport)
    }

    func testBespokeTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = GooseTransport(
            baseURL: URL(string: "http://localhost:3000")!,
            apiKey: "test-key"
        )
        XCTAssertNotNil(transport)
    }

    func testFixtureTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = FixtureGooseTransport(
            scenario: .proposalLoopSuccess
        )
        XCTAssertNotNil(transport)
    }

    // MARK: - LiveRuntimeConfiguration with transportAPI

    func testLiveRuntimeConfigurationWithGooseServer() {
        let config = LiveRuntimeConfiguration(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            apiKey: "test-secret",
            override: LiveExecutionOverride(
                enabled: true,
                provider: "claude-code",
                model: "default",
                effort: "high"
            ),
            transportMode: .network,
            transportAPI: .gooseServer
        )

        XCTAssertEqual(config.transportAPI, .gooseServer)
        XCTAssertTrue(config.sourceDescription.contains("goosed"))
    }

    func testLiveRuntimeConfigurationWithBespoke() {
        let config = LiveRuntimeConfiguration(
            baseURL: URL(string: "http://localhost:3000")!,
            apiKey: "test-key",
            override: nil,
            transportMode: .network,
            transportAPI: .bespoke
        )

        XCTAssertEqual(config.transportAPI, .bespoke)
        XCTAssertTrue(config.sourceDescription.contains("bespoke"))
    }

    func testLiveRuntimeConfigurationFixtureMode() {
        let config = LiveRuntimeConfiguration(
            baseURL: URL(string: "http://fixture.local")!,
            apiKey: nil,
            override: nil,
            transportMode: .fixtureProposalLoopSuccess,
            transportAPI: .bespoke
        )

        XCTAssertTrue(config.sourceDescription.contains("Fixture"))
    }

    // MARK: - REQ-010: ChatRequest Encoding Unit Tests

    /// Verifies that submitPrompt() encodes metadata.userVisible and metadata.agentVisible
    /// in the /reply request body (Proposal 005, Section 4.3 — required fields).
    func testChatRequestEncodingContainsRequiredMetadata() async throws {
        let transport = makeMockTransport()

        // Set up mock handler that captures and returns a minimal SSE response
        MockURLProtocol.requestHandler = { request in
            let sseBody = "data: {\"type\":\"Finish\",\"reason\":\"stop\"}\n\n"
            return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
        }

        let prompt = GoosePromptRequest(
            content: "Test prompt for encoding verification",
            context: nil
        )

        let stream = transport.submitPrompt(sessionID: "test-session-1", prompt: prompt)
        // Consume the stream
        for try await _ in stream {}

        // Find the /reply request
        let requests = MockURLProtocol.recordedRequests()
        let replyRequest = requests.first { $0.url.contains("/reply") }
        XCTAssertNotNil(replyRequest, "Should have sent a POST /reply request")

        guard let body = replyRequest?.body,
              let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
            XCTFail("Request body should be valid JSON")
            return
        }

        // Verify session_id
        XCTAssertEqual(json["session_id"] as? String, "test-session-1")

        // Verify user_message structure
        guard let userMessage = json["user_message"] as? [String: Any] else {
            XCTFail("Request body should contain user_message")
            return
        }

        XCTAssertEqual(userMessage["role"] as? String, "user")
        XCTAssertNotNil(userMessage["created"] as? Int, "created must be a Unix timestamp integer")

        // Verify content array
        guard let content = userMessage["content"] as? [[String: Any]],
              let firstContent = content.first else {
            XCTFail("user_message.content should be a non-empty array")
            return
        }
        XCTAssertEqual(firstContent["type"] as? String, "text")
        XCTAssertTrue((firstContent["text"] as? String)?.contains("Test prompt for encoding verification") == true)

        // REQ-010 critical assertion: metadata.userVisible and metadata.agentVisible are REQUIRED
        guard let metadata = userMessage["metadata"] as? [String: Any] else {
            XCTFail("user_message.metadata must be present (server returns 422 without it)")
            return
        }
        XCTAssertEqual(metadata["userVisible"] as? Bool, true,
                       "metadata.userVisible must be true (required by goosed, 422 without it)")
        XCTAssertEqual(metadata["agentVisible"] as? Bool, true,
                       "metadata.agentVisible must be true (required by goosed, 422 without it)")
    }

    /// Verifies that context attachments are serialized into the message text.
    func testChatRequestEncodingIncludesContextAttachments() async throws {
        let transport = makeMockTransport()

        MockURLProtocol.requestHandler = { request in
            let sseBody = "data: {\"type\":\"Finish\",\"reason\":\"stop\"}\n\n"
            return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
        }

        let prompt = GoosePromptRequest(
            content: "Main task prompt",
            context: [
                GooseContextAttachment(type: "text", name: "workspace_context", content: "Run ID: 123\nStage: state_1", path: nil),
                GooseContextAttachment(type: "artifact", name: "input_artifact", content: "artifact content here", path: nil)
            ]
        )

        let stream = transport.submitPrompt(sessionID: "test-session-ctx", prompt: prompt)
        for try await _ in stream {}

        let requests = MockURLProtocol.recordedRequests()
        let replyRequest = requests.first { $0.url.contains("/reply") }

        guard let body = replyRequest?.body,
              let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
              let userMessage = json["user_message"] as? [String: Any],
              let content = userMessage["content"] as? [[String: Any]],
              let text = content.first?["text"] as? String else {
            XCTFail("Should have a valid /reply request with text content")
            return
        }

        // Context attachments should be embedded in the message text
        XCTAssertTrue(text.contains("Main task prompt"), "Should contain the main prompt")
        XCTAssertTrue(text.contains("workspace_context"), "Should contain workspace_context attachment")
        XCTAssertTrue(text.contains("input_artifact"), "Should contain input_artifact attachment")
        XCTAssertTrue(text.contains("artifact content here"), "Should contain attachment content")
    }

    /// Verifies that the session system prompt is embedded into the /reply payload for goosed.
    func testChatRequestEncodingIncludesSessionSystemPrompt() async throws {
        let transport = makeMockTransport()

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.path ?? ""
            if url.hasSuffix("/agent/start") {
                let response = #"{"id":"prompt-session-1","working_dir":"/tmp/test","name":"Prompt Session"}"#
                return (200, ["Content-Type": "application/json"], Data(response.utf8))
            }
            if url.hasSuffix("/reply") {
                let sseBody = "data: {\"type\":\"Finish\",\"reason\":\"stop\"}\n\n"
                return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
            }
            return (200, ["Content-Type": "application/json"], Data("{}".utf8))
        }

        let session = try await transport.createSession(request: GooseSessionRequest(
            systemPrompt: "Do not call xcode_mcp.",
            workingDirectory: "/tmp/test-workspace",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: nil
        ))

        let stream = transport.submitPrompt(
            sessionID: session.sessionId,
            prompt: GoosePromptRequest(content: "Reply with exactly ok", context: nil)
        )
        for try await _ in stream {}

        let requests = MockURLProtocol.recordedRequests()
        let replyRequest = requests.first { $0.url.contains("/reply") }

        guard let body = replyRequest?.body,
              let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
              let userMessage = json["user_message"] as? [String: Any],
              let content = userMessage["content"] as? [[String: Any]],
              let text = content.first?["text"] as? String else {
            XCTFail("Should have a valid /reply request with text content")
            return
        }

        XCTAssertTrue(text.contains("Do not call xcode_mcp."),
                      "System prompt must be embedded in the goosed /reply payload")
        XCTAssertTrue(text.contains("Reply with exactly ok"))
    }

    /// Verifies that X-Secret-Key header is used (not Authorization: Bearer).
    func testSecretKeyHeaderUsedNotBearerAuth() async throws {
        let transport = makeMockTransport(secretKey: "my-test-secret")

        MockURLProtocol.requestHandler = { request in
            let sseBody = "data: {\"type\":\"Finish\",\"reason\":\"stop\"}\n\n"
            return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
        }

        let prompt = GoosePromptRequest(content: "test", context: nil)
        let stream = transport.submitPrompt(sessionID: "test-session-auth", prompt: prompt)
        for try await _ in stream {}

        let requests = MockURLProtocol.recordedRequests()
        let replyRequest = requests.first { $0.url.contains("/reply") }

        XCTAssertEqual(replyRequest?.headers["X-Secret-Key"], "my-test-secret",
                       "Should use X-Secret-Key header")
        XCTAssertNil(replyRequest?.headers["Authorization"],
                     "Should NOT use Authorization header (goosed uses X-Secret-Key)")
    }

    // MARK: - REQ-011: Mock-HTTP Integration Test — Full Round-Trip

    /// Full round-trip integration test: createSession → (update_provider) → submitPrompt → closeSession.
    /// Uses MockURLProtocol to simulate goosed responses without a real server.
    func testFullRoundTripCreateSubmitClose() async throws {
        let transport = makeMockTransport(secretKey: "integration-secret", provider: "claude-code", model: "test-model")

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.path ?? ""

            if url.hasSuffix("/agent/start") {
                // Simulate goosed /agent/start response
                let response = """
                {"id":"mock-session-42","working_dir":"/tmp/test","name":"New Session"}
                """
                return (200, ["Content-Type": "application/json"], Data(response.utf8))
            }

            if url.hasSuffix("/agent/update_provider") {
                return (200, ["Content-Type": "application/json"], Data("{}".utf8))
            }

            if url.hasSuffix("/reply") {
                // Simulate goosed SSE stream: Ping → Message → Finish
                let sseBody = """
                data: {"type":"Ping"}

                data: {"type":"Message","message":{"role":"assistant","content":[{"type":"text","text":"Hello from mock goosed"}]},"token_state":{"total_tokens":15}}

                data: {"type":"Finish","reason":"stop","token_state":{"total_tokens":15}}

                """
                return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
            }

            if url.contains("/sessions/") && request.httpMethod == "DELETE" {
                return (200, ["Content-Type": "application/json"], Data("{}".utf8))
            }

            return (404, [:], Data("Not Found".utf8))
        }

        // Step 1: Create session
        let sessionRequest = GooseSessionRequest(
            systemPrompt: "You are a test agent.",
            workingDirectory: "/tmp/test-workspace",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: nil
        )
        let sessionResponse = try await transport.createSession(request: sessionRequest)

        XCTAssertEqual(sessionResponse.sessionId, "mock-session-42",
                       "Session ID should come from the mock /agent/start response")
        XCTAssertEqual(sessionResponse.policyAcknowledgement?.accepted, true,
                       "GooseServerTransport synthesizes policy acknowledgement")

        // Verify the request sequence so far
        let requestsSoFar = MockURLProtocol.recordedRequests()
        let startRequest = requestsSoFar.first { $0.url.contains("/agent/start") }
        XCTAssertNotNil(startRequest, "Should have called /agent/start")
        XCTAssertEqual(startRequest?.method, "POST")
        XCTAssertEqual(startRequest?.headers["X-Secret-Key"], "integration-secret")

        // Verify /agent/start body contains working_dir
        if let startBody = startRequest?.body,
           let startJSON = try? JSONSerialization.jsonObject(with: startBody) as? [String: Any] {
            XCTAssertEqual(startJSON["working_dir"] as? String, "/tmp/test-workspace")
        } else {
            XCTFail("Start request should have JSON body with working_dir")
        }

        let providerRequest = requestsSoFar.first { $0.url.contains("/agent/update_provider") }
        XCTAssertNotNil(providerRequest, "Should have called /agent/update_provider")
        XCTAssertEqual(providerRequest?.method, "POST")

        // Verify /agent/update_provider body
        if let providerBody = providerRequest?.body,
           let providerJSON = try? JSONSerialization.jsonObject(with: providerBody) as? [String: Any] {
            XCTAssertEqual(providerJSON["session_id"] as? String, "mock-session-42")
            XCTAssertEqual(providerJSON["provider"] as? String, "claude-code")
            XCTAssertEqual(providerJSON["model"] as? String, "test-model")
        } else {
            XCTFail("Provider request should have JSON body with session_id, provider, model")
        }

        // Step 2: Submit prompt and collect events
        let promptRequest = GoosePromptRequest(
            content: "Write a hello world program",
            context: nil
        )
        let eventStream = transport.submitPrompt(sessionID: sessionResponse.sessionId, prompt: promptRequest)

        var events: [GooseStreamEvent] = []
        for try await event in eventStream {
            events.append(event)
        }

        // Verify events: should have sessionStarted, promptSubmitted, textChunk, finalOutput, sessionClosed
        // (Ping is ignored by mapper)
        let eventTypes = events.map { event -> String in
            switch event {
            case .sessionStarted: return "sessionStarted"
            case .promptSubmitted: return "promptSubmitted"
            case .textChunk: return "textChunk"
            case .finalOutput: return "finalOutput"
            case .sessionClosed: return "sessionClosed"
            case .toolCallStarted: return "toolCallStarted"
            case .toolCallFinished: return "toolCallFinished"
            case .error: return "error"
            case .unknown: return "unknown"
            }
        }

        XCTAssertTrue(eventTypes.contains("sessionStarted"), "Should emit sessionStarted")
        XCTAssertTrue(eventTypes.contains("promptSubmitted"), "Should emit promptSubmitted")
        XCTAssertTrue(eventTypes.contains("textChunk"), "Should emit textChunk from Message event")
        XCTAssertTrue(eventTypes.contains("finalOutput"), "Should emit finalOutput from Finish event")
        XCTAssertTrue(eventTypes.contains("sessionClosed"), "Should emit sessionClosed after Finish")

        // Verify the text content
        let textChunks = events.compactMap { event -> String? in
            if case .textChunk(let text) = event { return text }
            return nil
        }
        XCTAssertTrue(textChunks.contains("Hello from mock goosed"),
                      "Text chunk should contain the mock response")

        // Step 3: Close session
        do {
            try await transport.closeSession(sessionID: sessionResponse.sessionId)
        } catch {
            XCTFail("closeSession should not throw: \(error)")
        }

        // Verify DELETE was called
        let allRequests = MockURLProtocol.recordedRequests()
        let deleteRequest = allRequests.first { $0.method == "DELETE" && $0.url.contains("/sessions/") }
        XCTAssertNotNil(deleteRequest, "Should have called DELETE /sessions/{id}")
        XCTAssertTrue(deleteRequest?.url.contains("mock-session-42") == true,
                      "DELETE should target the correct session ID")
    }

    /// Tests that createSession fails gracefully when /agent/start returns an error.
    func testCreateSessionFailsOnServerError() async throws {
        let transport = makeMockTransport()

        MockURLProtocol.requestHandler = { _ in
            return (500, [:], Data("Internal Server Error".utf8))
        }

        let request = GooseSessionRequest(
            systemPrompt: "test",
            workingDirectory: "/tmp/test",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: nil
        )

        do {
            _ = try await transport.createSession(request: request)
            XCTFail("Should have thrown on 500 error")
        } catch let error as GooseTransportError {
            if case .httpError(let code, _) = error {
                XCTAssertEqual(code, 500)
            } else {
                XCTFail("Expected httpError, got \(error)")
            }
        }
    }

    /// Tests that SSE error events are properly mapped.
    func testSubmitPromptMapsErrorEvent() async throws {
        let transport = makeMockTransport()

        MockURLProtocol.requestHandler = { request in
            let sseBody = "data: {\"type\":\"Error\",\"error\":\"Provider not set\"}\n\n"
            return (200, ["Content-Type": "text/event-stream"], Data(sseBody.utf8))
        }

        let prompt = GoosePromptRequest(content: "test", context: nil)
        let stream = transport.submitPrompt(sessionID: "error-session", prompt: prompt)

        var events: [GooseStreamEvent] = []
        for try await event in stream {
            events.append(event)
        }

        let errorEvents = events.compactMap { event -> String? in
            if case .error(let msg) = event { return msg }
            return nil
        }
        XCTAssertTrue(errorEvents.contains("Provider not set"),
                      "Should map Error event with the correct message")
    }

    // MARK: - Fixture Transport Unchanged

    func testFixtureTransportCreateSession() async throws {
        let transport = FixtureGooseTransport(scenario: .proposalLoopSuccess)
        let request = GooseSessionRequest(
            systemPrompt: "Test prompt",
            workingDirectory: "/tmp/test",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: nil
        )

        let response = try await transport.createSession(request: request)
        XCTAssertTrue(response.sessionId.hasPrefix("fixture-"))
        XCTAssertEqual(response.policyAcknowledgement?.accepted, true)
    }

    func testFixtureTransportSubmitPrompt() async throws {
        let transport = FixtureGooseTransport(scenario: .proposalLoopSuccess)

        let request = GooseSessionRequest(
            systemPrompt: "Test prompt",
            workingDirectory: "/tmp/test",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: ["agent_id": "test_agent"]
        )
        let session = try await transport.createSession(request: request)

        let promptRequest = GoosePromptRequest(
            content: "## Task: normalize_idea_and_prepare_proposal_brief\n\n### Expected Outputs\n- idea_brief\n\nOutput directory: /tmp/test-output/",
            context: nil
        )
        let stream = transport.submitPrompt(sessionID: session.sessionId, prompt: promptRequest)

        var events: [GooseStreamEvent] = []
        for try await event in stream {
            events.append(event)
        }

        XCTAssertTrue(events.count >= 5, "Fixture should produce multiple events")

        let hasSessionStarted = events.contains { if case .sessionStarted = $0 { return true }; return false }
        let hasFinalOutput = events.contains { if case .finalOutput = $0 { return true }; return false }
        let hasSessionClosed = events.contains { if case .sessionClosed = $0 { return true }; return false }

        XCTAssertTrue(hasSessionStarted)
        XCTAssertTrue(hasFinalOutput)
        XCTAssertTrue(hasSessionClosed)
    }

    func testFixtureTransportCloseSession() async throws {
        let transport = FixtureGooseTransport(scenario: .proposalLoopSuccess)
        do {
            try await transport.closeSession(sessionID: "fixture-test")
        } catch {
            XCTFail("closeSession should not throw: \(error)")
        }
    }
}
