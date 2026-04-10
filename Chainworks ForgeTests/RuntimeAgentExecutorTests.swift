import Testing
import Foundation
import os
import SwiftData
@testable import Chainworks_Forge

// MARK: - RuntimeAgentExecutorTests (Proposal 004, Section 12.1)

/// Unit tests for RuntimeAgentExecutor.
/// Tests session creation, streaming, receipt persistence, output validation, and result building.
@Suite("RuntimeAgentExecutor", .serialized)
struct RuntimeAgentExecutorTests {

    // MARK: - Helpers

    private func makeAgent(
        id: String = "test_agent",
        mode: String = "autonomous",
        outputs: [String] = ["test_output.md"],
        worktreeWriteEnabled: Bool = false
    ) -> ResolvedAgent {
        ResolvedAgent(
            id: id,
            title: "Test Agent",
            mode: mode,
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
            outputs: outputs,
            worktreeWriteEnabled: worktreeWriteEnabled
        )
    }

    private func makeTask(
        agent: String = "test_agent",
        task: String = "test_task"
    ) -> AgentTask {
        AgentTask(agent: agent, task: task, inputs: nil, outputs: nil)
    }

    private final class PromptCaptureTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionResult: RuntimeSessionResponse?
            var createSessionError: Error?
            var streamEvents: [RuntimeStreamEvent] = []
            var lastSessionRequest: RuntimeSessionRequest?
            var lastPromptRequest: RuntimePromptRequest?
            var closeSessionCalled = false
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func configure(
            sessionResult: RuntimeSessionResponse? = nil,
            sessionError: Error? = nil,
            events: [RuntimeStreamEvent] = []
        ) async {
            state.withLock { state in
                state.createSessionResult = sessionResult
                state.createSessionError = sessionError
                state.streamEvents = events
                state.lastSessionRequest = nil
                state.lastPromptRequest = nil
                state.closeSessionCalled = false
            }
        }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let (result, error): (RuntimeSessionResponse?, Error?) = state.withLock { state in
                state.lastSessionRequest = request
                return (state.createSessionResult, state.createSessionError)
            }
            if let error { throw error }
            return result ?? RuntimeSessionResponse(
                sessionId: "prompt-capture-\(UUID().uuidString.prefix(8))",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            let events = state.withLock { state in
                state.lastPromptRequest = prompt
                return state.streamEvents
            }
            return AsyncThrowingStream { continuation in
                Task { for e in events { continuation.yield(e) }; continuation.finish() }
            }
        }

        func closeSession(sessionID: String) async throws {
            state.withLock { $0.closeSessionCalled = true }
        }

        var lastSessionRequest: RuntimeSessionRequest? {
            get async { state.withLock { $0.lastSessionRequest } }
        }

        var lastPromptRequest: RuntimePromptRequest? {
            get async { state.withLock { $0.lastPromptRequest } }
        }

