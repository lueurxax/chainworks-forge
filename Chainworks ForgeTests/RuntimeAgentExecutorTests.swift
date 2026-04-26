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
        var mcpRuntimeNamespace: String? { "codex" }

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

    private final class CodexRunawayGuardrailTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

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

            return RuntimeSessionResponse(
                sessionId: "codex-guard-\(callCount)",
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
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "codex-guard-1" {
                        continuation.yield(.toolCallStarted(toolName: "search", raw: "{}"))
                        continuation.yield(.toolCallFinished(toolName: "search", raw: "{}"))
                        continuation.yield(.toolCallStarted(toolName: "search", raw: "{}"))
                        continuation.yield(.toolCallFinished(toolName: "search", raw: "{}"))
                        continuation.yield(.toolCallStarted(toolName: "search", raw: "{}"))

                        while !Task.isCancelled {
                            try? await Task.sleep(for: .seconds(60))
                        }
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: "guardrail-recovered"))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 42, raw: "{}"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
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

    private final class CodexUsageLimitTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "codex-usage-limit",
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
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"codex-usage-limit"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"codex-usage-limit"}"#))
                    continuation.finish(throwing: NSError(
                        domain: "RuntimeTest",
                        code: -32603,
                        userInfo: [
                            NSLocalizedDescriptionKey: #"Internal error (code -32603) {"codex_error_info":"usage_limit_exceeded","message":"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again later."}"#
                        ]
                    ))
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class SessionClosedWithoutOutputRetryTransport: RuntimeTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            return RuntimeSessionResponse(
                sessionId: "session-closed-\(callCount)",
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
            let attempt = state.withLock { state -> Int in
                state.submitPromptCallCount += 1
                return state.submitPromptCallCount
            }

            return AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if attempt == 1 {
                        continuation.yield(.textChunk(text: "[thinking] Reviewing proposal context"))
                        continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: """
<<<CHAINWORKS_OUTPUT:proposal_review_po>>>
{"score":8}
<<<END_CHAINWORKS_OUTPUT>>>
"""))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 12, raw: "{}"))
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
    }

    private final class CodexOversizedPayloadTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

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

            return RuntimeSessionResponse(
                sessionId: "codex-payload-\(callCount)",
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
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "codex-payload-1" {
                        let hugePayload = String(repeating: "x", count: 512)
                        continuation.yield(.toolCallStarted(toolName: "build_sim", raw: "{}"))
                        continuation.yield(.toolCallFinished(toolName: "build_sim", raw: hugePayload))
                        while !Task.isCancelled {
                            try? await Task.sleep(for: .seconds(60))
                        }
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: "payload-guardrail-recovered"))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 21, raw: "{}"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
                }
            }
        }

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            RuntimeSessionRuntimeState(enabledExtensions: [])
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

    private final class CodexRuntimeHomeGuardrailTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())
        private let runtimeHomeURL: URL

        init(runtimeHomeURL: URL) {
            self.runtimeHomeURL = runtimeHomeURL
        }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            return RuntimeSessionResponse(
                sessionId: "codex-home-\(callCount)",
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
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "codex-home-1" {
                        let oversized = runtimeHomeURL.appendingPathComponent("oversized.bin")
                        let data = Data(repeating: 0x41, count: 512)
                        try? data.write(to: oversized)
                        while !Task.isCancelled {
                            try? await Task.sleep(for: .seconds(60))
                        }
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: "runtime-home-guardrail-recovered"))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 21, raw: "{}"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
                }
            }
        }

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            RuntimeSessionRuntimeState(
                enabledExtensions: [],
                runtimeHomePath: runtimeHomeURL.path
            )
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }
    }

    private final class CodexSessionHistoryGuardrailTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

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

            return RuntimeSessionResponse(
                sessionId: "codex-history-\(callCount)",
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
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))

                    if sessionID == "codex-history-1" {
                        continuation.yield(.toolCallStarted(toolName: "search", raw: "{}"))
                        continuation.yield(.unknown(type: "usage_update", data: #"{"usage":{"input_tokens":600000,"cached_input_tokens":500000,"output_tokens":2048,"model_context_window":258400}}"#))
                        while !Task.isCancelled {
                            try? await Task.sleep(for: .seconds(60))
                        }
                        continuation.finish()
                        return
                    }

                    continuation.yield(.finalOutput(content: "session-history-guardrail-recovered"))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 21, raw: "{}"))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
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

    private final class ProviderDiagnosticStreamFailureTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "provider-diagnostic-stream-failure",
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
                    continuation.finish(throwing: RuntimeTransportError.streamingFailed(
                        reason: "Codex ACP stream ended before final result was received"
                    ))
                }
            }
        }

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            RuntimeSessionRuntimeState(
                enabledExtensions: [],
                providerDiagnostics: [
                    RuntimeProviderDiagnostic(
                        source: "codex_stderr",
                        severity: .error,
                        message: "write_stdin failed: stdin is closed for this session; rerun exec_command with tty=true to keep stdin open",
                        normalizedReason: "stdin_closed_for_session"
                    )
                ]
            )
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class FatalProviderDiagnosticDuringStreamTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        private struct State {
            var runtimeStateReadCount = 0
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            state.withLock { $0.createSessionCallCount += 1 }
            return RuntimeSessionResponse(
                sessionId: "fatal-provider-diagnostic-session",
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
            state.withLock { $0.submitPromptCallCount += 1 }
            return AsyncThrowingStream { continuation in
                let task = Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
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

        func readSessionRuntimeState(sessionID: String) async throws -> RuntimeSessionRuntimeState? {
            let count = state.withLock { state -> Int in
                state.runtimeStateReadCount += 1
                return state.runtimeStateReadCount
            }
            if count < 2 {
                return RuntimeSessionRuntimeState(enabledExtensions: [])
            }
            return RuntimeSessionRuntimeState(
                enabledExtensions: [],
                providerDiagnostics: [
                    RuntimeProviderDiagnostic(
                        source: "codex_stderr",
                        severity: .error,
                        message: "apply_patch verification failed: Failed to find expected lines in AddTransactionView.swift",
                        normalizedReason: "apply_patch_verification_failed"
                    )
                ]
            )
        }

        func closeSession(sessionID: String) async throws {}

        var createSessionCallCount: Int {
            get async { state.withLock { $0.createSessionCallCount } }
        }

        var submitPromptCallCount: Int {
            get async { state.withLock { $0.submitPromptCallCount } }
        }
    }

    private final class ACPReadLoopStallTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "claude_agent" }

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
                    continuation.yield(.toolCallStarted(toolName: "read_file", raw: #"{"tool_name":"read_file","tool_call_id":"call-read-2"}"#))
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
        var mcpRuntimeNamespace: String? { "claude_agent" }

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

    private final class StreamingThinkingBeforeOutputTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "claude_agent" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "streaming-thinking-before-output",
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
                    for chunk in [
                        "[thinking] analyzing reviews",
                        "[thinking] aggregating scores",
                        "[thinking] drafting output envelope"
                    ] {
                        try? await Task.sleep(for: .milliseconds(30))
                        continuation.yield(.textChunk(text: chunk))
                    }
                    try? await Task.sleep(for: .milliseconds(20))
                    continuation.yield(.finalOutput(content: """
                    <<<CHAINWORKS_OUTPUT:proposal_review_summary>>>
                    {"pass":false,"average_score":7.75,"aggregate_score":7.75,"min_individual_score":4,"blocker_count":3,"summary":"revise","required_changes":["a"],"recurring_themes":["b"],"decision":"revise"}
                    <<<END_CHAINWORKS_OUTPUT>>>
                    """))
                    continuation.yield(.finish(reason: "end_turn", totalTokens: 42, raw: #"{"stopReason":"end_turn"}"#))
                    continuation.yield(.sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish()
                }

                continuation.onTermination = { @Sendable _ in
                    task.cancel()
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class CompletedMutationWithoutSideEffectTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "mutation-without-side-effect",
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
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"mutation-without-side-effect"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"mutation-without-side-effect"}"#))
                    continuation.yield(.textChunk(text: "Applying an edit now."))
                    continuation.yield(.toolCallStarted(toolName: "edit", raw: #"{"tool_name":"edit","tool_call_id":"call-edit-1"}"#))
                    continuation.yield(.toolCallStarted(toolName: "permission:edit", raw: #"{"tool_name":"permission:edit","tool_call_id":"call-perm-edit-1"}"#))
                    continuation.yield(.toolCallFinished(toolName: "permission:edit", raw: #"{"tool_name":"permission:edit","tool_call_id":"call-perm-edit-1"}"#))
                    continuation.yield(.toolCallFinished(toolName: "edit", raw: #"{"tool_name":"edit","tool_call_id":"call-edit-1"}"#))
                    continuation.yield(.textChunk(text: "Edit reported success."))
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

    private final class StartedEditWithoutCompletionTransport: RuntimeTransportProtocol, @unchecked Sendable {
        var mcpRuntimeNamespace: String? { "codex" }

        func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
            RuntimeSessionResponse(
                sessionId: "started-edit-without-completion",
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
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"started-edit-without-completion"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"started-edit-without-completion"}"#))
                    continuation.yield(.textChunk(text: "Applying an edit now."))
                    continuation.yield(.toolCallStarted(toolName: "edit", raw: #"{"tool_name":"edit","tool_call_id":"call-edit-1"}"#))
                    continuation.yield(.toolCallStarted(toolName: "permission:edit", raw: #"{"tool_name":"permission:edit","tool_call_id":"call-perm-edit-1"}"#))
                    continuation.yield(.toolCallFinished(toolName: "permission:edit", raw: #"{"tool_name":"permission:edit","tool_call_id":"call-perm-edit-1"}"#))
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

    private func makeContext(
        runID: UUID = UUID(),
        iteration: Int = 1,
        agentAttemptNumber: Int? = nil,
        retryReason: String? = nil,
        supersedesAgentExecutionID: UUID? = nil
    ) -> ExecutionContext {
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
            providerBinding: nil,
            agentAttemptNumber: agentAttemptNumber,
            retryReason: retryReason,
            supersedesAgentExecutionID: supersedesAgentExecutionID
        )
    }

    private func makeContext(
        runID: UUID = UUID(),
        iteration: Int = 1,
        worktreeRoot: URL?,
        agentAttemptNumber: Int? = nil,
        retryReason: String? = nil,
        supersedesAgentExecutionID: UUID? = nil
    ) -> ExecutionContext {
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
            providerBinding: nil,
            agentAttemptNumber: agentAttemptNumber,
            retryReason: retryReason,
            supersedesAgentExecutionID: supersedesAgentExecutionID
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
            #expect(receipt.receiptVersion == "1.2")
        }
    }

    @MainActor
    @Test("Executor receipt preserves retry lineage metadata from execution context")
    func runtimeExecutorReceiptPreservesRetryLineageMetadata() async throws {
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
        let agent = makeAgent(id: "code_writer", outputs: ["changed_files_manifest"])
        let task = makeTask(agent: "code_writer", task: "continue_implementation")
        let superseded = UUID()
        let context = makeContext(
            agentAttemptNumber: 2,
            retryReason: "automatic_watchdog_retry",
            supersedesAgentExecutionID: superseded
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)
        let receiptKey = try #require(result.outputs.keys.first { $0.contains("_receipt.json") })
        let data = try #require(result.outputs[receiptKey])

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let receipt = try decoder.decode(ExecutionReceipt.self, from: data)
        #expect(receipt.agentAttemptNumber == 2)
        #expect(receipt.retryReason == "automatic_watchdog_retry")
        #expect(receipt.supersedesAgentExecutionID == superseded)
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

    @MainActor
    @Test("Executor prefers provider-observed Codex model over planned model on usage-limit failure")
    func runtimeExecutorPrefersProviderObservedCodexModelOnUsageLimitFailure() async throws {
        let executor = RuntimeAgentExecutor(transport: CodexUsageLimitTransport())
        let configuredProviderID = UUID()
        let binding = ResolvedProviderBinding(
            agentID: "code_writer",
            backendProfileID: "codex_builder_high",
            configuredProviderID: configuredProviderID,
            providerFamily: "codex",
            providerIdentifier: "codex",
            model: "GPT-5.4",
            effort: "high",
            transport: "cli",
            adapterVersion: "v1",
            runtimeProfileID: "codex_acp",
            adapterFamily: "codex_acp",
            capabilityClass: .operatorGrade
        )
        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            provider: "codex",
            model: "GPT-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "CODE_WRITE",
            skillRef: "code_writer_core",
            skillRole: nil,
            prompt: "Implement the approved proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["changed_files_manifest"],
            worktreeWriteEnabled: true,
            sessionReuseScope: .none
        )
        let task = AgentTask(
            agent: "code_writer",
            task: "continue_implementation",
            inputs: nil,
            outputs: ["changed_files_manifest"]
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

        #expect(!result.succeeded)
        #expect(result.runtimeModel == "gpt-5.3-codex-spark")
        #expect(result.providerReceipt?.model == "gpt-5.3-codex-spark")
        #expect(result.resolvedModel == "GPT-5.4")
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
    @Test("Executor materializes trailing returned output block when final end marker is missing")
    func runtimeExecutorMaterializesTrailingOutputBlockWithoutEndMarker() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: """
                <<<CHAINWORKS_OUTPUT:proposal_current>>>
                {"title":"Proposal"}
                <<<END_CHAINWORKS_OUTPUT>>>

                <<<CHAINWORKS_OUTPUT:proposal_revision_summary>>>
                {"summary":"ok"}
                <<<END_CHAINWORKS_OUTPUT>>>

                <<<CHAINWORKS_OUTPUT:proposal_feedback_coverage>>>
                {"coverage":"full"}
                """),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary", "proposal_feedback_coverage"]
        )
        let task = makeTask(agent: "proposal_writer", task: "refine_proposal")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(String(decoding: try #require(result.outputs["proposal_current"]), as: UTF8.self).contains("\"title\":\"Proposal\""))
        #expect(String(decoding: try #require(result.outputs["proposal_revision_summary"]), as: UTF8.self).contains("\"summary\":\"ok\""))
        #expect(String(decoding: try #require(result.outputs["proposal_feedback_coverage"]), as: UTF8.self).contains("\"coverage\":\"full\""))
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
        #expect(result.canonicalOutcome == .completedWithTransportError)
        #expect(result.outputPresence == .durableOutput)
        #expect(result.providerStopReason == "session_closed_without_transition")
        #expect(result.errorMessage == "Execution produced output but transport errored afterward")
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
    @Test("Executor does not spawn git status while preparing mutation observer baseline")
    func runtimeExecutorDoesNotSpawnGitStatusForMutationObserverBaseline() async throws {
        let transport = PromptCaptureTransport()
        await transport.configure(
            sessionResult: RuntimeSessionResponse(
                sessionId: "mutation-baseline-session",
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
                .promptSubmitted(raw: "{}"),
                .finalOutput(content: "implementation complete"),
                .sessionClosed(raw: "{}")
            ]
        )

        let fakeRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("fake-git-\(UUID().uuidString)", isDirectory: true)
        let binDir = fakeRoot.appendingPathComponent("bin", isDirectory: true)
        let markerPath = fakeRoot.appendingPathComponent("git-invoked").path
        let gitPath = binDir.appendingPathComponent("git").path
        try FileManager.default.createDirectory(at: binDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: fakeRoot) }

        let script = """
        #!/bin/sh
        echo invoked > "\(markerPath)"
        sleep 1
        exit 0
        """
        try script.write(toFile: gitPath, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: gitPath)

        let originalPath = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        setenv("PATH", "\(binDir.path):\(originalPath)", 1)
        defer { setenv("PATH", originalPath, 1) }

        let worktreeRoot = fakeRoot.appendingPathComponent("repo", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)
        try Data("gitdir: /tmp/not-used".utf8).write(to: worktreeRoot.appendingPathComponent(".git"))

        let executor = RuntimeAgentExecutor(transport: transport)
        let context = makeContext(worktreeRoot: worktreeRoot)
        let agent = makeAgent(id: "code_writer", outputs: [], worktreeWriteEnabled: true)
        let task = makeTask(agent: "code_writer", task: "continue_implementation")

        _ = try await executor.execute(task: task, agent: agent, context: context)

        #expect(FileManager.default.fileExists(atPath: markerPath) == false)
    }

    @MainActor
    @Test("Executor surfaces provider stderr verification failure instead of generic missing-final-output")
    func runtimeExecutorSurfacesProviderVerificationFailureOnSettledStream() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .toolCallStarted(toolName: "edit", raw: "{}"),
                .toolCallFinished(toolName: "permission:edit", raw: "{}"),
                .sessionClosed(raw: "{}")
            ],
            runtimeState: RuntimeSessionRuntimeState(
                enabledExtensions: [],
                providerDiagnostics: [
                    RuntimeProviderDiagnostic(
                        source: "codex_stderr",
                        severity: .error,
                        message: "apply_patch verification failed: Failed to find expected lines in OnboardingFlowView.swift",
                        normalizedReason: "apply_patch_verification_failed"
                    )
                ]
            )
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext(iteration: 1)
        let codexBinding = ResolvedProviderBinding(
            agentID: "code_writer",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: codexBinding
        )
        let agent = makeAgent(id: "code_writer", outputs: [])
        let task = makeTask(agent: "code_writer", task: "continue_implementation")

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.transportErrorKind == .provider)
        #expect(result.providerStopReason == "apply_patch_verification_failed")
        #expect(result.errorMessage == "apply_patch verification failed: Failed to find expected lines in OnboardingFlowView.swift")
        #expect(result.outcomeEnvelope?.providerStopReason == "apply_patch_verification_failed")
        #expect(result.outcomeEnvelope?.rawErrorMessage == "apply_patch verification failed: Failed to find expected lines in OnboardingFlowView.swift")
    }

    @MainActor
    @Test("Executor fails fast when live Codex provider diagnostics report fatal patch-context failure")
    func runtimeExecutorFailsFastOnLiveCodexProviderDiagnostic() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalPoll = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPoll
        }

        let transport = FatalProviderDiagnosticDuringStreamTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext(iteration: 1)
        let codexBinding = ResolvedProviderBinding(
            agentID: "code_writer",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: codexBinding
        )

        let result = try await executor.execute(
            task: makeTask(agent: "code_writer", task: "continue_implementation"),
            agent: makeAgent(id: "code_writer", outputs: []),
            context: context
        )

        #expect(!result.succeeded)
        #expect(result.transportErrorKind == .provider)
        #expect(result.providerStopReason == "apply_patch_verification_failed")
        #expect(result.errorMessage == "apply_patch verification failed: Failed to find expected lines in AddTransactionView.swift")
        #expect(result.supervisionClassification == nil)
        #expect(result.outcomeEnvelope?.providerStopReason == "apply_patch_verification_failed")
        #expect(await transport.createSessionCallCount == 1)
        #expect(await transport.submitPromptCallCount == 1)
    }

    @MainActor
    @Test("Executor surfaces lazy artifact lookup failure instead of generic missing-final-output")
    func runtimeExecutorSurfacesLazyArtifactLookupFailureOnSettledStream() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .toolCallStarted(
                    toolName: "get_lazy_artifact",
                    raw: #"{"toolCallId":"call_lazy_1","name":"get_lazy_artifact"}"#
                ),
                .toolCallFinished(
                    toolName: "get_lazy_artifact",
                    raw: #"{"toolCallId":"call_lazy_1","name":"get_lazy_artifact","status":"failed","rawOutput":{"stdout":"lazy artifact not found: proposal_review_architect_json\n"}}"#
                ),
                .sessionClosed(raw: "{}")
            ],
            runtimeState: RuntimeSessionRuntimeState(
                enabledExtensions: [],
                providerDiagnostics: []
            )
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext(iteration: 1)
        let handoffPacket = HandoffPacket(
            profileID: "current_mixed_baseline",
            mode: .selective,
            task: "refine_proposal",
            mandatoryArtifacts: [:],
            summaries: [:],
            lazyArtifactRefs: [
                "proposal_review_architect": ArtifactPointer(
                    artifactName: "proposal_review_architect",
                    absolutePath: "/tmp/proposal_review_architect",
                    byteCount: 10
                )
            ],
            checkpoint: nil,
            summaryMetrics: HandoffSummaryMetrics(
                mandatoryArtifactCount: 0,
                summarizedArtifactCount: 0,
                lazyArtifactCount: 1,
                compactionCount: 0,
                payloadBytesBeforeStrategy: 100,
                payloadBytesAfterStrategy: 20
            ),
            promotedArtifacts: []
        )
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: nil,
            handoffPacket: handoffPacket
        )
        let agent = makeAgent(id: "proposal_writer", outputs: [])
        let task = makeTask(agent: "proposal_writer", task: "refine_proposal")

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage == "Lazy artifact request failed: proposal_review_architect_json")
    }

    @MainActor
    @Test("Executor surfaces provider stderr on stream failure instead of generic transport text")
    func runtimeExecutorSurfacesProviderDiagnosticOnStreamFailure() async throws {
        let executor = RuntimeAgentExecutor(transport: ProviderDiagnosticStreamFailureTransport())
        let baseContext = makeContext(iteration: 1)
        let codexBinding = ResolvedProviderBinding(
            agentID: "code_writer",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: codexBinding
        )
        let result = try await executor.execute(
            task: makeTask(agent: "code_writer", task: "continue_implementation"),
            agent: makeAgent(id: "code_writer", outputs: []),
            context: context
        )

        #expect(!result.succeeded)
        #expect(result.transportErrorKind == .provider)
        #expect(result.providerStopReason == "stdin_closed_for_session")
        #expect(result.errorMessage == "write_stdin failed: stdin is closed for this session; rerun exec_command with tty=true to keep stdin open")
        #expect(result.outcomeEnvelope?.providerStopReason == "stdin_closed_for_session")
        #expect(result.outcomeEnvelope?.rawErrorMessage == "write_stdin failed: stdin is closed for this session; rerun exec_command with tty=true to keep stdin open")
    }

    @MainActor
    @Test("Executor surfaces Gemini capacity exhaustion instead of generic missing outputs")
    func runtimeExecutorSurfacesGeminiCapacityExhaustionOnSettledStream() async throws {
        let transport = ObservableRuntimeTransport()
        await transport.configure(
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .sessionClosed(raw: "{}")
            ],
            runtimeState: RuntimeSessionRuntimeState(
                enabledExtensions: [],
                providerDiagnostics: [
                    RuntimeProviderDiagnostic(
                        source: "gemini_stderr",
                        severity: .error,
                        message: "Attempt 1 failed with status 429. No capacity available for model gemini-3.1-pro-preview on the server. MODEL_CAPACITY_EXHAUSTED",
                        normalizedReason: "model_capacity_exhausted"
                    )
                ]
            )
        )

        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext(iteration: 1)
        let geminiBinding = ResolvedProviderBinding(
            agentID: "proposal_reviewer_ui",
            backendProfileID: "gemini_review_pro",
            configuredProviderID: UUID(),
            providerFamily: "gemini",
            providerIdentifier: "gemini",
            model: "gemini-3.1-pro-preview",
            effort: "high",
            transport: "cli",
            adapterVersion: "test",
            runtimeProfileID: "gemini_cli_acp",
            adapterFamily: "gemini_cli_acp",
            capabilityClass: .operatorGrade
        )
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: geminiBinding
        )

        let result = try await executor.execute(
            task: makeTask(agent: "proposal_reviewer_ui", task: "review_proposal_ui"),
            agent: makeAgent(id: "proposal_reviewer_ui", outputs: ["proposal_review_ui"]),
            context: context
        )

        #expect(!result.succeeded)
        #expect(result.transportErrorKind == .provider)
        #expect(result.providerStopReason == "model_capacity_exhausted")
        #expect(result.errorMessage == "Attempt 1 failed with status 429. No capacity available for model gemini-3.1-pro-preview on the server. MODEL_CAPACITY_EXHAUSTED")
        #expect(result.outcomeEnvelope?.providerStopReason == "model_capacity_exhausted")
        #expect(result.outcomeEnvelope?.rawErrorMessage == "Attempt 1 failed with status 429. No capacity available for model gemini-3.1-pro-preview on the server. MODEL_CAPACITY_EXHAUSTED")
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
    @Test("Executor retries session-closed proposal review attempts that end before any final output")
    func runtimeExecutorRetriesSessionClosedWithoutOutput() async throws {
        let transport = SessionClosedWithoutOutputRetryTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let agent = makeAgent(
            id: "proposal_reviewer_product_owner",
            mode: "proposal_review.product_owner",
            outputs: ["proposal_review_po"]
        )
        let task = makeTask(agent: agent.id, task: "review_proposal_as_product_owner")
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.outputs["proposal_review_po"] != nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
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
    @Test("Executor does not reuse Codex sessions across settled executions")
    func runtimeExecutorDoesNotReuseCodexSessionsAcrossTurns() async throws {
        let transport = StaleReuseTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: try makeSessionManager())

        let runID = UUID()
        let context1 = makeContext(runID: runID, iteration: 1)
        let context2 = makeContext(runID: runID, iteration: 2)
        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            provider: "codex_acp",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "RW_IMPLEMENT",
            skillRef: "code_writer",
            skillRole: nil,
            prompt: "Implement the approved changes.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["changed_files_manifest"],
            worktreeWriteEnabled: true,
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "implementation_loop"
        )
        let task = AgentTask(
            agent: "code_writer",
            task: "implement_changes",
            inputs: nil,
            outputs: ["changed_files_manifest"]
        )

        let firstResult = try await executor.execute(task: task, agent: agent, context: context1)
        #expect(firstResult.succeeded)
        #expect(firstResult.sessionReuseDisposition == .fresh)

        let secondResult = try await executor.execute(task: task, agent: agent, context: context2)
        #expect(secondResult.succeeded)
        #expect(secondResult.sessionReuseDisposition == .fresh)
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
        #expect(result.sessionReuseDisposition == SessionReuseDisposition.fresh)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 1)
        #expect(await transport.submittedSessionIDs == ["session-fresh-2"])
    }

    @MainActor
    @Test("Executor forces fresh Codex ACP sessions across settled executions")
    func executorForcesFreshCodexACPSessions() async throws {
        let executor = RuntimeAgentExecutor(transport: PromptCaptureTransport())

        let baseContext = makeContext(iteration: 1)
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

        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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

        let effectiveAgent = executor.effectiveAgentForExecution(agent, context: context)
        #expect(effectiveAgent.sessionReuseScope == .none)
        #expect(effectiveAgent.runtimeProfileID == agent.runtimeProfileID)
    }

    @MainActor
    @Test("Executor retries codex ACP after runaway guardrail trips")
    func executorRetriesCodexAfterRunawayGuardrail() async throws {
        let originalMaxToolCalls = RuntimeAgentExecutor.codexACPMaxToolCallCount
        let originalPollInterval = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.codexACPMaxToolCallCount = 3
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.codexACPMaxToolCallCount = originalMaxToolCalls
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPollInterval
        }

        let transport = CodexRunawayGuardrailTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let baseContext = makeContext(iteration: 1)
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

        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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
            outputs: [],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: []
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["codex-guard-1", "codex-guard-2"])
    }

    @MainActor
    @Test("Executor invalidates the active generation before retrying a codex runaway guardrail failure")
    func executorInvalidatesGenerationBeforeRetryingCodexRunawayGuardrail() async throws {
        let originalMaxToolCalls = RuntimeAgentExecutor.codexACPMaxToolCallCount
        let originalPollInterval = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.codexACPMaxToolCallCount = 3
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.codexACPMaxToolCallCount = originalMaxToolCalls
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPollInterval
        }

        let sessionManager = try makeSessionManager()
        let transport = CodexRunawayGuardrailTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)
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

        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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
            outputs: [],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: []
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["codex-guard-1", "codex-guard-2"])

        let lineageID = try await sessionManager.getOrCreateLineage(
            runID: runID,
            agentID: agent.id,
            scope: agent.sessionReuseScope,
            familyID: agent.sessionFamilyID
        )
        let lineage = try #require(try await sessionManager.getLineage(id: lineageID))
        #expect(lineage.activeGenerationID == nil)
        #expect(lineage.generations.count == 2)

        let generations = lineage.generations.sorted { $0.generation < $1.generation }
        #expect(generations[0].status == .invalidated)
        #expect(generations[0].endReason?.localizedCaseInsensitiveContains("runaway guardrail") == true)
        #expect(generations[1].status == .closed)
        #expect(lineage.events.contains { $0.generationID == generations[0].id && $0.eventType == .invalidated })
        #expect(lineage.events.contains { $0.generationID == generations[1].id && $0.eventType == .closed })
    }

    @MainActor
    @Test("Session manager supersedes a lingering active generation when creating a fresh generation")
    func sessionManagerSupersedesLingeringActiveGeneration() async throws {
        let sessionManager = try makeSessionManager()
        let runID = UUID()
        let lineageID = try await sessionManager.getOrCreateLineage(
            runID: runID,
            agentID: "code_writer",
            scope: .same_invocation_owner,
            familyID: nil
        )

        let firstGenerationID = try await sessionManager.createGeneration(
            lineageID: lineageID,
            invocationOwnerKey: "owner-1",
            providerSessionID: "session-1",
            bindingFingerprint: "fingerprint-1",
            workingDirectory: "/tmp/work-1",
            workspaceMode: "read_write",
            runtimeProvider: "codex_acp",
            runtimeModel: "gpt-5.4"
        )

        let secondGenerationID = try await sessionManager.createGeneration(
            lineageID: lineageID,
            invocationOwnerKey: "owner-2",
            providerSessionID: "session-2",
            bindingFingerprint: "fingerprint-2",
            workingDirectory: "/tmp/work-2",
            workspaceMode: "read_write",
            runtimeProvider: "codex_acp",
            runtimeModel: "gpt-5.4"
        )

        let lineage = try #require(try await sessionManager.getLineage(id: lineageID))
        #expect(lineage.activeGenerationID == secondGenerationID)
        #expect(lineage.generations.count == 2)

        let firstGeneration = try #require(lineage.generations.first(where: { $0.id == firstGenerationID }))
        let secondGeneration = try #require(lineage.generations.first(where: { $0.id == secondGenerationID }))
        #expect(firstGeneration.status == .invalidated)
        #expect(firstGeneration.endReason == "Superseded by new generation")
        #expect(secondGeneration.status == .active)
        #expect(lineage.events.contains { $0.generationID == firstGenerationID && $0.eventType == .invalidated })
    }

    @Test("Codex ACP proposal authoring gets a longer execution-age guardrail than implementation")
    func codexExecutionAgeGuardrailIsLongerForProposalAuthoring() {
        let proposalAuthoringAgent = makeAgent(id: "proposal_writer", mode: "proposal_authoring")
        let proposalReviewerAgent = makeAgent(id: "proposal_reviewer_architect", mode: "proposal_review.architect")
        let implementationAgent = makeAgent(
            id: "code_writer",
            mode: "implementation",
            outputs: ["changed_files_manifest"],
            worktreeWriteEnabled: true
        )

        #expect(
            RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: proposalAuthoringAgent)
            > RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: implementationAgent)
        )
        #expect(
            RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: proposalAuthoringAgent) == 1_200
        )
        #expect(
            RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: proposalReviewerAgent)
            > RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: proposalAuthoringAgent)
        )
        #expect(
            RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: proposalReviewerAgent) == 1_800
        )
        #expect(
            RuntimeAgentExecutor.codexACPExecutionAgeLimitSeconds(for: implementationAgent)
            == RuntimeAgentExecutor.codexACPMaxExecutionSeconds
        )
    }

    @Test("ACP proposal reviewers use a longer idle-after-progress watchdog than generic ACP agents")
    func acpProposalReviewUsesLongerIdleAfterProgressWatchdog() {
        let originalProposalReviewIdle = RuntimeAgentExecutor.acpProposalReviewIdleAfterProgressDeadlineSeconds
        let originalGenericIdle = RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds
        RuntimeAgentExecutor.acpProposalReviewIdleAfterProgressDeadlineSeconds = 1_200
        RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds = 300
        defer {
            RuntimeAgentExecutor.acpProposalReviewIdleAfterProgressDeadlineSeconds = originalProposalReviewIdle
            RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds = originalGenericIdle
        }

        let proposalReviewerAgent = makeAgent(id: "proposal_reviewer_architect", mode: "proposal_review.architect")
        let implementationAgent = makeAgent(id: "code_writer", mode: "implementation")

        #expect(
            RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds(for: proposalReviewerAgent)
            > RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds(for: implementationAgent)
        )
        #expect(RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds(for: proposalReviewerAgent) == 1_200)
        #expect(RuntimeAgentExecutor.acpIdleAfterProgressDeadlineSeconds(for: implementationAgent) == 300)
    }

    @Test("Codex ACP default runaway tool-call ceiling allows moderate discovery-heavy turns")
    func codexDefaultRunawayToolCallCeiling() {
        #expect(RuntimeAgentExecutor.codexACPMaxToolCallCount == 300)
    }

    @Test("Codex ACP default raw payload ceiling allows larger implementation turns")
    func codexDefaultRawPayloadCeiling() {
        #expect(RuntimeAgentExecutor.codexACPMaxRawToolPayloadBytes == 10_000_000)
    }

    @MainActor
    @Test("Executor retries codex ACP after oversized raw tool payload guardrail trips")
    func executorRetriesCodexAfterOversizedRawToolPayloadGuardrail() async throws {
        let originalMaxRawPayloadBytes = RuntimeAgentExecutor.codexACPMaxRawToolPayloadBytes
        let originalPollInterval = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.codexACPMaxRawToolPayloadBytes = 256
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.codexACPMaxRawToolPayloadBytes = originalMaxRawPayloadBytes
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPollInterval
        }

        let transport = CodexOversizedPayloadTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let baseContext = makeContext(iteration: 1)
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

        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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
            outputs: [],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: []
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["codex-payload-1", "codex-payload-2"])
    }

    @MainActor
    @Test("Executor retries codex ACP after runtime home growth guardrail trips")
    func executorRetriesCodexAfterRuntimeHomeGuardrail() async throws {
        let originalMaxRuntimeHomeBytes = RuntimeAgentExecutor.codexACPMaxRuntimeHomeBytes
        let originalPollInterval = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.codexACPMaxRuntimeHomeBytes = 256
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10

        let runtimeHomeURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("runtime-home-guardrail-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: runtimeHomeURL, withIntermediateDirectories: true)

        defer {
            RuntimeAgentExecutor.codexACPMaxRuntimeHomeBytes = originalMaxRuntimeHomeBytes
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPollInterval
            try? FileManager.default.removeItem(at: runtimeHomeURL)
        }

        let transport = CodexRuntimeHomeGuardrailTransport(runtimeHomeURL: runtimeHomeURL)
        let executor = RuntimeAgentExecutor(transport: transport)

        let baseContext = makeContext(iteration: 1)
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

        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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
            outputs: [],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(
            agent: "proposal_writer",
            task: "draft_initial_proposal",
            inputs: nil,
            outputs: []
        )

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
    }

    @MainActor
    @Test("Executor retries codex ACP after session history token budget trips")
    func executorRetriesCodexAfterSessionHistoryTokenBudgetTrips() async throws {
        let originalMaxInputTokens = RuntimeAgentExecutor.codexACPMaxInputTokens
        let originalMaxCachedInputTokens = RuntimeAgentExecutor.codexACPMaxCachedInputTokens
        let originalPollInterval = RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds
        RuntimeAgentExecutor.codexACPMaxInputTokens = 500_000
        RuntimeAgentExecutor.codexACPMaxCachedInputTokens = 400_000
        RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = 10
        defer {
            RuntimeAgentExecutor.codexACPMaxInputTokens = originalMaxInputTokens
            RuntimeAgentExecutor.codexACPMaxCachedInputTokens = originalMaxCachedInputTokens
            RuntimeAgentExecutor.codexACPGuardrailPollIntervalMilliseconds = originalPollInterval
        }

        let transport = CodexSessionHistoryGuardrailTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let baseContext = makeContext(iteration: 1)
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
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
            outputs: [],
            sessionReuseScope: .same_invocation_owner
        )
        let task = AgentTask(agent: "proposal_writer", task: "draft_initial_proposal", inputs: nil, outputs: [])

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["codex-history-1", "codex-history-2"])
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
        let originalThreshold = RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold
        let originalPoll = RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds
        RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = 0.1
        RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold = 2
        RuntimeAgentExecutor.acpProposalReviewStallPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.acpProposalReviewStallSilenceSeconds = originalSilence
            RuntimeAgentExecutor.acpProposalReviewReadLoopThreshold = originalThreshold
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
    @Test("Executor surfaces watchdog first-progress hangs without performing retry lineage itself")
    func executorSurfacesWatchdogFirstProgressHang() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalFirstProgress = RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds
        let originalPoll = RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = 0.05
        RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = originalFirstProgress
            RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = originalPoll
        }

        let transport = WatchdogTimeoutTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext()
        let binding = ResolvedProviderBinding(
            agentID: "test_agent",
            backendProfileID: "claude_orchestrator_high",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            projectRoot: baseContext.projectRoot,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            ideaAttachmentPath: baseContext.ideaAttachmentPath,
            providerBinding: binding
        )

        let result = try await executor.execute(
            task: makeTask(),
            agent: makeAgent(),
            context: context
        )

        #expect(!result.succeeded)
        #expect(result.supervisionClassification == .idleHangBeforeFirstProgress)
        #expect(result.outputs["test_agent_receipt.json"] != nil)
        #expect(result.outputs["test_agent_transcript.md"] != nil)
        try await Task.sleep(for: .milliseconds(100))
        #expect(await transport.createSessionCallCount == 1)
    }

    @MainActor
    @Test("Executor does not classify streaming thinking activity as before-first-progress hang")
    func executorDoesNotKillStreamingThinkingBeforeOutput() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalFirstProgress = RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds
        let originalPoll = RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = 0.05
        RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = originalFirstProgress
            RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = originalPoll
        }

        let transport = StreamingThinkingBeforeOutputTransport()
        let executor = RuntimeAgentExecutor(transport: transport)
        let baseContext = makeContext()
        let binding = ResolvedProviderBinding(
            agentID: "lead_orchestrator",
            backendProfileID: "claude_orchestrator_high",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            projectRoot: baseContext.projectRoot,
            stageID: "state_4_proposal_reviewed",
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            ideaAttachmentPath: baseContext.ideaAttachmentPath,
            providerBinding: binding
        )

        let result = try await executor.execute(
            task: makeTask(agent: "lead_orchestrator", task: "aggregate_proposal_reviews"),
            agent: makeAgent(id: "lead_orchestrator", outputs: ["proposal_review_summary"]),
            context: context
        )

        #expect(result.succeeded)
        #expect(result.supervisionClassification == nil)
        #expect(result.errorMessage == nil)
    }

    @MainActor
    @Test("Executor invalidates the active session generation before returning watchdog failure truth")
    func executorInvalidatesGenerationBeforeReturningWatchdogFailure() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalFirstProgress = RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds
        let originalPoll = RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = 0.05
        RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.acpFirstProgressDeadlineSeconds = originalFirstProgress
            RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = originalPoll
        }

        let sessionManager = try makeSessionManager()
        let transport = WatchdogTimeoutTransport()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)
        let runID = UUID()
        let baseContext = makeContext(runID: runID)
        let binding = ResolvedProviderBinding(
            agentID: "test_agent",
            backendProfileID: "claude_orchestrator_high",
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
        let context = ExecutionContext(
            workspace: baseContext.workspace,
            stageID: baseContext.stageID,
            stageLineageID: baseContext.stageLineageID,
            ownerExecutionLineageID: baseContext.ownerExecutionLineageID,
            iteration: baseContext.iteration,
            attemptNumber: baseContext.attemptNumber,
            inputArtifacts: baseContext.inputArtifacts,
            inputArtifactPaths: baseContext.inputArtifactPaths,
            variables: baseContext.variables,
            ideaBody: baseContext.ideaBody,
            providerBinding: binding
        )
        let agent = makeAgent()

        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: context
        )

        #expect(!result.succeeded)
        #expect(result.supervisionClassification == .idleHangBeforeFirstProgress)

        let lineageID = try await sessionManager.getOrCreateLineage(
            runID: runID,
            agentID: agent.id,
            scope: agent.sessionReuseScope,
            familyID: agent.sessionFamilyID
        )
        let lineage = try #require(try await sessionManager.getLineage(id: lineageID))
        #expect(lineage.activeGenerationID == nil)
        let generation = try #require(lineage.generations.first)
        #expect(generation.id == result.sessionGenerationID)
        #expect(generation.status == .invalidated)
        #expect(generation.endReason?.contains("idle_hang_before_first_progress") == true)
        #expect(lineage.events.contains { $0.generationID == generation.id && $0.eventType == .invalidated })
    }

    @MainActor
    @Test("Executor fails closed when mutating tool success produces no filesystem side effect")
    func executorFailsClosedOnMutationSideEffectMissing() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalMutationDeadline = RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds
        let originalPoll = RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds = 0.05
        RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds = originalMutationDeadline
            RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = originalPoll
        }

        let transport = CompletedMutationWithoutSideEffectTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let worktreeRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("mutation-missing-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)

        let binding = ResolvedProviderBinding(
            agentID: "code_writer",
            backendProfileID: "codex_builder_high",
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
        let context = makeContext(worktreeRoot: worktreeRoot)
        let writeContext = ExecutionContext(
            workspace: context.workspace,
            projectRoot: context.projectRoot,
            stageID: "state_8_implementation_continued",
            stageLineageID: "state_8_implementation_continued",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            inputArtifactPaths: [:],
            variables: [:],
            ideaBody: "Test idea body",
            providerBinding: binding
        )

        let result = try await executor.execute(
            task: makeTask(agent: "code_writer", task: "continue_implementation"),
            agent: makeAgent(id: "code_writer", outputs: ["changed_files_manifest"], worktreeWriteEnabled: true),
            context: writeContext
        )

        #expect(!result.succeeded)
        #expect(result.supervisionClassification == .mutationSideEffectMissing)
    }

    @MainActor
    @Test("Executor treats started-but-unfinished edit as post-edit hang, not missing side effect")
    func executorTreatsStartedButUnfinishedEditAsHang() async throws {
        let originalTimeout = RuntimeAgentExecutor.executionTimeoutSeconds
        let originalMutationDeadline = RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds
        let originalFirstEditDeadline = RuntimeAgentExecutor.acpFirstEditSilenceDeadlineSeconds
        let originalPoll = RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds
        RuntimeAgentExecutor.executionTimeoutSeconds = 1
        RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds = 1
        RuntimeAgentExecutor.acpFirstEditSilenceDeadlineSeconds = 0.05
        RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = 20
        defer {
            RuntimeAgentExecutor.executionTimeoutSeconds = originalTimeout
            RuntimeAgentExecutor.acpMutationSideEffectDeadlineSeconds = originalMutationDeadline
            RuntimeAgentExecutor.acpFirstEditSilenceDeadlineSeconds = originalFirstEditDeadline
            RuntimeAgentExecutor.acpWatchdogPollIntervalMilliseconds = originalPoll
        }

        let transport = StartedEditWithoutCompletionTransport()
        let executor = RuntimeAgentExecutor(transport: transport)

        let worktreeRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("started-edit-hang-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)

        let binding = ResolvedProviderBinding(
            agentID: "code_writer",
            backendProfileID: "codex_builder_high",
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
        let context = makeContext(worktreeRoot: worktreeRoot)
        let writeContext = ExecutionContext(
            workspace: context.workspace,
            projectRoot: context.projectRoot,
            stageID: "state_8_implementation_continued",
            stageLineageID: "state_8_implementation_continued",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            inputArtifactPaths: [:],
            variables: [:],
            ideaBody: "Test idea body",
            providerBinding: binding
        )

        let result = try await executor.execute(
            task: makeTask(agent: "code_writer", task: "continue_implementation"),
            agent: makeAgent(id: "code_writer", outputs: ["changed_files_manifest"], worktreeWriteEnabled: true),
            context: writeContext
        )

        #expect(!result.succeeded)
        #expect(result.supervisionClassification == .idleHangAfterFirstEdit)
        #expect(result.errorMessage?.contains("'edit'") == true)
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

    @MainActor
    @Test("Executor invalidates silent Codex EOF generations before retrying with session reuse enabled")
    func executorInvalidatesSilentEOFGenerationBeforeRetry() async throws {
        let transport = SilentEOFRetryTransport()
        let sessionManager = try makeSessionManager()
        let executor = RuntimeAgentExecutor(transport: transport, sessionManager: sessionManager)
        let runID = UUID()

        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "drafting",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 10,
            temperature: 0,
            permissionProfile: "read_only",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "Refine the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"],
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "proposal_loop"
        )

        let result = try await executor.execute(
            task: makeTask(agent: "proposal_writer", task: "revise_proposal"),
            agent: agent,
            context: makeContext(runID: runID)
        )

        #expect(result.succeeded)
        #expect(result.outputs["proposal_current"] != nil)
        #expect(result.sessionID == "session-eof-2")
        #expect(result.sessionReuseDisposition == .fresh_after_transport_error)
        #expect(await transport.createSessionCallCount == 2)
        #expect(await transport.submitPromptCallCount == 2)
        #expect(await transport.submittedSessionIDs == ["session-eof-1", "session-eof-2"])
    }
}
