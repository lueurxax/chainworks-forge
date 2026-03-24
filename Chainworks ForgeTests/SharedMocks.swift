import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - Shared Mock Objects
//
// Consolidated test doubles extracted from OrchestratorTests and GooseAgentExecutorTests.
// Prevents duplicate MockGooseTransport implementations with divergent behavior.

// MARK: - MockGooseTransportProtocol

/// Protocol-conformant mock transport for GooseAgentExecutor tests.
/// Tracks session lifecycle and returns pre-configured responses without real HTTP.
///
/// Consolidated from:
///   - OrchestratorTests.MockGooseTransport (subclass-based)
///   - GooseAgentExecutorTests.MockGooseTransport (protocol-based)
final class SharedMockGooseTransport: GooseTransportProtocol, @unchecked Sendable {
    // MARK: - Configuration
    var createSessionResult: GooseSessionResponse?
    var createSessionError: Error?
    var streamEvents: [GooseStreamEvent] = []

    // MARK: - Observation
    private(set) var closeSessionCalled = false
    private(set) var lastSessionID: String?
    private(set) var lastSessionRequest: GooseSessionRequest?
    private(set) var createSessionCallCount = 0
    private(set) var submitPromptCallCount = 0

    init() {}

    /// Convenience init for common "happy path" scenario.
    convenience init(events: [GooseStreamEvent]) {
        self.init()
        self.streamEvents = events
    }

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        createSessionCallCount += 1
        lastSessionRequest = request
        if let error = createSessionError {
            throw error
        }
        return createSessionResult ?? GooseSessionResponse(
            sessionId: "mock-session-\(UUID().uuidString.prefix(8))",
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true,
                capabilityToken: "mock-read-only",
                backendPolicyVersion: "mock-v1"
            )
        )
    }

    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        submitPromptCallCount += 1
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

    func closeSession(sessionID: String) async throws {
        closeSessionCalled = true
    }

    /// Resets all observation state for reuse across tests.
    func reset() {
        closeSessionCalled = false
        lastSessionID = nil
        lastSessionRequest = nil
        createSessionCallCount = 0
        submitPromptCallCount = 0
    }
}

// MARK: - StaticResultExecutor

/// An executor that always returns a pre-configured result.
/// Useful for testing orchestrator behavior with deterministic agent output.
struct SharedStaticResultExecutor: AgentExecutor {
    let result: AgentResult

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        result
    }
}

// MARK: - Thread-Safe Event Collector

/// Thread-safe collector for execution events.
/// Use in GooseAgentExecutor tests to avoid unsafe mutation of captured vars in @Sendable closures.
final class SharedEventCollector: @unchecked Sendable {
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

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return _events.count
    }
}
