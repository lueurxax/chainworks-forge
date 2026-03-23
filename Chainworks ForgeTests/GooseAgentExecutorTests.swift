import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseAgentExecutorTests (Proposal 004, Section 12.1)

/// Unit tests for GooseAgentExecutor.
/// Tests session creation, streaming, receipt persistence, output validation, and result building.
final class GooseAgentExecutorTests: XCTestCase {

    // MARK: - Thread-Safe Event Collector

    /// Thread-safe collector for execution events.
    /// Avoids unsafe mutation of captured vars in @Sendable closures.
    final class EventCollector: @unchecked Sendable {
        private let lock = NSLock()
        private var _events: [ExecutionEvent] = []

        func append(_ event: ExecutionEvent) {
            lock.lock()
            _events.append(event)
            lock.unlock()
        }

        var events: [ExecutionEvent] {
            lock.lock()
            defer { lock.unlock() }
            return _events
        }
    }

    // MARK: - Test Doubles

    /// Mock transport that returns pre-configured responses without real HTTP.
    final class MockGooseTransport: GooseTransport {
        var createSessionResult: GooseSessionResponse?
        var createSessionError: Error?
        var streamEvents: [GooseStreamEvent] = []
        var closeSessionCalled = false
        var lastSessionID: String?

        init() {
            super.init(baseURL: URL(string: "http://localhost:0")!)
        }

        override func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            if let error = createSessionError {
                throw error
            }
            return createSessionResult ?? GooseSessionResponse(
                sessionId: "test-session-\(UUID().uuidString.prefix(8))",
                status: "active"
            )
        }

