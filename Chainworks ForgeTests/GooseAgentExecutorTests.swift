import Testing
import Foundation
import os
import SwiftData
@testable import Chainworks_Forge

// MARK: - GooseAgentExecutorTests (Proposal 004, Section 12.1)

/// Unit tests for GooseAgentExecutor.
/// Tests session creation, streaming, receipt persistence, output validation, and result building.
@Suite("GooseAgentExecutor")
struct GooseAgentExecutorTests {

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

    private final class PromptCaptureTransport: GooseTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionResult: GooseSessionResponse?
            var createSessionError: Error?
            var streamEvents: [GooseStreamEvent] = []
            var lastSessionRequest: GooseSessionRequest?
            var lastPromptRequest: GoosePromptRequest?
            var closeSessionCalled = false
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func configure(
            sessionResult: GooseSessionResponse? = nil,
            sessionError: Error? = nil,
            events: [GooseStreamEvent] = []
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

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            let (result, error): (GooseSessionResponse?, Error?) = state.withLock { state in
                state.lastSessionRequest = request
                return (state.createSessionResult, state.createSessionError)
            }
            if let error { throw error }
            return result ?? GooseSessionResponse(
                sessionId: "prompt-capture-\(UUID().uuidString.prefix(8))",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
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

        var lastSessionRequest: GooseSessionRequest? {
            get async { state.withLock { $0.lastSessionRequest } }
        }

        var lastPromptRequest: GoosePromptRequest? {
            get async { state.withLock { $0.lastPromptRequest } }
        }

        var closeSessionCalled: Bool {
            get async { state.withLock { $0.closeSessionCalled } }
        }
    }

    private final class StaleReuseTransport: GooseTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
            var sessionUseCounts: [String: Int] = [:]
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-reused" : "session-fresh-\(callCount)"
            return GooseSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
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
                            domain: "GooseTest",
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

    private final class StaleReuseSSEErrorTransport: GooseTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submitPromptCallCount = 0
            var submittedSessionIDs: [String] = []
            var sessionUseCounts: [String: Int] = [:]
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            let callCount = state.withLock { state -> Int in
                state.createSessionCallCount += 1
                return state.createSessionCallCount
            }

            let sessionID = callCount == 1 ? "session-reused" : "session-fresh-\(callCount)"
            return GooseSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
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

    private final class PersistentSessionUnavailableTransport: GooseTransportProtocol, @unchecked Sendable {
        private let counter = OSAllocatedUnfairLock(initialState: 0)

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            let sessionNumber = counter.withLock { value -> Int in
                value += 1
                return value
            }
            return GooseSessionResponse(
                sessionId: "session-\(sessionNumber)",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
            AsyncThrowingStream { continuation in
                Task {
                    continuation.yield(.sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.yield(.promptSubmitted(raw: #"{"session_id":"\#(sessionID)"}"#))
                    continuation.finish(throwing: NSError(
                        domain: "GooseTest",
                        code: 404,
                        userInfo: [NSLocalizedDescriptionKey: "Failed to read session: Session not found"]
                    ))
                }
            }
        }

        func closeSession(sessionID: String) async throws {}
    }

    private final class DuplicateFreshSessionTransport: GooseTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submittedSessionIDs: [String] = []
            var closedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
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

            return GooseSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
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

    private final class RecycledCompletedSessionTransport: GooseTransportProtocol, @unchecked Sendable {
        private struct State {
            var createSessionCallCount = 0
            var submittedSessionIDs: [String] = []
        }

        private let state = OSAllocatedUnfairLock(initialState: State())

        func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
            let sessionID = state.withLock { state -> String in
                state.createSessionCallCount += 1
                return "recycled-session"
            }

            return GooseSessionResponse(
                sessionId: sessionID,
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-token",
                    backendPolicyVersion: "mock-v1"
                )
            )
        }

        func submitPrompt(
            sessionID: String,
            prompt: GoosePromptRequest
        ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
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
            stageLineageID: "state_1",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea body",
            providerBinding: nil
        )
    }

    private func waitForSessionClose(_ transport: ObservableGooseTransport) async -> Bool {
        for _ in 0..<20 {
            let closed = await transport.closeSessionCalled
            if closed { return true }
            try? await Task.sleep(for: .milliseconds(25))
        }
        return await transport.closeSessionCalled
    }

    // MARK: - Tests

    /// testGooseExecutorCreatesSession — Section 12.1
    @MainActor
    @Test("Executor creates session with correct policy and produces receipt/transcript artifacts")
    func gooseExecutorCreatesSession() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-abc123",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
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

        let executor = GooseAgentExecutor(transport: transport)
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

    /// testGooseExecutorStreamsEvents — Section 12.1
    @MainActor
    @Test("Executor streams events to event callback during execution")
    func gooseExecutorStreamsEvents() async throws {
        let transport = ObservableGooseTransport()
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
        let executor = GooseAgentExecutor(transport: transport)
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

    /// testGooseExecutorPersistsReceiptArtifact — Section 12.1
    @MainActor
    @Test("Executor persists receipt artifact with correct agent ID and version")
    func gooseExecutorPersistsReceiptArtifact() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "Test output"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
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
    func gooseExecutorPrefersFrozenProviderBindingForRuntimeTruth() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-bound-truth",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
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
        let executor = GooseAgentExecutor(transport: transport, override: override)
        let configuredProviderID = UUID()
        let binding = ResolvedProviderBinding(
            agentID: "proposal_reviewer_ui",
            backendProfileID: "gemini_review_flash",
            configuredProviderID: configuredProviderID,
            providerFamily: "gemini",
            providerIdentifier: "gemini",
            model: "gemini-2.5-pro",
            effort: "medium",
            transport: "gooseServer",
            adapterVersion: "v1"
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

    /// testGooseExecutorFailsWhenRequiredOutputsMissing — Section 12.1
    @MainActor
    @Test("Executor fails when required outputs are missing from stream")
    func gooseExecutorFailsWhenRequiredOutputsMissing() async throws {
        let transport = ObservableGooseTransport()
        // Stream completes but no files are written and no final output
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["required_output.json"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        // Should fail because required output is missing
        #expect(!result.succeeded, "Should fail when required outputs are missing")
        #expect(result.errorMessage != nil)
        #expect(result.errorMessage?.contains("required_output.json") == true)
    }

    /// testGooseExecutorReturnsAgentResult — Section 12.1
    @MainActor
    @Test("Executor returns agent result with log snippet and cost estimate")
    func gooseExecutorReturnsAgentResult() async throws {
        let transport = ObservableGooseTransport()
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

        let executor = GooseAgentExecutor(transport: transport)
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
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
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
        let transport = ObservableGooseTransport()
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

        let executor = GooseAgentExecutor(transport: transport)
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
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
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

    /// testGooseExecutorSessionCreationFailure
    @MainActor
    @Test("Executor handles session creation failure gracefully")
    func gooseExecutorSessionCreationFailure() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: GooseTransportError.httpError(
                statusCode: 500,
                body: "Internal server error"
            ),
            events: []
        )

        let executor = GooseAgentExecutor(transport: transport)
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
            sessionResult: GooseSessionResponse(
                sessionId: "prompt-capture-session",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
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

        let executor = GooseAgentExecutor(transport: transport)
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
    func gooseExecutorFailsWithoutPolicyAcknowledgement() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-missing-ack",
                status: "active",
                policyAcknowledgement: nil
            ),
            sessionError: nil,
            events: []
        )

        let executor = GooseAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage?.contains("read-only execution policy") == true)
    }

    @MainActor
    @Test("Executor does not retry contract or missing-output failures as transport errors")
    func gooseExecutorDoesNotRetryContractFailures() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-contract-failure",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
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

        let executor = GooseAgentExecutor(transport: transport)
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
    @Test("Executor records lazy evidence hits when on-demand helper is invoked")
    func gooseExecutorRecordsLazyEvidenceHits() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-lazy-hit",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
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

        let executor = GooseAgentExecutor(transport: transport)
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
    func gooseExecutorFallsBackWhenReusedSessionDisappearsMidStream() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("GooseAgentExecutorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = StaleReuseTransport()
        let executor = GooseAgentExecutor(transport: transport, sessionManager: sessionManager)

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
    func gooseExecutorRejectsDuplicateFreshProviderSessionIDsAcrossLineages() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("GooseAgentExecutorCollisionTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = DuplicateFreshSessionTransport()
        let executor = GooseAgentExecutor(transport: transport, sessionManager: sessionManager)

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
    func gooseExecutorIgnoresCompletedGenerationDuringFreshCollisionCheck() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("GooseAgentExecutorCompletedCollisionTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = RecycledCompletedSessionTransport()
        let executor = GooseAgentExecutor(transport: transport, sessionManager: sessionManager)

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
    func gooseExecutorFallsBackWhenReusedSessionEndsWithSSEError() async throws {
        let schema = Schema([AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self])
        let config = ModelConfiguration("GooseAgentExecutorTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let sessionManager = AgentSessionManager(container: container)
        let transport = StaleReuseSSEErrorTransport()
        let executor = GooseAgentExecutor(transport: transport, sessionManager: sessionManager)

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
    func gooseExecutorMapsQuotaErrorToLimitExhaustion() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: nil,
            sessionError: nil,
            events: [
                .sessionStarted(raw: "{}"),
                .promptSubmitted(raw: "{}"),
                .error(message: "Claude monthly quota exhausted; rate limit exceeded")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
        let result = try await executor.execute(task: makeTask(), agent: makeAgent(), context: makeContext())

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .limitExhaustedBeforeOutput)
        #expect(result.errorMessage == "Provider or app limit exhausted")
    }

    @MainActor
    @Test("Executor maps Gemini capacity exhaustion to retryable limit failure after durable output")
    func gooseExecutorMapsGeminiCapacityErrorAfterOutput() async throws {
        let transport = ObservableGooseTransport()
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

        let executor = GooseAgentExecutor(transport: transport)
        let result = try await executor.execute(task: makeTask(), agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.canonicalOutcome == .limitExhaustedAfterOutput)
        #expect(result.transportErrorKind == .provider)
        #expect(result.errorMessage == "Provider capacity exhausted; retry the agent")
    }

    @MainActor
    @Test("Executor surfaces session loss as provider-session unavailable instead of raw not-found text")
    func gooseExecutorSurfacesBoundedSessionUnavailableMessage() async throws {
        let transport = PersistentSessionUnavailableTransport()
        let executor = GooseAgentExecutor(transport: transport)

        let result = try await executor.execute(task: makeTask(), agent: makeAgent(), context: makeContext())

        #expect(!result.succeeded)
        #expect(result.errorMessage == "Provider session became unavailable during execution")
        #expect(result.errorMessage?.contains("Session not found") == false)
    }
}
