import Foundation
import os
@testable import Chainworks_Forge

// MARK: - Shared Mock Objects
//
// Two-lane test double strategy for GooseTransportProtocol:
// - Lane A (StubGooseTransport): lightweight Sendable struct for stimulus-only tests
// - Lane B (ObservableGooseTransport): actor-backed observable for side-effect assertions

// MARK: - Lane A: StubGooseTransport (lightweight value witness)

/// Lightweight struct stub for tests that only need stimulus injection (pre-configured
/// responses and event streams) and do NOT assert on transport-side effects.
///
/// Applicable to: GooseStreamEventMapperTests, SimulatedAgentExecutorTests,
/// stream-only tests in GooseServerTransportTests, EndToEndTests, and any
/// new test that does not need observation.
struct StubGooseTransport: GooseTransportProtocol, Sendable {
    var onCreateSession: @Sendable (GooseSessionRequest) async throws -> GooseSessionResponse = { _ in
        GooseSessionResponse(
            sessionId: "stub-\(UUID().uuidString.prefix(8))",
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true, capabilityToken: "stub", backendPolicyVersion: "v1"
            )
        )
    }
    var events: [GooseStreamEvent] = []

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        try await onCreateSession(request)
    }

    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        let events = self.events
        return AsyncThrowingStream { c in
            Task { for e in events { c.yield(e) }; c.finish() }
        }
    }

    func closeSession(sessionID: String) async throws {}
}

// MARK: - Lane B: ObservableGooseTransport (actor-backed observable mock)

/// Actor-backed observable mock for tests that need to assert on request content,
/// session lifecycle, and call counts after execution.
///
/// Applicable to: GooseAgentExecutorTests, GooseSessionBridgeTests, OrchestratorTests,
/// and session-lifecycle tests in GooseServerTransportTests.
///
/// Key difference from SharedMockGooseTransport: `actor` provides compiler-verified
/// Sendable safety without `@unchecked`. Observable state is accessed via `await`
/// from tests, which is natural in async test functions.
actor ObservableGooseTransport: GooseTransportProtocol {
    // Stimulus configuration
    var createSessionResult: GooseSessionResponse?
    var createSessionError: Error?
    var streamEvents: [GooseStreamEvent] = []

    // Observable state
    private(set) var closeSessionCalled = false
    private(set) var lastSessionID: String?
    private(set) var lastSessionRequest: GooseSessionRequest?
    private(set) var createSessionCallCount = 0
    private(set) var submitPromptCallCount = 0

    /// Convenience configuration method for test setup.
    func configure(
        sessionResult: GooseSessionResponse? = nil,
        sessionError: Error? = nil,
        events: [GooseStreamEvent] = []
    ) {
        self.createSessionResult = sessionResult
        self.createSessionError = sessionError
        self.streamEvents = events
    }

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        createSessionCallCount += 1
        lastSessionRequest = request
        if let error = createSessionError { throw error }
        return createSessionResult ?? GooseSessionResponse(
            sessionId: "obs-\(UUID().uuidString.prefix(8))",
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true, capabilityToken: "obs", backendPolicyVersion: "v1"
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
        return AsyncThrowingStream { c in
            Task { for e in events { c.yield(e) }; c.finish() }
        }
    }

    func closeSession(sessionID: String) async throws {
        closeSessionCalled = true
    }

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

/// Thread-safe collector for execution events (compiler-verified Sendable via OSAllocatedUnfairLock).
/// Use in GooseAgentExecutor tests to avoid unsafe mutation of captured vars in @Sendable closures.
/// Replaces the former `@unchecked Sendable` class per TEST-004.
final class SharedEventCollector: Sendable {
    private let storage = OSAllocatedUnfairLock(initialState: [ExecutionEvent]())

    func append(_ event: ExecutionEvent) {
        storage.withLock { $0.append(event) }
    }

    var events: [ExecutionEvent] {
        storage.withLock { Array($0) }
    }

    var count: Int {
        storage.withLock { $0.count }
    }
}
