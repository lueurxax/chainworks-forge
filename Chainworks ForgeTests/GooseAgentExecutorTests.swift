import Testing
import Foundation
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
            ideaBody: "Test idea body",
            providerBinding: nil
        )
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
        let closeSessionCalled = await transport.closeSessionCalled
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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .promptSubmitted(raw: "{}"),
            .toolCallStarted(toolName: "write_file", raw: "{}"),
            .toolCallFinished(toolName: "write_file", raw: "{}"),
            .textChunk(text: "Working on it..."),
            .finalOutput(content: "Done!"),
            .sessionClosed(raw: "{}")
        ])

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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .finalOutput(content: "Test output"),
            .sessionClosed(raw: "{}")
        ])

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
            #expect(receipt.receiptVersion == "1.0")
        }
    }

    /// testGooseExecutorFailsWhenRequiredOutputsMissing — Section 12.1
    @MainActor
    @Test("Executor fails when required outputs are missing from stream")
    func gooseExecutorFailsWhenRequiredOutputsMissing() async throws {
        let transport = ObservableGooseTransport()
        // Stream completes but no files are written and no final output
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .sessionClosed(raw: "{}")
        ])

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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .toolCallStarted(toolName: "read_file", raw: "{}"),
            .toolCallFinished(toolName: "read_file", raw: "{}"),
            .finalOutput(content: "# My Proposal\n\nThis is a great proposal."),
            .sessionClosed(raw: "{}")
        ])

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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
            .sessionClosed(raw: "{}")
        ])

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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .textChunk(text: "partial draft"),
            .finish(reason: "max_tokens", totalTokens: 128, raw: #"{"type":"Finish","reason":"max_tokens"}"#),
            .sessionClosed(raw: "{}")
        ])

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
        await transport.configure(events: [
            .sessionStarted(raw: "{}"),
            .finish(reason: "stop", totalTokens: 42, raw: #"{"type":"Finish","reason":"stop"}"#),
            .sessionClosed(raw: "{}")
        ])

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
            sessionError: GooseTransportError.httpError(
                statusCode: 500,
                body: "Internal server error"
            )
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
    @Test("Executor fails without policy acknowledgement in session response")
    func gooseExecutorFailsWithoutPolicyAcknowledgement() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-missing-ack",
                status: "active",
                policyAcknowledgement: nil
            )
        )

        let executor = GooseAgentExecutor(transport: transport)
        let agent = makeAgent(outputs: ["proposal_current"])
        let task = makeTask()
        let context = makeContext()

        let result = try await executor.execute(task: task, agent: agent, context: context)

        #expect(!result.succeeded)
        #expect(result.errorMessage?.contains("read-only execution policy") == true)
    }
}
