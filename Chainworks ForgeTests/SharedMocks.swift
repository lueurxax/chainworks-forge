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

/// Lock-backed observable mock for tests that need to assert on request content,
/// session lifecycle, and call counts after execution.
///
/// Applicable to: GooseAgentExecutorTests, GooseSessionBridgeTests, OrchestratorTests,
/// and session-lifecycle tests in GooseServerTransportTests.
///
/// Keep the surface async-friendly for tests, but avoid actor/protocol isolation
/// mismatches with the synchronous `submitPrompt` requirement.
final class ObservableGooseTransport: GooseTransportProtocol, @unchecked Sendable {
    private struct State {
        var createSessionResult: GooseSessionResponse?
        var createSessionError: Error?
        var streamEvents: [GooseStreamEvent] = []
        var closeSessionCalled = false
        var lastSessionID: String?
        var lastSessionRequest: GooseSessionRequest?
        var createSessionCallCount = 0
        var submitPromptCallCount = 0
    }

    private let state = OSAllocatedUnfairLock(initialState: State())

    /// Convenience configuration method for test setup.
    func configure(
        sessionResult: GooseSessionResponse? = nil,
        sessionError: Error? = nil,
        events: [GooseStreamEvent] = []
    ) async {
        state.withLock { state in
            state.createSessionResult = sessionResult
            state.createSessionError = sessionError
            state.streamEvents = events
        }
    }

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        let (result, error): (GooseSessionResponse?, Error?) = state.withLock { state in
            state.createSessionCallCount += 1
            state.lastSessionRequest = request
            return (state.createSessionResult, state.createSessionError)
        }
        if let error { throw error }
        return result ?? GooseSessionResponse(
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
        let events = state.withLock { state in
            state.submitPromptCallCount += 1
            state.lastSessionID = sessionID
            return state.streamEvents
        }
        return AsyncThrowingStream { c in
            Task { for e in events { c.yield(e) }; c.finish() }
        }
    }

    func closeSession(sessionID: String) async throws {
        state.withLock { $0.closeSessionCalled = true }
    }

    func reset() async {
        state.withLock { state in
            state.closeSessionCalled = false
            state.lastSessionID = nil
            state.lastSessionRequest = nil
            state.createSessionCallCount = 0
            state.submitPromptCallCount = 0
        }
    }

    var closeSessionCalled: Bool {
        get async { state.withLock { $0.closeSessionCalled } }
    }

    var lastSessionID: String? {
        get async { state.withLock { $0.lastSessionID } }
    }

    var lastSessionRequest: GooseSessionRequest? {
        get async { state.withLock { $0.lastSessionRequest } }
    }

    var createSessionCallCount: Int {
        get async { state.withLock { $0.createSessionCallCount } }
    }

    var submitPromptCallCount: Int {
        get async { state.withLock { $0.submitPromptCallCount } }
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
