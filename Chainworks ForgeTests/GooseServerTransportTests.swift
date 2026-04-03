import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseServerTransportTests (Proposal 005, Section 8)

/// Unit and integration tests for GooseServerTransport.
/// Covers initialization, protocol conformance, ChatRequest encoding (REQ-010),
/// and full mock-HTTP round-trip (REQ-011).
@Suite("GooseServerTransport", .serialized)
struct GooseServerTransportTests {

    // MARK: - MockURLProtocol

    /// URLProtocol subclass that intercepts HTTP requests for testing.
    /// Records all requests and returns pre-configured responses.
    ///
    /// `@unchecked Sendable` is required and cannot be eliminated per TEST-004:
    /// `URLProtocol` is an ObjC class requiring subclassing (no struct/actor
    /// alternative exists), and the URL loading system calls instances from
    /// arbitrary threads. Static state is manually synchronized via `logQueue`.
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
        model: String? = "default",
        diagnosticsSink: @escaping @Sendable (GooseServerTransportDiagnosticEvent) -> Void = { _ in }
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
            sessionConfiguration: config,
            diagnosticsSink: diagnosticsSink
        )
    }

    init() {
        MockURLProtocol.reset()
    }

    // MARK: - Transport API Selection

    @Test("GooseTransportAPI raw values match expected strings")
    func gooseTransportAPIRawValues() {
        #expect(GooseTransportAPI.bespoke.rawValue == "bespoke")
        #expect(GooseTransportAPI.gooseServer.rawValue == "goose_server")
    }

    @Test("GooseTransportAPI initializes from valid raw values and returns nil for invalid")
    func gooseTransportAPIFromRawValue() {
        #expect(GooseTransportAPI(rawValue: "bespoke") == .bespoke)
        #expect(GooseTransportAPI(rawValue: "goose_server") == .gooseServer)
        #expect(GooseTransportAPI(rawValue: "invalid") == nil)
    }

    // MARK: - GooseServerTransport Initialization

    @Test("GooseServerTransport stores initialization parameters correctly")
    func serverTransportInitialization() {
        let transport = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            secretKey: "test-secret",
            provider: "claude-code",
            model: "default"
        )

        #expect(transport.baseURL.absoluteString == "https://127.0.0.1:51200")
        #expect(transport.secretKey == "test-secret")
        #expect(transport.provider == "claude-code")
        #expect(transport.model == "default")
        #expect(transport.requestTimeout == 900)
    }

    @Test("GooseServerTransport normalizes internal claude_code identifier for goosed provider binding")
    func serverTransportNormalizesClaudeProviderForGoosed() async throws {
        let transport = makeMockTransport(provider: "claude_code", model: "opus")

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.absoluteString ?? ""
            if url.hasSuffix("/agent/start") {
                let body = #"{"id":"mock-session-claude","working_dir":"/tmp/test-workspace"}"#
                return (200, ["Content-Type": "application/json"], Data(body.utf8))
            }
            if url.hasSuffix("/agent/update_provider") {
                return (200, ["Content-Type": "application/json"], Data("{}".utf8))
            }
            return (404, [:], Data())
        }

        _ = try await transport.createSession(
            request: GooseSessionRequest(
                systemPrompt: "You are a test agent.",
                workingDirectory: "/tmp/test-workspace",
                model: nil,
                provider: nil,
                executionPolicy: nil,
                metadata: nil
            )
        )

        let requests = MockURLProtocol.recordedRequests()
        let providerRequest = try #require(
            requests.first { $0.url.contains("/agent/update_provider") },
            "Should call /agent/update_provider"
        )
        let providerBody = try #require(providerRequest.body, "Provider request should have a body")
        let providerJSON = try #require(
            try JSONSerialization.jsonObject(with: providerBody) as? [String: Any],
            "Provider request should contain JSON"
        )
        #expect(providerJSON["provider"] as? String == "claude-code")
        #expect(providerJSON["model"] as? String == "opus")
    }

    @Test("GooseServerTransport normalizes internal gemini identifier for goosed provider binding")
    func serverTransportNormalizesGeminiProviderForGoosed() async throws {
        let transport = makeMockTransport(provider: "gemini", model: "gemini-2.5-pro")

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.absoluteString ?? ""
            if url.hasSuffix("/agent/start") {
                let body = #"{"id":"mock-session-gemini","working_dir":"/tmp/test-workspace"}"#
                return (200, ["Content-Type": "application/json"], Data(body.utf8))
            }
            if url.hasSuffix("/agent/update_provider") {
                return (200, ["Content-Type": "application/json"], Data("{}".utf8))
            }
            return (404, [:], Data())
        }

        _ = try await transport.createSession(
            request: GooseSessionRequest(
                systemPrompt: "You are a test agent.",
                workingDirectory: "/tmp/test-workspace",
                model: nil,
                provider: nil,
                executionPolicy: nil,
                metadata: nil
            )
        )

        let requests = MockURLProtocol.recordedRequests()
        let providerRequest = try #require(
            requests.first { $0.url.contains("/agent/update_provider") },
            "Should call /agent/update_provider"
        )
        let providerBody = try #require(providerRequest.body, "Provider request should have a body")
        let providerJSON = try #require(
            try JSONSerialization.jsonObject(with: providerBody) as? [String: Any],
            "Provider request should contain JSON"
        )
        #expect(providerJSON["provider"] as? String == "gemini-cli")
        #expect(providerJSON["model"] as? String == "gemini-2.5-pro")
    }

    @Test("GooseServerTransport removes all session extensions after session creation")
    func serverTransportRemovesAllSessionExtensionsAfterCreation() async throws {
        let transport = makeMockTransport(provider: "claude_code", model: "opus")

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.absoluteString ?? ""
            if url.hasSuffix("/agent/start") {
                let body = """
                {
                  "id": "mock-session-exts",
                  "working_dir": "/tmp/test-workspace",
                  "extension_data": {
                    "enabled_extensions.v0": {
                      "extensions": [
                        { "type": "stdio", "name": "xcode", "description": "xcode mcp server" },
                        { "type": "platform", "name": "developer", "description": "Write and edit files" },
                        { "type": "platform", "name": "Extension Manager", "description": "Enable and disable extensions" }
                      ]
                    }
                  }
                }
                """
                return (200, ["Content-Type": "application/json"], Data(body.utf8))
            }
            if url.hasSuffix("/agent/update_provider") {
                return (200, ["Content-Type": "application/json"], Data("{}".utf8))
            }
            if url.hasSuffix("/agent/remove_extension") {
                return (200, ["Content-Type": "text/plain"], Data("ok".utf8))
            }
            return (404, [:], Data())
        }

        _ = try await transport.createSession(
            request: GooseSessionRequest(
                systemPrompt: "You are a test agent.",
                workingDirectory: "/tmp/test-workspace",
                model: nil,
                provider: nil,
                executionPolicy: nil,
                metadata: nil
            )
        )

        let requests = MockURLProtocol.recordedRequests()
        let removeRequests = requests.filter { $0.url.contains("/agent/remove_extension") }
        #expect(removeRequests.count == 3)

        let removedNames = try removeRequests.map { request in
            let body = try #require(request.body, "Remove extension request should have a body")
            let json = try #require(
                try JSONSerialization.jsonObject(with: body) as? [String: Any],
                "Remove extension request should contain JSON"
            )
            return try #require(json["name"] as? String, "Remove extension request should contain name")
        }

        #expect(Set(removedNames) == Set(["xcode", "developer", "Extension Manager"]))
    }

    @Test("GooseServerTransport accepts custom timeout")
    func serverTransportCustomTimeout() {
        let transport = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            requestTimeout: 600
        )
        #expect(transport.requestTimeout == 600)
    }

    // MARK: - Protocol Conformance

    @Test("GooseServerTransport conforms to GooseTransportProtocol")
    func serverTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = GooseServerTransport(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            secretKey: "test-secret"
        )
        #expect(transport != nil)
    }

    @Test("GooseTransport (bespoke) conforms to GooseTransportProtocol")
    func bespokeTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = GooseTransport(
            baseURL: URL(string: "http://localhost:3000")!,
            apiKey: "test-key"
        )
        #expect(transport != nil)
    }

    @Test("FixtureGooseTransport conforms to GooseTransportProtocol")
    func fixtureTransportConformsToProtocol() {
        let transport: any GooseTransportProtocol = FixtureGooseTransport(
            scenario: .proposalLoopSuccess
        )
        #expect(transport != nil)
    }

    // MARK: - LiveRuntimeConfiguration with transportAPI

    @Test("LiveRuntimeConfiguration with gooseServer transport API")
    func liveRuntimeConfigurationWithGooseServer() {
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

        #expect(config.transportAPI == .gooseServer)
        #expect(config.sourceDescription.contains("goosed"))
    }

    @Test("LiveRuntimeConfiguration with bespoke transport API")
    func liveRuntimeConfigurationWithBespoke() {
        let config = LiveRuntimeConfiguration(
            baseURL: URL(string: "http://localhost:3000")!,
            apiKey: "test-key",
            override: nil,
            transportMode: .network,
            transportAPI: .bespoke
        )

        #expect(config.transportAPI == .bespoke)
        #expect(config.sourceDescription.contains("bespoke"))
    }

    @Test("LiveRuntimeConfiguration in fixture mode")
    func liveRuntimeConfigurationFixtureMode() {
        let config = LiveRuntimeConfiguration(
            baseURL: URL(string: "http://fixture.local")!,
            apiKey: nil,
            override: nil,
            transportMode: .fixtureProposalLoopSuccess,
            transportAPI: .bespoke
        )

        #expect(config.sourceDescription.contains("Fixture"))
    }

    // MARK: - REQ-010: ChatRequest Encoding Unit Tests

    /// Verifies that submitPrompt() encodes metadata.userVisible and metadata.agentVisible
    /// in the /reply request body (Proposal 005, Section 4.3 — required fields).
    @Test("ChatRequest encoding contains required metadata fields (REQ-010)")
    func chatRequestEncodingContainsRequiredMetadata() async throws {
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
        let unwrappedReply = try #require(replyRequest, "Should have sent a POST /reply request")

        let body = try #require(unwrappedReply.body, "Request should have a body")
        let json = try #require(
            try JSONSerialization.jsonObject(with: body) as? [String: Any],
            "Request body should be valid JSON"
        )

        // Verify session_id
        #expect(json["session_id"] as? String == "test-session-1")

        // Verify user_message structure
        let userMessage = try #require(
            json["user_message"] as? [String: Any],
            "Request body should contain user_message"
        )

        #expect(userMessage["role"] as? String == "user")
        #expect(userMessage["created"] as? Int != nil, "created must be a Unix timestamp integer")

        // Verify content array
        let content = try #require(
            userMessage["content"] as? [[String: Any]],
            "user_message.content should be a non-empty array"
        )
        let firstContent = try #require(content.first, "user_message.content should be a non-empty array")
        #expect(firstContent["type"] as? String == "text")
        #expect((firstContent["text"] as? String)?.contains("Test prompt for encoding verification") == true)

        // REQ-010 critical assertion: metadata.userVisible and metadata.agentVisible are REQUIRED
        let metadata = try #require(
            userMessage["metadata"] as? [String: Any],
            "user_message.metadata must be present (server returns 422 without it)"
        )
        #expect(metadata["userVisible"] as? Bool == true,
                "metadata.userVisible must be true (required by goosed, 422 without it)")
        #expect(metadata["agentVisible"] as? Bool == true,
                "metadata.agentVisible must be true (required by goosed, 422 without it)")
    }

    /// Verifies that context attachments are serialized into the message text.
    @Test("ChatRequest encoding includes context attachments")
    func chatRequestEncodingIncludesContextAttachments() async throws {
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

        let body = try #require(replyRequest?.body, "Should have a valid /reply request body")
        let json = try #require(
            try JSONSerialization.jsonObject(with: body) as? [String: Any],
            "Should have valid JSON body"
        )
        let userMessage = try #require(json["user_message"] as? [String: Any], "Should have user_message")
        let content = try #require(userMessage["content"] as? [[String: Any]], "Should have content array")
        let text = try #require(content.first?["text"] as? String, "Should have text content")

        // Context attachments should be embedded in the message text
        #expect(text.contains("Main task prompt"), "Should contain the main prompt")
        #expect(text.contains("workspace_context"), "Should contain workspace_context attachment")
        #expect(text.contains("input_artifact"), "Should contain input_artifact attachment")
        #expect(text.contains("artifact content here"), "Should contain attachment content")
    }

    /// Verifies that the session system prompt is embedded into the /reply payload for goosed.
    @Test("ChatRequest encoding includes session system prompt")
    func chatRequestEncodingIncludesSessionSystemPrompt() async throws {
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

        let body = try #require(replyRequest?.body, "Should have a valid /reply request body")
        let json = try #require(
            try JSONSerialization.jsonObject(with: body) as? [String: Any],
            "Should have valid JSON body"
        )
        let userMessage = try #require(json["user_message"] as? [String: Any], "Should have user_message")
        let content = try #require(userMessage["content"] as? [[String: Any]], "Should have content array")
        let text = try #require(content.first?["text"] as? String, "Should have text content")

        #expect(text.contains("Do not call xcode_mcp."),
                "System prompt must be embedded in the goosed /reply payload")
        #expect(text.contains("Reply with exactly ok"))
    }

    /// Verifies that X-Secret-Key header is used (not Authorization: Bearer).
    @Test("Secret key header is used instead of Bearer auth")
    func secretKeyHeaderUsedNotBearerAuth() async throws {
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

        #expect(replyRequest?.headers["X-Secret-Key"] == "my-test-secret",
                "Should use X-Secret-Key header")
        #expect(replyRequest?.headers["Authorization"] == nil,
                "Should NOT use Authorization header (goosed uses X-Secret-Key)")
    }

    // MARK: - REQ-011: Mock-HTTP Integration Test — Full Round-Trip

    /// Full round-trip integration test: createSession -> (update_provider) -> submitPrompt -> closeSession.
    /// Uses MockURLProtocol to simulate goosed responses without a real server.
    @Test("Full round-trip: createSession, submitPrompt, closeSession (REQ-011)")
    func fullRoundTripCreateSubmitClose() async throws {
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
                // Simulate goosed SSE stream: Ping -> Message -> Finish
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

        #expect(sessionResponse.sessionId == "mock-session-42",
                "Session ID should come from the mock /agent/start response")
        #expect(sessionResponse.policyAcknowledgement?.accepted == true,
                "GooseServerTransport synthesizes policy acknowledgement")

        // Verify the request sequence so far
        let requestsSoFar = MockURLProtocol.recordedRequests()
        let startRequest = requestsSoFar.first { $0.url.contains("/agent/start") }
        let unwrappedStart = try #require(startRequest, "Should have called /agent/start")
        #expect(unwrappedStart.method == "POST")
        #expect(unwrappedStart.headers["X-Secret-Key"] == "integration-secret")

        // Verify /agent/start body contains working_dir
        let startBody = try #require(unwrappedStart.body, "Start request should have a body")
        let startJSON = try #require(
            try JSONSerialization.jsonObject(with: startBody) as? [String: Any],
            "Start request should have JSON body with working_dir"
        )
        #expect(startJSON["working_dir"] as? String == "/tmp/test-workspace")

        let providerRequest = requestsSoFar.first { $0.url.contains("/agent/update_provider") }
        let unwrappedProvider = try #require(providerRequest, "Should have called /agent/update_provider")
        #expect(unwrappedProvider.method == "POST")

        // Verify /agent/update_provider body
        let providerBody = try #require(unwrappedProvider.body, "Provider request should have a body")
        let providerJSON = try #require(
            try JSONSerialization.jsonObject(with: providerBody) as? [String: Any],
            "Provider request should have JSON body with session_id, provider, model"
        )
        #expect(providerJSON["session_id"] as? String == "mock-session-42")
        #expect(providerJSON["provider"] as? String == "claude-code")
        #expect(providerJSON["model"] as? String == "test-model")

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

        // Verify events: should have sessionStarted, promptSubmitted, textChunk, finish, sessionClosed.
        // Some mocked Finish payloads do not carry a separate finalOutput event.
        // (Ping is ignored by mapper.)
        let eventTypes = events.map { event -> String in
            switch event {
            case .sessionStarted: return "sessionStarted"
            case .promptSubmitted: return "promptSubmitted"
            case .textChunk: return "textChunk"
            case .finalOutput: return "finalOutput"
            case .finish: return "finish"
            case .sessionClosed: return "sessionClosed"
            case .toolCallStarted: return "toolCallStarted"
            case .toolCallFinished: return "toolCallFinished"
            case .error: return "error"
            case .unknown: return "unknown"
            }
        }

        #expect(eventTypes.contains("sessionStarted"), "Should emit sessionStarted")
        #expect(eventTypes.contains("promptSubmitted"), "Should emit promptSubmitted")
        #expect(eventTypes.contains("textChunk"), "Should emit textChunk from Message event")
        #expect(
            eventTypes.contains("finalOutput") || eventTypes.contains("finish"),
            "Should emit terminal output or finish event"
        )
        #expect(eventTypes.contains("sessionClosed"), "Should emit sessionClosed after Finish")

        // Verify the text content
        let textChunks = events.compactMap { event -> String? in
            if case .textChunk(let text) = event { return text }
            return nil
        }
        #expect(textChunks.contains("Hello from mock goosed"),
                "Text chunk should contain the mock response")

        // Step 3: Close session
        try await transport.closeSession(sessionID: sessionResponse.sessionId)

        // Verify DELETE was called
        let allRequests = MockURLProtocol.recordedRequests()
        let deleteRequest = allRequests.first { $0.method == "DELETE" && $0.url.contains("/sessions/") }
        let unwrappedDelete = try #require(deleteRequest, "Should have called DELETE /sessions/{id}")
        #expect(unwrappedDelete.url.contains("mock-session-42"),
                "DELETE should target the correct session ID")
    }

    /// Tests that createSession fails gracefully when /agent/start returns an error.
    @Test("createSession fails on server error with httpError")
    func createSessionFailsOnServerError() async throws {
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

        await #expect {
            _ = try await transport.createSession(request: request)
        } throws: { error in
            guard let transportError = error as? GooseTransportError,
                  case .httpError(let code, _) = transportError else {
                return false
            }
            return code == 500
        }
    }

    @Test("createSession emits transport diagnostics for start and provider binding")
    func createSessionEmitsTransportDiagnostics() async throws {
        let recorder = LockedDiagnosticsRecorder()
        let transport = makeMockTransport(
            secretKey: "diagnostic-secret",
            provider: "codex",
            model: "GPT-5.4",
            diagnosticsSink: { event in
                recorder.record(event)
            }
        )

        MockURLProtocol.requestHandler = { request in
            let url = request.url?.path ?? ""
            if url.hasSuffix("/agent/start") {
                let response = """
                {"id":"diagnostic-session-7","working_dir":"/tmp/diagnostic","name":"New Session"}
                """
                return (200, ["Content-Type": "application/json"], Data(response.utf8))
            }

            if url.hasSuffix("/agent/update_provider") {
                return (200, ["Content-Type": "application/json"], Data(#"{"status":"ok"}"#.utf8))
            }

            return (404, [:], Data("Not Found".utf8))
        }

        _ = try await transport.createSession(request: GooseSessionRequest(
            systemPrompt: "Diagnostic test agent",
            workingDirectory: "/tmp/diagnostic",
            model: nil,
            provider: nil,
            executionPolicy: nil,
            metadata: [
                "run_id": "run-123",
                "stage_id": "state_4_proposal_reviewed",
                "agent_id": "proposal_reviewer_architect"
            ]
        ))

        let events = recorder.events
        #expect(events.count == 2)

        let startEvent = try #require(events.first(where: { $0.kind == .agentStart }))
        #expect(startEvent.sessionID == "diagnostic-session-7")
        #expect(startEvent.httpStatus == 200)
        #expect(startEvent.runID == "run-123")
        #expect(startEvent.stageID == "state_4_proposal_reviewed")
        #expect(startEvent.agentID == "proposal_reviewer_architect")
        #expect(startEvent.workingDirectory == "/tmp/diagnostic")

        let providerEvent = try #require(events.first(where: { $0.kind == .updateProvider }))
        #expect(providerEvent.sessionID == "diagnostic-session-7")
        #expect(providerEvent.provider == "codex")
        #expect(providerEvent.model == "GPT-5.4")
        #expect(providerEvent.httpStatus == 200)
        #expect(providerEvent.agentID == "proposal_reviewer_architect")
    }

    /// Tests that SSE error events are properly mapped.
    @Test("submitPrompt maps SSE error events correctly")
    func submitPromptMapsErrorEvent() async throws {
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
        #expect(errorEvents.contains("Provider not set"),
                "Should map Error event with the correct message")
    }

    // MARK: - Fixture Transport Unchanged

    @Test("FixtureGooseTransport createSession returns fixture session")
    func fixtureTransportCreateSession() async throws {
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
        #expect(response.sessionId.hasPrefix("fixture-"))
        #expect(response.policyAcknowledgement?.accepted == true)
    }

    @Test("FixtureGooseTransport submitPrompt produces expected event sequence")
    func fixtureTransportSubmitPrompt() async throws {
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

        #expect(events.count >= 5, "Fixture should produce multiple events")

        let hasSessionStarted = events.contains { if case .sessionStarted = $0 { return true }; return false }
        let hasFinalOutput = events.contains { if case .finalOutput = $0 { return true }; return false }
        let hasSessionClosed = events.contains { if case .sessionClosed = $0 { return true }; return false }

        #expect(hasSessionStarted)
        #expect(hasFinalOutput)
        #expect(hasSessionClosed)
    }

    @Test("FixtureGooseTransport closeSession does not throw")
    func fixtureTransportCloseSession() async throws {
        let transport = FixtureGooseTransport(scenario: .proposalLoopSuccess)
        try await transport.closeSession(sessionID: "fixture-test")
    }
}

private final class LockedDiagnosticsRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [GooseServerTransportDiagnosticEvent] = []

    var events: [GooseServerTransportDiagnosticEvent] {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func record(_ event: GooseServerTransportDiagnosticEvent) {
        lock.lock()
        storage.append(event)
        lock.unlock()
    }
}