        override func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
            lastSessionID = sessionID
            let events = streamEvents
            return AsyncThrowingStream { continuation in
                Task {
                    for event in events {
                        continuation.yield(event)
                    }
                    continuation.finish()
                }
            }
        }

        override func closeSession(sessionID: String) async throws {
            closeSessionCalled = true
        }
    }

    // MARK: - Helpers

    private func makeAgent(
        id: String = "test_agent",
        outputs: [String] = ["test_output.md"]
    ) -> ResolvedAgent {
        ResolvedAgent(
            id: id,
            title: "Test Agent",
            mode: "autonomous",
            provider: "test_provider",
            model: "test_model",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "You are a test agent.",
            outputContract: "test_contract",
            requiresHumanApproval: false,
            inputs: [],
            outputs: outputs
        )
    }

    private func makeTask(
        agent: String = "test_agent",
        task: String = "test_task"
    ) -> AgentTask {
        AgentTask(agent: agent, task: task, inputs: nil, outputs: nil)
    }

    private func makeContext(runID: UUID = UUID()) -> ExecutionContext {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("test-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)

        // Create directories
        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        return ExecutionContext(
            workspace: workspace,
            stageID: "state_1",
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea body"
        )
    }

    private func cleanupContext(_ context: ExecutionContext) {
        try? FileManager.default.removeItem(at: context.workspace.workspaceRoot)
    }

    // MARK: - Tests

    /// testGooseExecutorCreatesSession — Section 12.1
    @MainActor
    func testGooseExecutorCreatesSession() async throws {
        let mockTransport = MockGooseTransport()
        mockTransport.createSessionResult = GooseSessionResponse(
            sessionId: "session-abc123",
            status: "active"
        )
        mockTransport.streamEvents = [
            .sessionStarted(raw: "{}"),
            .finalOutput(content: "# Test Output\n\nThis is a test proposal."),
            .sessionClosed(raw: "{}")
        ]

        let executor = GooseAgentExecutor(transport: mockTransport)
        let agent = makeAgent(outputs: ["test_output.md"])
        let task = makeTask()
        let context = makeContext()
        defer { cleanupContext(context) }

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Session should have been created and closed
        XCTAssertTrue(mockTransport.closeSessionCalled, "Session should be closed after execution")
        // Result should contain receipt artifacts
        XCTAssertTrue(result.outputs.keys.contains(where: { $0.hasSuffix("_receipt.json") }),
                       "Result should contain receipt artifact")
        XCTAssertTrue(result.outputs.keys.contains(where: { $0.hasSuffix("_transcript.md") }),
                       "Result should contain transcript artifact")
    }

    /// testGooseExecutorStreamsEvents — Section 12.1
    @MainActor
    func testGooseExecutorStreamsEvents() async throws {
        let mockTransport = MockGooseTransport()
        mockTransport.streamEvents = [
            .sessionStarted(raw: "{}"),
            .promptSubmitted(raw: "{}"),
            .toolCallStarted(toolName: "write_file", raw: "{}"),
            .toolCallFinished(toolName: "write_file", raw: "{}"),
            .textChunk(text: "Working on it..."),
            .finalOutput(content: "Done!"),
            .sessionClosed(raw: "{}")
        ]

        // Use thread-safe collection to avoid concurrent mutation of captured var
        let eventCollector = EventCollector()
        let executor = GooseAgentExecutor(transport: mockTransport)
        executor.onExecutionEvent = { _, event in
            eventCollector.append(event)
        }

        let agent = makeAgent(outputs: ["test_output.md"])
        let task = makeTask()
        let context = makeContext()
        defer { cleanupContext(context) }

        _ = try await executor.execute(task: task, agent: agent, context: context)

        // Should have received multiple events
        let receivedEvents = eventCollector.events
        XCTAssertTrue(receivedEvents.count >= 5, "Should receive at least 5 events, got \(receivedEvents.count)")

        let eventTypes = receivedEvents.map(\.type)
        XCTAssertTrue(eventTypes.contains(.sessionStarted))
        XCTAssertTrue(eventTypes.contains(.promptSubmitted))
        XCTAssertTrue(eventTypes.contains(.toolCallStarted))
        XCTAssertTrue(eventTypes.contains(.finalOutput))
    }

    /// testGooseExecutorPersistsReceiptArtifact — Section 12.1
    @MainActor
    func testGooseExecutorPersistsReceiptArtifact() async throws {
        let mockTransport = MockGooseTransport()
        mockTransport.streamEvents = [
            .sessionStarted(raw: "{}"),
            .finalOutput(content: "Test output"),
            .sessionClosed(raw: "{}")
        ]

        let executor = GooseAgentExecutor(transport: mockTransport)
        let agent = makeAgent(id: "proposal_writer", outputs: ["proposal_current"])
        let task = makeTask(agent: "proposal_writer", task: "draft_initial_proposal")
        let context = makeContext()
        defer { cleanupContext(context) }

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Should have receipt artifacts
        let receiptKey = result.outputs.keys.first { $0.contains("_receipt.json") }
        XCTAssertNotNil(receiptKey, "Should produce a receipt artifact")

        if let key = receiptKey, let data = result.outputs[key] {
            // Parse the receipt JSON (must match encoder's .iso8601 date strategy)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let receipt = try decoder.decode(ExecutionReceipt.self, from: data)
            XCTAssertEqual(receipt.agentID, "proposal_writer")
            XCTAssertTrue(receipt.succeeded)
            XCTAssertEqual(receipt.receiptVersion, "1.0")
        }
    }

    /// testGooseExecutorFailsWhenRequiredOutputsMissing — Section 12.1
    @MainActor
    func testGooseExecutorFailsWhenRequiredOutputsMissing() async throws {
        let mockTransport = MockGooseTransport()
        // Stream completes but no files are written and no final output
        mockTransport.streamEvents = [
            .sessionStarted(raw: "{}"),
            .sessionClosed(raw: "{}")
        ]

        let executor = GooseAgentExecutor(transport: mockTransport)
        let agent = makeAgent(outputs: ["required_output.json"])
        let task = makeTask()
        let context = makeContext()
        defer { cleanupContext(context) }

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Should fail because required output is missing
        XCTAssertFalse(result.succeeded, "Should fail when required outputs are missing")
        XCTAssertNotNil(result.errorMessage)
        XCTAssertTrue(result.errorMessage?.contains("required_output.json") == true)
    }

    /// testGooseExecutorReturnsAgentResult — Section 12.1
    @MainActor
    func testGooseExecutorReturnsAgentResult() async throws {
        let mockTransport = MockGooseTransport()
        mockTransport.streamEvents = [
            .sessionStarted(raw: "{}"),
            .toolCallStarted(toolName: "read_file", raw: "{}"),
            .toolCallFinished(toolName: "read_file", raw: "{}"),
            .finalOutput(content: "# My Proposal\n\nThis is a great proposal."),
            .sessionClosed(raw: "{}")
        ]

        let executor = GooseAgentExecutor(transport: mockTransport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()
        defer { cleanupContext(context) }

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Final output should be used as primary output when no files written
        XCTAssertTrue(result.succeeded || result.outputs.keys.contains("proposal_current"),
                      "Result should either succeed or contain the primary output")
        XCTAssertNotNil(result.logSnippet, "Should have a log snippet")
        XCTAssertNotNil(result.costCents, "Should have a cost estimate")
    }

    /// testGooseExecutorSessionCreationFailure
    @MainActor
    func testGooseExecutorSessionCreationFailure() async throws {
        let mockTransport = MockGooseTransport()
        mockTransport.createSessionError = GooseTransportError.httpError(
            statusCode: 500,
            body: "Internal server error"
        )

        let executor = GooseAgentExecutor(transport: mockTransport)
        let agent = makeAgent()
        let task = makeTask()
        let context = makeContext()
        defer { cleanupContext(context) }

        let result = try await executor.execute(task: task, agent: agent, context: context)

        XCTAssertFalse(result.succeeded)
        XCTAssertTrue(result.errorMessage?.contains("Session creation failed") == true)
    }
}