        var closeSessionCalled: Bool {
            get async { state.withLock { $0.closeSessionCalled } }
        }
    }

    private final class StalledACPReviewTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "claude_agent" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "stalled-acp-review",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            AsyncThrowingStream { continuation in
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.textChunk(text: "[thinking] Let me load the lazy artifacts first."))
                    continuation.yield(.toolCallStarted(toolName: "read", raw: "{}"))
                    continuation.yield(.toolCallStarted(toolName: "permission:read", raw: "{}"))
                    continuation.yield(.toolCallStarted(toolName: "read", raw: "{}"))
                    continuation.yield(.toolCallStarted(toolName: "permission:read", raw: "{}"))

                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(60))
                    }
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class StaleReuseTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
            var sessionUseCounts: [String: Int] = [:]
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-reused" : "session-fresh-\(callCount)"
            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            let useCount = state.withLock { state -> Int in
                state.submitPromptCallCount += 1
                state.submittedSessionIDs.append(sessionID)
                state.sessionUseCounts[sessionID, default: 0] += 1
                return state.sessionUseCounts[sessionID] ?? 0
            }

            return AsyncThrowingStream { continuation in
                Task {
                    if sessionID == "session-reused" && useCount == 2 {
                        continuation.yield(.sessionStarted(raw: #"{"session_id":"session-reused"}"#))
                        continuation.yield(.promptSubmitted(raw: #"{"session_id":"session-reused"}"#))
                        continuation.finish(throwing: NSError(
                            domain: "RuntimeTest",
                            code: 404,
                            userInfo: [NSLocalizedDescriptionKey: "Failed to read session: Session not found"]
                        ))
                        return
                    }

                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.finalOutput(content: sessionID == "session-reused" ? "first pass output" : "fresh fallback output"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private final class StaleReuseSSEErrorTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
            var sessionUseCounts: [String: Int] = [:]
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-reused" : "session-fresh-\(callCount)"
            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            let useCount = state.withLock { state -> Int in
                state.submitPromptCallCount += 1
                state.submittedSessionIDs.append(sessionID)
                state.sessionUseCounts[sessionID, default: 0] += 1
                return state.sessionUseCounts[sessionID] ?? 0
            }

            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "session-reused" && useCount == 2 {
                        continuation.yield(.error(message: "Failed to read session: Session not found"))
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: sessionID == "session-reused" ? "first pass output" : "fresh fallback output"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private final class StaleReuseNoActiveCodexSessionTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
            var runtimeStateReadsBySessionID: [String: Int] = [:]
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-reused" : "session-fresh-\(callCount)"
            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            state.withLock { state in
                state.submitPromptCallCount += 1
                state.submittedSessionIDs.append(sessionID)
            }

            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.finalOutput(content: sessionID == "session-reused" ? "first pass output" : "fresh fallback output"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            let readCount = state.withLock { state -> Int in
                state.runtimeStateReadsBySessionID[sessionID, default: 0] += 1
                return state.runtimeStateReadsBySessionID[sessionID] ?? 0
            }

            if sessionID == "session-reused" && readCount == 2 {
                throw RuntimeTransportError.streamingFailed(reason: "No active Codex session for ID: \(sessionID)")
            }

            return RuntimeSessionRuntimeState(enabledExtensions: [])
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private final class FreshSessionMissingTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-initial" : "session-fresh-\(callCount)"
            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            state.withLock { state in
                state.submitPromptCallCount += 1
                state.submittedSessionIDs.append(sessionID)
            }

            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.finalOutput(content: "fresh retry output"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            if sessionID == "session-initial" {
                throw RuntimeTransportError.streamingFailed(reason: "No active session for ID: \(sessionID)")
            }
            return RuntimeSessionRuntimeState(enabledExtensions: [])
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private final class ACPReadLoopStallTransport: RuntimeTransportProtocol, @unchecked Sendable {
        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "acp-stall-session",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"acp-stall-session"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"acp-stall-session"}"#))
                    continuation.yield(.textChunk(text: "Need to read the proposal and idea brief before I can review."))
                    continuation.yield(.toolCallStarted(toolName: "read", raw: #"{"tool_name":"read","tool_call_id":"call-read-1"}"#))
                    continuation.yield(.toolCallStarted(toolName: "permission:read", raw: #"{"tool_name":"permission:read","tool_call_id":"call-perm-1"}"#))
                    try? await Task.sleep(for: .seconds(5))
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class PersistentSessionUnavailableTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private let counter = OSAllocatedUnfairLock(initialState: 0)

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let sessionNumber = counter.withLock { value -> Int in
                value += 1
                return value
            }
            return RuntimeSessionResponse(
                sessionId: "session-\(sessionNumber)",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish(throwing: NSError(
                        domain: "RuntimeTest",
                        code: 404,
                        userInfo: [NSLocalizedDescriptionKey: "Failed to read session: Session not found"]
                    ))
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class WatchdogTimeoutTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var closeSessionCallCount = 0
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let sessionNumber = state.withLock { value -> Int in
                value.createSessionCallCount += 1
                return value.createSessionCallCount
            }
            return RuntimeSessionResponse(
                sessionId: "watchdog-timeout-\(sessionNumber)",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            AsyncThrowingStream { continuation in
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.textChunk(text: "Starting a long-running non-ACP execution."))
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(60))
                    }
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
                }
            }
        }

        func closeSession(sessionID: String) async throws {
            state.withLock { $0.closeSessionCallCount += 1 }
        }

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var closeSessionCallCount: Int {
            get async { state.withLock { $0.closeSessionCallCount } }
        }
    }

    private final class SilentEOFRetryTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }
            let sessionID = callCount == 1 ? "session-eof-1" : "session-eof-2"
            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            state.withLock { state in
                state.submitPromptCallCount += 1
                state.submittedSessionIDs.append(sessionID)
            }

            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "session-eof-1" {
                        continuation.yield(.textChunk(text: "partial progress before EOF"))
                        continuation.finish(throwing: RuntimeTransportError.streamingFailed(
                            reason: "Codex ACP stream ended before final result was received"
                        ))
                        return
                    }

                    continuation.yield(.finalOutput(content: """
                    <<<CHAINWORKS_OUTPUT:proposal_current>>>
                    # Recovered Proposal
                    <<<END_CHAINWORKS_OUTPUT>>>
                    """))
                    continuation.yield(.finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private final class DuplicateFreshSessionTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submittedSessionIDs: [String] = []
            var closedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID: String
            switch callCount {
            case 1, 2:
                sessionID = "dup-session"
            default:
                sessionID = "fresh-\(callCount)"
            }

            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            state.withLock { $0.submittedSessionIDs.append(sessionID) }
            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.finalOutput(content: "output from \(sessionID)"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func closeSession(sessionID: String) async throws {
            state.withLock { $0.closedSessionIDs.append(sessionID) }
        }

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }

        var closedSessionIDs: [String] {
            get async { state.withLock { $0.closedSessionIDs } }
        }
    }

    private final class RecycledCompletedSessionTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submittedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let sessionID = state.withLock { state -> String in
                state.createSessionCallCount += 1
                return "recycled-session"
            }

            return RuntimeSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: RuntimePromptRequest
        ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
            state.withLock { $0.submittedSessionIDs.append(sessionID) }
            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.finalOutput(content: "output from \(sessionID)"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submittedSessionIDs: [String] {
            get async { state.withLock { $0.submittedSessionIDs } }
        }
    }

    private func makeSessionManager() throws -> AgentSessionManager {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration(
            "RuntimeAgentExecutorTests-\(UUID().uuidString)",
            schema: schema,
            isStoredInMemoryOnly: true
        )
        let container = try ModelContainer(for: schema, configurations: [config])
        return AgentSessionManager(container: container)
    }

    private func makeContext(runID: UUID = UUID(), iteration: Int = 1) -> ExecutionContext {
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
            stageLineageID: "state_1",
            ownerExecutionLineageID: UUID(),
            iteration: iteration,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea body",
            providerBinding: nil
        )
    }

    private func makeContext(runID: UUID = UUID(), iteration: Int = 1, worktreeRoot: URL?) -> ExecutionContext {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("test-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)

        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: worktreeRoot
        )

        return ExecutionContext(
            workspace: workspace,
            stageID: "state_1",
            stageLineageID: "state_1",
            ownerExecutionLineageID: UUID(),
            iteration: iteration,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea body",
            providerBinding: nil
        )
    }

    private func makeACPProposalReviewContext(runID: UUID = UUID()) -> ExecutionContext {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("test-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)

        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let binding = ResolvedProviderBinding(
            agentID: "proposal_reviewer_product_owner",
            backendProfileID: "claude_product_high",
            configuredProviderID: UUID(),
            providerFamily: "claude_code",
            providerIdentifier: "claude_code",
            model: "opus",
            effort: "high",
            transport: "cli",
            adapterVersion: "test",
            runtimeProfileID: "claude_agent_acp",
            adapterFamily: "claude_agent_acp",
            capabilityClass: .operatorGrade
        )

        return ExecutionContext(
            workspace: workspace,
            stageID: "state_4_proposal_reviewed",
            stageLineageID: "state_4_proposal_reviewed",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Need feature flags instead of code deletion.",
            providerBinding: binding
        )
    }

    private func initializeGitRepository(at root: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["init"]
        process.currentDirectoryURL = root
        try process.run()
        process.waitUntilExit()
    }

    private func waitForSessionClose(_ transport: ObservableRuntimeTransport) async -> Bool {
        for _ in 0..<20 {
            let closed = await transport.closeSessionCalled
            if closed { return true }
            try? await Task.sleep(for: .milliseconds(25))
        }
        return await transport.closeSessionCalled
    }

    // MARK: - Tests

    /// testRuntimeExecutorCreatesSession — Section 12.1
    @MainActor
    @Test("Executor creates session with correct policy and produces receipt/transcript artifacts")
    func runtimeExecutorCreatesSession() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "session-abc123",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "# Test Output\n\nThis is a test proposal."),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["test_output.md"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Session should have been created and closed
        let closeSessionCalled = await waitForSessionClose(transport)
        #expect(closeSessionCalled, "Session should be closed after execution")

        let lastSessionRequest = await transport.lastSessionRequest
        #expect(lastSessionRequest?.executionPolicy?.permissionProfileID == "read_only")
        #expect(lastSessionRequest?.executionPolicy?.workspaceMode == "read_only")
        #expect(lastSessionRequest?.executionPolicy?.gitOperationsAllowed == false)
        #expect(lastSessionRequest?.executionPolicy?.releaseOperationsAllowed == false)
        #expect(lastSessionRequest?.executionPolicy?.repoWritesAllowed == false)

        // Result should contain receipt artifacts
        #expect(result.outputs.keys.contains(where: { $0.hasSuffix("_receipt.json") }),
                "Result should contain receipt artifact")
        #expect(result.outputs.keys.contains(where: { $0.hasSuffix("_transcript.md") }),
                "Result should contain transcript artifact")
    }

    /// testRuntimeExecutorStreamsEvents — Section 12.1
    @MainActor
    @Test("Executor streams events to event callback during execution")
    func runtimeExecutorStreamsEvents() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .toolCallStarted(toolName: "write_file", raw: "{}"),
                .toolCallFinished(toolName: "write_file", raw: "{}"),
                .textChunk(text: "Working on it..."),
                .finalOutput(content: "Done!"),
                .sessionClosed(raw: "{}")
            ]
        )

        // Use thread-safe collection to avoid concurrent mutation of captured var
        let eventCollector = SharedEventCollector()
        let executor = RuntimeAgentExecutor(transport: transport)
        executor.onExecutionEvent = { _, event in
            eventCollector.append(event)
        }

        let agent = makeAgent(outputs: ["test_output.md"])
        let task = makeTask()
        let context = makeContext()

        _ = try await executor.execute(task: task, agent: agent, context: context)

        // Should have received multiple events
        let receivedEvents = eventCollector.events
        #expect(receivedEvents.count >= 5, "Should receive at least 5 events, got \(receivedEvents.count)")

        let eventTypes = receivedEvents.map(\.type)
        #expect(eventTypes.contains(.sessionStarted))
        #expect(eventTypes.contains(.promptSubmitted))
        #expect(eventTypes.contains(.toolCallStarted))
        #expect(eventTypes.contains(.finalOutput))
    }

    /// testRuntimeExecutorPersistsReceiptArtifact — Section 12.1
    @MainActor
    @Test("Executor persists receipt artifact with correct agent ID and version")
    func runtimeExecutorPersistsReceiptArtifact() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "Test output"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(id: "proposal_writer", outputs: ["proposal_current"])
        let task = makeTask(agent: "proposal_writer", task: "draft_initial_proposal")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Should have receipt artifacts
        let receiptKey = try #require(result.outputs.keys.first { $0.contains("_receipt.json") })

        if let data = result.outputs[receiptKey] {
            // Parse the receipt JSON (must match encoder's .iso8601 date strategy)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let receipt = try decoder.decode(ExecutionReceipt.self, from: data)
            #expect(receipt.agentID == "proposal_writer")
            #expect(receipt.succeeded)
            #expect(receipt.receiptVersion == "1.1")
        }
    }

    @MainActor
    @Test("Executor runtime truth prefers frozen provider binding over live override")
    func runtimeExecutorPrefersFrozenProviderBindingForRuntimeTruth() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "session-bound-truth",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "# UI Review\n\nLooks good."),
                .sessionClosed(raw: "{}")
            ]
        )

        let override = LiveExecutionOverride(enabled: true, provider: "claude-code", model: "default", effort: "medium")
        let executor = RuntimeAgentExecutor(transport: transport, override: override)
        let configuredProviderID = UUID()
        let binding = ResolvedProviderBinding(
            agentID: "proposal_reviewer_ui",
            backendProfileID: "gemini_review_pro",
            configuredProviderID: configuredProviderID,
            providerFamily: "gemini",
            providerIdentifier: "gemini",
            model: "gemini-2.5-pro",
            effort: "medium",
            transport: "acp_stdio",
            adapterVersion: "v1",
            runtimeProfileID: "gemini_cli_acp",
            adapterFamily: "gemini_cli_acp",
            capabilityClass: .operatorGrade
        )
        let agent = ResolvedAgent(
            id: "proposal_reviewer_ui",
            title: "Proposal Reviewer / UI",
            mode: "review",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Review the proposal from a UI perspective.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_ui"]
        )
        let task = AgentTask(
            agent: "proposal_reviewer_ui",
            task: "review_proposal_as_ui_designer",
            inputs: nil,
            outputs: ["proposal_review_ui"]
        )
        let base = makeContext()
        let context = ExecutionContext(
            workspace: base.workspace,
            stageID: base.stageID,
            stageLineageID: base.stageLineageID,
            ownerExecutionLineageID: base.ownerExecutionLineageID,
            iteration: base.iteration,
            attemptNumber: base.attemptNumber,
            inputArtifacts: base.inputArtifacts,
            inputArtifactPaths: base.inputArtifactPaths,
            variables: base.variables,
            ideaBody: base.ideaBody,
            providerBinding: binding
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        let lastSessionRequest = await transport.lastSessionRequest
        #expect(lastSessionRequest?.provider == "gemini")
        #expect(lastSessionRequest?.model == "gemini-2.5-pro")

        #expect(result.runtimeProvider == "gemini")
        #expect(result.runtimeModel == "gemini-2.5-pro")
        #expect(result.resolvedModel == "gemini-2.5-pro")
        #expect(result.providerReceipt?.providerFamily == "gemini")
        #expect(result.providerReceipt?.model == "gemini-2.5-pro")
        #expect(result.providerReceipt?.transport == "acp_stdio")
        #expect(result.configuredProviderID == configuredProviderID)

        let receiptKey = try #require(result.outputs.keys.first { $0.hasSuffix("_receipt.json") })
        let receiptData = try #require(result.outputs[receiptKey])
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let receipt = try decoder.decode(ExecutionReceipt.self, from: receiptData)
        #expect(receipt.provider == "gemini")
        #expect(receipt.model == "gemini-2.5-pro")
        #expect(receipt.effort == "medium")
    }

    /// testRuntimeExecutorFailsWhenRequiredOutputsMissing — Section 12.1
    @MainActor
    @Test("Executor fails when required outputs are missing from stream")
    func runtimeExecutorFailsWhenRequiredOutputsMissing() async throws {
        let transport = ObservableRuntimeTransport()
        // Stream completes but no files are written and no final output
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["required_output.json"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Should fail because required output is missing
        #expect(!result.succeeded, "Should fail when required outputs are missing")
        #expect(result.errorMessage != nil)
        #expect(result.errorMessage?.contains("required_output.json") == true)
    }

    @MainActor
    @Test("Executor materializes multiple required outputs from returned output blocks without disk writes")
    func runtimeExecutorMaterializesReturnedOutputBlocks() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: """
                <<<CHAINWORKS_OUTPUT:proposal_current>>>
                # Proposal

                This proposal came from the runtime response envelope.
                <<<END_CHAINWORKS_OUTPUT>>>

                <<<CHAINWORKS_OUTPUT:proposal_revision_summary>>>
                Revision summary came from the runtime response envelope.
                <<<END_CHAINWORKS_OUTPUT>>>
                """),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = makeTask(agent: "proposal_writer", task: "refine_proposal")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        let proposalCurrent = try #require(result.outputs["proposal_current"])
        let proposalSummary = try #require(result.outputs["proposal_revision_summary"])
        #expect(String(decoding: proposalCurrent, as: UTF8.self).contains("# Proposal"))
        #expect(String(decoding: proposalSummary, as: UTF8.self).contains("Revision summary"))
    }

    @MainActor
    @Test("Executor merges required output blocks across final output and accumulated text")
    func runtimeExecutorMergesReturnedOutputBlocksAcrossSources() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .textChunk(text: """
                <<<CHAINWORKS_OUTPUT:proposal_current>>>
                # Proposal

                Full proposal body.
                <<<END_CHAINWORKS_OUTPUT>>>

                <<<CHAINWORKS_OUTPUT:proposal_revision_summary>>>
                Revision summary from accumulated text.
                <<<END_CHAINWORKS_OUTPUT>>>
                """),
                .finalOutput(content: """
                <<<CHAINWORKS_OUTPUT:proposal_current>>>
                # Proposal

                Full proposal body.
                <<<END_CHAINWORKS_OUTPUT>>>
                """),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = makeTask(agent: "proposal_writer", task: "refine_proposal")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        let proposalCurrent = try #require(result.outputs["proposal_current"])
        let proposalSummary = try #require(result.outputs["proposal_revision_summary"])
        #expect(String(decoding: proposalCurrent, as: UTF8.self).contains("# Proposal"))
        #expect(String(decoding: proposalSummary, as: UTF8.self).contains("Revision summary from accumulated text"))
    }

    @MainActor
    @Test("Executor tolerates degraded returned output end markers in accumulated text")
    func runtimeExecutorToleratesDegradedReturnedOutputEndMarkers() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .textChunk(text: """
                <<<CHAINWORKS_OUTPUT:proposal_current>>>
                # Proposal

                Full proposal body.
                <<<END_CHAINWORKS_OUTPUT>>

                <<<CHAINWORKS_OUTPUT:proposal_revision_summary>>>
                Revision summary from accumulated text.
                <<<END_CHAINWORKS_OUTPUT>>
                """),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = makeTask(agent: "proposal_writer", task: "refine_proposal")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        let proposalCurrent = try #require(result.outputs["proposal_current"])
        let proposalSummary = try #require(result.outputs["proposal_revision_summary"])
        #expect(String(decoding: proposalCurrent, as: UTF8.self).contains("# Proposal"))
        #expect(String(decoding: proposalSummary, as: UTF8.self).contains("Revision summary from accumulated text"))
    }

    @MainActor
    @Test("Executor synthesizes partial implementation artifacts when code writer fails before writing them")
    func runtimeExecutorSynthesizesPartialImplementationArtifactsOnFailure() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .sessionClosed(raw: "{}")
            ]
        )

        let runID = UUID()
        let worktreeRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("code-writer-worktree-\(runID.uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)
        try initializeGitRepository(at: worktreeRoot)
        try FileManager.default.createDirectory(
            at: worktreeRoot.appendingPathComponent("Sources", isDirectory: true),
            withIntermediateDirectories: true
        )
        try "print(\"debug\")\n".write(
            to: worktreeRoot.appendingPathComponent("Sources/App.swift"),
            atomically: true,
            encoding: .utf8
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "code_writer",
            outputs: [
                "implementation_progress",
                "implementation_self_assessment",
                "changed_files_manifest",
                "tests_result"
            ],
            worktreeWriteEnabled: true
        )
        let task = makeTask(agent: "code_writer", task: "initial_implementation")
        let context = makeContext(runID: runID, worktreeRoot: worktreeRoot)

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.outputs["implementation_progress"] != nil)
        #expect(result.outputs["implementation_self_assessment"] != nil)
        #expect(result.outputs["changed_files_manifest"] != nil)
        #expect(result.outputs["tests_result"] != nil)
    }

    @MainActor
    @Test("Implementation artifact synthesis collects changed files off the main thread")
    func implementationFailureSynthesisRunsCollectorOffMainThread() async throws {
        let agent = makeAgent(
            id: "code_writer",
            outputs: [
                "implementation_progress",
                "implementation_self_assessment",
                "changed_files_manifest",
                "tests_result"
            ],
            worktreeWriteEnabled: true
        )
        let context = makeContext(worktreeRoot: FileManager.default.temporaryDirectory)

        let outputs = await ImplementationFailureArtifactSynthesizer.supplementMissingOutputs(
            existingOutputs: [:],
            expectedOutputs: agent.outputs,
            agent: agent,
            context: context,
            failureSummary: "Synthetic failure for test"
        ) { _ in
            #expect(!Thread.isMainThread)
            return ["Sources/App.swift", "docs/plan.md"]
        }

        #expect(outputs["implementation_progress"] != nil)
        #expect(outputs["implementation_self_assessment"] != nil)
        #expect(outputs["changed_files_manifest"] != nil)
        #expect(outputs["tests_result"] != nil)
    }

    /// testRuntimeExecutorReturnsAgentResult — Section 12.1
    @MainActor
    @Test("Executor returns agent result with log snippet and cost estimate")
    func runtimeExecutorReturnsAgentResult() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .toolCallStarted(toolName: "read_file", raw: "{}"),
                .toolCallFinished(toolName: "read_file", raw: "{}"),
                .finalOutput(content: "# My Proposal\n\nThis is a great proposal."),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Final output should be used as primary output when no files written
        #expect(result.succeeded || result.outputs.keys.contains("proposal_current"),
                "Result should either succeed or contain the primary output")
        #expect(result.logSnippet != nil, "Should have a log snippet")
        #expect(result.costCents != nil, "Should have a cost estimate")
    }

    @MainActor
    @Test("Neutral finish marker alone does not count as success")
    func neutralFinishMarkerDoesNotCountAsSuccess() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["required_output.json"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage != nil)
        #expect(result.canonicalOutcome == .failedBeforeOutput)
        #expect(result.providerStopReason == "stop")
    }

    @MainActor
    @Test("Limit exhaustion after output preserves artifacts and records canonical outcome")
    func limitExhaustionAfterOutputPreservesArtifacts() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .textChunk(text: "partial draft"),
                .finish(reason: "max_tokens", totalTokens: 128, raw: #"{"type":"Finish","reason":"max_tokens"}"#),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()

        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)
        try FileManager.default.createDirectory(at: outputDir, withIntermediateDirectories: true)
        try Data("# Partial Proposal".utf8).write(to: outputDir.appendingPathComponent("proposal_current"))

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.outputs["proposal_current"] != nil)
        #expect(result.canonicalOutcome == .limitExhaustedAfterOutput)
        #expect(result.outputPresence == .durableOutput)
        #expect(result.providerStopReason == "max_tokens")
        #expect(result.errorMessage?.contains("limit") == true || result.errorMessage?.contains("output") == true)
    }

    @MainActor
    @Test("Durable structured outputs with neutral stop are treated as completed and receipt reflects success")
    func durableStructuredOutputsWithNeutralStopUseDurableTruth() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(id: "lead_orchestrator", outputs: ["proposal_review_summary"])
        let task = makeTask(agent: "lead_orchestrator", task: "aggregate_proposal_reviews")
        let context = makeContext()

        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)
        try FileManager.default.createDirectory(at: outputDir, withIntermediateDirectories: true)
        let summary = """
        {
          "average_score": 8.25,
          "min_individual_score": 7.0,
          "blocker_count": 0,
          "decision": "approve"
        }
        """
        try Data(summary.utf8).write(to: outputDir.appendingPathComponent("proposal_review_summary"))

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.canonicalOutcome == .completed)
        #expect(result.outputPresence == .durableOutput)
        #expect(result.errorMessage == nil)

        let receiptKey = try #require(result.outputs.keys.first { $0.hasSuffix("_receipt.json") })
        let receiptData = try #require(result.outputs[receiptKey])
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let receipt = try decoder.decode(ExecutionReceipt.self, from: receiptData)
        #expect(receipt.succeeded)
        #expect(receipt.errorMessage == nil)
    }

    /// testRuntimeExecutorSessionCreationFailure
    @MainActor
    @Test("Executor handles session creation failure gracefully")
    func runtimeExecutorSessionCreationFailure() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: RuntimeTransportError.httpError(
                statusCode: 500,
                body: "Internal server error"
            ),
            events: []
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent()
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage?.contains("Session creation failed") == true)
    }

    @MainActor
    @Test("Executor embeds selective handoff packet content into submitted prompt")
    func executorEmbedsSelectiveHandoffIntoPrompt() async throws {
        let transport = PromptCaptureTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "prompt-capture-session",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "# Proposal current content"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_writer",
            outputs: ["proposal_current"]
        )
        let task = makeTask(
            agent: "proposal_writer",
            task: "refine_proposal"
        )
        let workspace = makeContext().workspace
        let inputArtifacts = [
            "idea_brief": Data("short idea".utf8),
            "proposal_current": Data("full proposal body".utf8),
            "proposal_review_summary": Data(String(repeating: "review ", count: 80).utf8),
            "security_audit_raw": Data("sensitive raw audit".utf8)
        ]
        let stewardProfile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]
        )
        let strategyProfile = stewardProfile.runtimeProfile(profileID: "selective_compression_and_escalation")

        let baseContext = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            variables: [:],
            ideaBody: "Build a refined proposal",
            providerBinding: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            contextStrategyProfile: strategyProfile,
            handoffPacket: nil
        )
        let handoffPacket = HandoffCompiler().compile(
            profileID: "selective_compression_and_escalation",
            profile: strategyProfile,
            agent: agent,
            task: task,
            context: baseContext
        )
        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            stageLineageID: "state_2",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            variables: [:],
            ideaBody: "Build a refined proposal",
            providerBinding: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            contextStrategyProfile: strategyProfile,
            handoffPacket: handoffPacket
        )

        _ = try await executor.execute(task: task, agent: agent, context: context)

        let prompt = await transport.lastPromptRequest
        let contextAttachments = prompt?.context ?? []
        #expect(prompt?.content.contains("Profile: selective_compression_and_escalation") == true)
        #expect(prompt?.content.contains("Mode: selective") == true)
        #expect(contextAttachments.contains { $0.name == "idea_brief" && $0.type == "artifact" })
        #expect(contextAttachments.contains { $0.name == "proposal_review_summary" && $0.type == "artifact" })
        #expect(contextAttachments.contains { $0.name == "lazy_security_audit_raw" && $0.type == "text" })
    }

    @MainActor
    @Test("Executor fails without policy acknowledgement in session response")
    func runtimeExecutorFailsWithoutPolicyAcknowledgement() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "session-missing-ack",
                status: "active",
                policyAcknowledgement: nil
            ),
            sessionError: nil,
            events: []
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage?.contains("read-only execution policy") == true)
    }

    @MainActor
    @Test("Executor does not retry contract or missing-output failures as transport errors")
    func runtimeExecutorDoesNotRetryContractFailures() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "session-contract-failure",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finish(reason: "stop", totalTokens: 12, raw: "{}"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["test_output.md"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage?.contains("Required outputs missing") == true)
        #expect(await transport.createSessionCallCount == 1)
        #expect(await transport.submitPromptCallCount == 1)
    }

    @MainActor
    @Test("Executor fail-closes ACP proposal review read-loop stalls before watchdog and emits durable failure evidence")
    func runtimeExecutorFailClosesACPProposalReviewReadLoopStall() async throws {
        let originalSilence = RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds
        let originalThreshold = RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold
        let originalPoll = RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds
        RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = 0.05
        RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold = 4
        RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = originalSilence
            RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold = originalThreshold
            RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds = originalPoll
        }

        let transport = StalledACPReviewTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_reviewer_product_owner",
            mode: "proposal_review.product_owner",
            outputs: ["proposal_review_po"]
        )
        let task = makeTask(agent: agent.id, task: "review_proposal_as_product_owner")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .failedBeforeOutput)
        #expect(result.transportErrorKind == .unknown)
        #expect(result.outputs["proposal_reviewer_product_owner_receipt.json"] != nil)
        #expect(result.outputs["proposal_reviewer_product_owner_transcript.md"] != nil)
    }

    @MainActor
    @Test("Executor records lazy evidence hits when on-demand helper is invoked")
    func runtimeExecutorRecordsLazyEvidenceHits() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "session-lazy-hit",
                status: "active",
                policyAcknowledgement: RuntimePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .toolCallStarted(
                    toolName: "get_lazy_artifact",
                    raw: #"{"tool_name":"get_lazy_artifact","artifact_name":"security_audit_raw"}"#
                ),
                .toolCallFinished(
                    toolName: "get_lazy_artifact",
                    raw: #"{"tool_name":"get_lazy_artifact","artifact_name":"security_audit_raw"}"#
                ),
                .finalOutput(content: "# Refined proposal"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(id: "proposal_writer", outputs: ["proposal_current"])
        let task = AgentTask(
            agent: "proposal_writer",
            task: "refine_proposal",
            inputs: ["idea_brief", "security_audit_raw"],
            outputs: ["proposal_current"]
        )
        let workspace = makeContext().workspace
        let lazyArtifactPath = workspace.artifactRoot.appendingPathComponent("security_audit_raw.txt")
        try Data("sensitive raw audit".utf8).write(to: lazyArtifactPath)
        let strategyProfile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]
        ).runtimeProfile(profileID: "selective_compression_and_escalation")
        let inputArtifacts = [
            "idea_brief": Data("short idea".utf8),
            "security_audit_raw": Data("sensitive raw audit".utf8)
        ]

        let baseContext = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            stageLineageID: "state_2",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            inputArtifactPaths: ["security_audit_raw": lazyArtifactPath.path],
            variables: [:],
            ideaBody: "Build a refined proposal",
            providerBinding: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            contextStrategyProfile: strategyProfile,
            handoffPacket: nil
        )
        let handoffPacket = HandoffCompiler().compile(
            profileID: "selective_compression_and_escalation",
            profile: strategyProfile,
            agent: agent,
            task: task,
            context: baseContext
        )
        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            stageLineageID: "state_2",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            inputArtifactPaths: ["security_audit_raw": lazyArtifactPath.path],
            variables: [:],
            ideaBody: "Build a refined proposal",
            providerBinding: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            contextStrategyProfile: strategyProfile,
            handoffPacket: handoffPacket
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.lazyEvidenceArtifactHits == ["security_audit_raw"])
    }

    @MainActor
    @Test("Executor starts a fresh session after a prior successful generation settles")
    func runtimeExecutorFallsBackWhenReusedSessionDisappearsMidStream() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("RuntimeAgentExecutorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = StaleReuseTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)

        let runID = UUID()
        let firstContext = makeContext(runID: runID)
        let secondContext = ExecutionContext(
            workspace: firstContext.workspace,
            stageID: firstContext.stageID,
            stageLineageID: firstContext.stageLineageID,
            ownerExecutionLineageID: UUID(),
            iteration: firstContext.iteration,
            attemptNumber: firstContext.attemptNumber,
            inputArtifacts: firstContext.inputArtifacts,
            inputArtifactPaths: firstContext.inputArtifactPaths,
            variables: firstContext.variables,
            ideaBody: firstContext.ideaBody,
            providerBinding: firstContext.providerBinding
        )
        let agent = ResolvedAgent(
            id: "lead_orchestrator",
            title: "Lead / Orchestrator",
            mode: "orchestration",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Aggregate proposal reviews.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_summary"],
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "orchestration_loop"
        )
        let task = AgentTask(
            agent: "lead_orchestrator",
            task: "aggregate_proposal_reviews",
            inputs: nil,
            outputs: ["proposal_review_summary"]
        )

        let firstResult = try await executor.execute(task: task, agent: agent, context: firstContext)
        #expect(firstResult.succeeded)
        #expect(firstResult.sessionReuseDisposition == SessionReuseDisposition.fresh)

        let secondResult = try await executor.execute(task: task, agent: agent, context: secondContext)

        #expect(secondResult.succeeded)
        #expect(secondResult.sessionReuseDisposition == SessionReuseDisposition.fresh)
        #expect(secondResult.outputs["proposal_review_summary"] != nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-reused", "session-fresh-2"])
    }

    @MainActor
    @Test("Executor rejects duplicate fresh provider session IDs when another lineage is still active")
    func runtimeExecutorRejectsDuplicateFreshProviderSessionIDsAcrossLineages() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("RuntimeAgentExecutorCollisionTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = DuplicateFreshSessionTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)

        let runID = UUID()
        let leadContext = makeContext(runID: runID)

        let reviewerLineageID = try await sessionManager.getOrCreateLineage(
            runID: runID,
            agentID: "proposal_reviewer_ui",
            scope: .same_invocation_owner,
            familyID: nil
        )
        _ = try await sessionManager.createGeneration(
            lineageID: reviewerLineageID,
            invocationOwnerKey: "review-owner",
            providerSessionID: "dup-session",
            bindingFingerprint: "review-fp",
            workingDirectory: leadContext.workspace.workspaceRoot.path,
            workspaceMode: "read_only",
            runtimeProvider: "gemini",
            runtimeModel: "gemini-2.5-pro"
        )

        let leadContextRetry = ExecutionContext(
            workspace: leadContext.workspace,
            stageID: leadContext.stageID,
            stageLineageID: leadContext.stageLineageID,
            ownerExecutionLineageID: UUID(),
            iteration: leadContext.iteration,
            attemptNumber: leadContext.attemptNumber,
            inputArtifacts: leadContext.inputArtifacts,
            inputArtifactPaths: leadContext.inputArtifactPaths,
            variables: leadContext.variables,
            ideaBody: leadContext.ideaBody,
            providerBinding: leadContext.providerBinding
        )
        let leadAgent = ResolvedAgent(
            id: "lead_orchestrator",
            title: "Lead / Orchestrator",
            mode: "orchestration",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Aggregate proposal reviews.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_summary"],
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "orchestration_loop"
        )

        let leadTask = AgentTask(
            agent: "lead_orchestrator",
            task: "aggregate_proposal_reviews",
            inputs: nil,
            outputs: ["proposal_review_summary"]
        )

        let leadResult = try await executor.execute(task: leadTask, agent: leadAgent, context: leadContextRetry)
        #expect(leadResult.succeeded)
        #expect(leadResult.sessionID == "fresh-3")
        #expect(await transport.createSessionCallCount == 3)
        let submittedSessionIDs = await transport.submittedSessionIDs
        #expect(submittedSessionIDs.contains("fresh-3"))
        let closedSessionIDs = await transport.closedSessionIDs
        #expect(closedSessionIDs.contains("dup-session"))
    }

    @MainActor
    @Test("Executor does not treat a completed generation as an active provider-session collision")
    func runtimeExecutorIgnoresCompletedGenerationDuringFreshCollisionCheck() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("RuntimeAgentExecutorCompletedCollisionTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = RecycledCompletedSessionTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)

        let runID = UUID()
        let writerContext = makeContext(runID: runID)
        let architectContext = ExecutionContext(
            workspace: writerContext.workspace,
            stageID: "state_4_proposal_reviewed",
            stageLineageID: "state_4_proposal_reviewed",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: writerContext.inputArtifacts,
            inputArtifactPaths: writerContext.inputArtifactPaths,
            variables: writerContext.variables,
            ideaBody: writerContext.ideaBody,
            providerBinding: writerContext.providerBinding
        )

        let writerAgent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "write",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Draft proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"],
            sessionReuseScope: .same_invocation_owner
        )
        let architectAgent = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Proposal Reviewer / Architect",
            mode: "review",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Review proposal architecture.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_architect"],
            sessionReuseScope: .same_invocation_owner
        )

        let writerTask = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: ["proposal_current"]
        )
        let architectTask = AgentTask(
            agent: "proposal_reviewer_architect",
            task: "review_proposal_as_architect",
            inputs: nil,
            outputs: ["proposal_review_architect"]
        )

        let writerResult = try await executor.execute(task: writerTask, agent: writerAgent, context: writerContext)
        #expect(writerResult.succeeded)
        #expect(writerResult.sessionID == "recycled-session")

        let architectResult = try await executor.execute(task: architectTask, agent: architectAgent, context: architectContext)
        #expect(architectResult.succeeded)
        #expect(architectResult.sessionID == "recycled-session")
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["recycled-session", "recycled-session"])
    }

    @MainActor
    @Test("Executor starts a fresh session after prior generation settles even if transport used SSE errors before")
    func runtimeExecutorFallsBackWhenReusedSessionEndsWithSSEError() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("RuntimeAgentExecutorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = StaleReuseSSEErrorTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)

        let runID = UUID()
        let firstContext = makeContext(runID: runID)
        let secondContext = ExecutionContext(
            workspace: firstContext.workspace,
            stageID: firstContext.stageID,
            stageLineageID: firstContext.stageLineageID,
            ownerExecutionLineageID: UUID(),
            iteration: firstContext.iteration,
            attemptNumber: firstContext.attemptNumber,
            inputArtifacts: firstContext.inputArtifacts,
            inputArtifactPaths: firstContext.inputArtifactPaths,
            variables: firstContext.variables,
            ideaBody: firstContext.ideaBody,
            providerBinding: firstContext.providerBinding
        )
        let agent = ResolvedAgent(
            id: "lead_orchestrator",
            title: "Lead / Orchestrator",
            mode: "orchestration",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Aggregate proposal reviews.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_summary"],
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "orchestration_loop"
        )
        let task = AgentTask(
            agent: "lead_orchestrator",
            task: "aggregate_proposal_reviews",
            inputs: nil,
            outputs: ["proposal_review_summary"]
        )

        let firstResult = try await executor.execute(task: task, agent: agent, context: firstContext)
        #expect(firstResult.succeeded)
        #expect(firstResult.sessionReuseDisposition == SessionReuseDisposition.fresh)

        let secondResult = try await executor.execute(task: task, agent: agent, context: secondContext)

        #expect(secondResult.succeeded)
        #expect(secondResult.sessionReuseDisposition == SessionReuseDisposition.fresh)
        #expect(secondResult.outputs["proposal_review_summary"] != nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-reused", "session-fresh-2"])
    }

    @MainActor
    @Test("Executor maps quota stream errors to canonical limit exhaustion")
    func runtimeExecutorMapsQuotaErrorToLimitExhaustion() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .error(message: "Claude monthly quota exhausted; rate limit exceeded")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let result = try await executor.execute(task: makeTask(), agent: makeAgent(), context: makeContext())

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .limitExhaustedBeforeOutput)
        #expect(result.errorMessage == "Provider or app limit exhausted")
    }

    @MainActor
    @Test("Executor falls back to fresh session when reused Codex session is no longer active")
    func runtimeExecutorFallsBackFromNoActiveCodexSession() async throws {
        let transport = StaleReuseNoActiveCodexSessionTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: try makeSessionManager())

        let baseContext = makeContext()
        let runID = baseContext.workspace.runID
        let context1 = makeContext(runID: runID, iteration: 1)
        let context2 = makeContext(runID: runID, iteration: 2)
        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "proposal_authoring",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "AUTHOR",
            skillRef: "proposal_writer_core",
            skillRole: nil,
            prompt: "Draft the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"],
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "proposal_authoring_loop"
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: ["proposal_current"]
        )

        let firstResult = try await executor.execute(task: task, agent: agent, context: context1)
        #expect(firstResult.succeeded)
        #expect(firstResult.sessionReuseDisposition == SessionReuseDisposition.fresh)

        let secondResult = try await executor.execute(task: task, agent: agent, context: context2)
        #expect(secondResult.succeeded)
        #expect(secondResult.sessionReuseDisposition == SessionReuseDisposition.fresh)
        #expect(secondResult.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-reused", "session-fresh-2"])
    }

    @MainActor
    @Test("Executor retries with a fresh session when a just-created ACP session is already inactive")
    func runtimeExecutorRecoversWhenFreshSessionImmediatelyDisappears() async throws {
        let transport = FreshSessionMissingTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: try makeSessionManager())

        let runID = UUID()
        let context1 = makeContext(runID: runID, iteration: 1)
        let agent = ResolvedAgent(
            id: "proposal_reviewer_product_owner",
            title: "Proposal Reviewer / Product Owner",
            mode: "proposal_review.product_owner",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: "product_owner",
            prompt: "Review the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_po"],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: agent.id,
            task: "review_proposal_as_product_owner",
            inputs: nil,
            outputs: ["proposal_review_po"]
        )

        let result = try await executor.execute(task: task, agent: agent, context: context1)

        #expect(result.succeeded)
        #expect(result.sessionReuseDisposition == SessionReuseDisposition.fresh_after_transport_error)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 1)
        #expect(await transport.submittedSessionIDs == ["session-fresh-2"])
    }

    @MainActor
    @Test("Executor disables session reuse for codex ACP even when the owner key matches")
    func executorDisablesSessionReuseForCodexACP() async throws {
        let transport = StaleReuseTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: try makeSessionManager())

        let runID = UUID()
        let baseContext = makeContext(runID: runID, iteration: 1)
        let codexBinding = ResolvedProviderBinding(
            agentID: "proposal_writer",
            backendProfileID: "codex_writer_high",
            configuredProviderID: UUID(),
            providerFamily: "codex",
            providerIdentifier: "codex",
            model: "gpt-5.4",
            effort: "high",
            transport: "cli",
            adapterVersion: "test",
            runtimeProfileID: "codex_acp",
            adapterFamily: "codex_acp",
            capabilityClass: .operatorGrade
        )

        let context1 = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: codexBinding
        )
        let context2 = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: 2,
            attemptNumber: 1,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: codexBinding
        )
        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "proposal_authoring",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "AUTHOR",
            skillRef: "proposal_writer_core",
            skillRole: nil,
            prompt: "Draft the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: ["proposal_current"]
        )

        let firstResult = try await executor.execute(task: task, agent: agent, context: context1)
        #expect(firstResult.succeeded)
        #expect(firstResult.sessionReuseDisposition == .fresh)

        let secondResult = try await executor.execute(task: task, agent: agent, context: context2)
        #expect(secondResult.succeeded)
        #expect(secondResult.sessionReuseDisposition == .fresh)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-reused", "session-fresh-2"])
    }

    @MainActor
    @Test("Executor maps Gemini capacity exhaustion to retryable limit failure after durable output")
    func runtimeExecutorMapsGeminiCapacityErrorAfterOutput() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .error(message: "Gemini CLI command failed: Attempt 1 failed with status 429. No capacity available for model gemini-2.5-pro on the server. MODEL_CAPACITY_EXHAUSTED")
            ]
        )

        let context = makeContext()
        let agent = makeAgent(outputs: ["test_output.md"])
        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)
        try FileManager.default.createDirectory(at: outputDir, withIntermediateDirectories: true)
        try "durable output".data(using: .utf8)?.write(to: outputDir.appendingPathComponent("test_output.md"))

        let executor = RuntimeAgentExecutor(transport: transport)
        let result = try await executor.execute(task: makeTask(), agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .limitExhaustedAfterOutput)
        #expect(result.transportErrorKind == .provider)
        #expect(result.errorMessage == "Provider capacity exhausted; retry the agent")
    }

    @MainActor
    @Test("Executor surfaces session loss as provider-session unavailable instead of raw not-found text")
    func runtimeExecutorSurfacesBoundedSessionUnavailableMessage() async throws {
        let transport = PersistentSessionUnavailableTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let result = try await executor.execute(task: makeTask(), agent: makeAgent(), context: makeContext())

        #expect(!result.succeeded)
        #expect(result.errorMessage == "Provider session became unavailable during execution")
        #expect(result.errorMessage?.contains("Session not found") == false)
    }

    @MainActor
    @Test("ACP proposal reviewer read-loop stall fails early with durable failure evidence")
    func acpProposalReviewerReadLoopStallFailsEarlyWithDurableFailureEvidence() async throws {
        let originalSilence = RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds
        let originalPoll = RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds
        RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = 0.1
        RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = originalSilence
            RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds = originalPoll
        }

        let transport = ACPReadLoopStallTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = ResolvedAgent(
            id: "proposal_reviewer_product_owner",
            title: "Proposal Reviewer / Product Owner",
            mode: "proposal_review.product_owner",
            backendProfileID: "claude_product_high",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: "product_owner",
            prompt: "Review the proposal as a product owner.",
            outputContract: "proposal_review_v1",
            requiresHumanApproval: false,
            inputs: ["idea_brief", "proposal_current"],
            outputs: ["proposal_review_po"],
            worktreeWriteEnabled: false,
            sessionReuseScope: .same_invocation_owner,
            sessionFamilyID: nil,
            runtimeProfileID: "claude_agent_acp"
        )
        let task = makeTask(agent: agent.id, task: "review_proposal_as_product_owner")
        let context = makeACPProposalReviewContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.outputs["proposal_reviewer_product_owner_receipt.json"] != nil)
        #expect(result.outputs["proposal_reviewer_product_owner_transcript.md"] != nil)
        #expect(result.canonicalOutcome == AgentCanonicalOutcome.failedBeforeOutput)
        #expect(result.transportErrorKind == .unknown)
    }

    @MainActor
    @Test("Executor settles watchdog timeouts through durable failure path without automatic retry")
    func executorSettlesWatchdogTimeoutWithoutAutomaticRetry() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 0.05
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
        }

        let transport = WatchdogTimeoutTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let result = try await executor.execute(
            task: makeTask(),
            agent: makeAgent(),
            context: makeContext()
        )

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .timedOutBeforeOutput)
        #expect(result.transportErrorKind == .timeout)
        #expect(result.outputs["test_agent_receipt.json"] != nil)
        #expect(result.outputs["test_agent_transcript.md"] != nil)
        try await Task.sleep(for: .milliseconds(100))
        #expect(await transport.createSessionCallCount == 1)
        #expect(await transport.closeSessionCallCount == 1)
    }

    @MainActor
    @Test("Executor retries silent Codex EOF before final result with a fresh session")
    func executorRetriesSilentEOFBeforeFinalResult() async throws {
        let transport = SilentEOFRetryTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let result = try await executor.execute(
            task: makeTask(agent: "proposal_writer", task: "revise_proposal"),
            agent: makeAgent(id: "proposal_writer", outputs: ["proposal_current"]),
            context: makeContext()
        )

        #expect(result.succeeded)
        #expect(result.outputs["proposal_current"] != nil)
        #expect(result.sessionID == "session-eof-2")
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-eof-1", "session-eof-2"])
    }
}
