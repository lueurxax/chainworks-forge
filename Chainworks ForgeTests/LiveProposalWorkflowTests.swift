import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - LiveProposalWorkflowTests (Proposal 004, Section 12.1)

/// Tests for the live proposal loop workflow compilation, agent resolution,
/// and fan-out parallelism recording.
@MainActor
@Suite("Live Proposal Workflow", .tags(.live))
struct LiveProposalWorkflowTests {

    // MARK: - Helpers

    private func loadLiveWorkflowAndCatalog() throws -> (WorkflowDefinition, AgentCatalog) {
        let workflow = try loadTestLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        return (workflow, catalog)
    }

    // MARK: - Tests

    /// testLiveProposalWorkflowCompiles — Section 12.1
    @Test("Live proposal workflow compiles with expected states")
    func liveProposalWorkflowCompiles() throws {
        let (workflow, _) = try loadLiveWorkflowAndCatalog()

        // Workflow ID should match
        #expect(workflow.workflow.id == "proposal_loop_live")
        #expect(workflow.workflow.name == "Proposal Loop (Live)")

        // Should have 6 states
        #expect(workflow.states.count == 6, "Live proposal workflow should have 6 states")

        // Should have an initial state
        #expect(workflow.initialState == "state_1_idea_received")

        // Validate state IDs
        let expectedStates = [
            "state_1_idea_received",
            "state_2_proposal_drafted",
            "state_3_proposal_reviewed",
            "state_4_proposal_refined",
            "state_5_proposal_approval",
            "state_6_workflow_complete"
        ]
        for stateID in expectedStates {
            #expect(workflow.states.keys.contains(stateID), "Missing state: \(stateID)")
        }

        // Start and end states
        #expect(workflow.states["state_1_idea_received"]?.type == "start")
        #expect(workflow.states["state_6_workflow_complete"]?.type == "end")

        // Approval gate
        #expect(workflow.states["state_5_proposal_approval"]?.approval == "required")
    }

    /// testLiveProposalWorkflowUsesExpectedAgents — Section 12.1
    @Test("Live proposal workflow references expected agents")
    func liveProposalWorkflowUsesExpectedAgents() throws {
        let (workflow, _) = try loadLiveWorkflowAndCatalog()

        // Collect all agent IDs referenced in the workflow
        var referencedAgents: Set<String> = []
        for (_, state) in workflow.states {
            referencedAgents.insert(state.owner)
            if let seq = state.run?.sequence {
                for task in seq { referencedAgents.insert(task.agent) }
            }
            if let par = state.run?.parallel {
                for task in par { referencedAgents.insert(task.agent) }
            }
            if let then = state.run?.then {
                for task in then { referencedAgents.insert(task.agent) }
            }
        }

        // Expected agent subset from Proposal 004 scope
        let expectedAgents: Set<String> = [
            "lead_orchestrator",
            "proposal_writer",
            "proposal_reviewer_product_owner",
            "proposal_reviewer_ux",
            "proposal_reviewer_ui",
            "proposal_reviewer_architect"
        ]

        for agent in expectedAgents {
            #expect(referencedAgents.contains(agent),
                    "Workflow should reference agent: \(agent)")
        }
    }

    /// testReviewFanoutParallelismIsRecordedCorrectly — Section 12.1
    @Test("Review fan-out parallelism is recorded correctly")
    func reviewFanoutParallelismIsRecordedCorrectly() throws {
        let (workflow, _) = try loadLiveWorkflowAndCatalog()

        // state_3_proposal_reviewed should have a parallel block with 4 reviewers
        guard let reviewState = workflow.states["state_3_proposal_reviewed"] else {
            Issue.record("Missing state: state_3_proposal_reviewed")
            return
        }

        let parallelTasks = reviewState.run?.parallel ?? []
        #expect(parallelTasks.count == 4,
                "Review fan-out should have 4 parallel reviewer tasks")

        let parallelAgents = Set(parallelTasks.map(\.agent))
        #expect(parallelAgents.contains("proposal_reviewer_product_owner"))
        #expect(parallelAgents.contains("proposal_reviewer_ux"))
        #expect(parallelAgents.contains("proposal_reviewer_ui"))
        #expect(parallelAgents.contains("proposal_reviewer_architect"))

        // Should also have a 'then' block with the lead_orchestrator aggregation
        let thenTasks = reviewState.run?.then ?? []
        #expect(thenTasks.count == 1, "Should have one aggregation task after parallel")
        #expect(thenTasks.first?.agent == "lead_orchestrator")
    }

    /// testLiveWorkflowHasLoopConfig
    @Test("Live workflow has loop configuration")
    func liveWorkflowHasLoopConfig() throws {
        let (workflow, _) = try loadLiveWorkflowAndCatalog()

        // state_3 should have a loop config
        guard let reviewState = workflow.states["state_3_proposal_reviewed"] else {
            Issue.record("Missing state: state_3_proposal_reviewed")
            return
        }

        #expect(reviewState.loop != nil, "Review state should have a loop config")
        #expect(reviewState.loop?.counter == "proposal_revision_count")
        #expect(reviewState.loop?.max == "vars.max_proposal_revision_cycles")
    }

    /// testLiveWorkflowVariables
    @Test("Live workflow has expected variables")
    func liveWorkflowVariables() throws {
        let (workflow, _) = try loadLiveWorkflowAndCatalog()

        let variables = workflow.variables ?? [:]
        #expect(variables["max_proposal_revision_cycles"] != nil)
        #expect(variables["proposal_score_target"] != nil)
        #expect(variables["min_individual_proposal_score"] != nil)
    }

    @Test("Review transitions use average score and blockers to loop back for refinement")
    func liveWorkflowReviewTransitionUsesAverageScoreAndBlockers() throws {
        let (_, catalog) = try loadLiveWorkflowAndCatalog()
        let workflow = try loadTestLiveWorkflow()
        let (_, modelContext) = try makeTestModelContainer()
        let plan = try RunPlanCompiler(modelContext: modelContext).previewCompile(
            workflow: workflow,
            catalog: catalog
        )

        guard let reviewState = plan.states["state_3_proposal_reviewed"] else {
            Issue.record("Missing state: state_3_proposal_reviewed")
            return
        }

        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: ["proposal_review_summary"],
            approvalGranted: false,
            approvalRejected: false,
            variables: plan.variables,
            artifactFields: [
                "proposal_review_summary": [
                    "average_score": .double(9.0),
                    "aggregate_score": .int(36),
                    "min_individual_score": .int(9),
                    "blocker_count": .int(1)
                ]
            ]
        )

        let transition = TransitionEvaluator.evaluateFirst(
            transitions: reviewState.transitions,
            context: context
        )

        #expect(transition?.to == "state_4_proposal_refined")
    }

    // MARK: - Receipt Builder Tests

    @Test("Receipt builder produces valid JSON")
    func receiptBuilderProducesValidJSON() throws {
        let receipt = ExecutionReceiptBuilder.buildReceipt(
            agentID: "test_agent",
            sessionID: "session-123",
            stageID: "state_1",
            iteration: 1,
            attemptNumber: 1,
            startedAt: Date(),
            completedAt: Date().addingTimeInterval(5),
            events: [
                ExecutionEvent(type: .sessionStarted, timestamp: Date(), detail: "Started"),
                ExecutionEvent(type: .finalOutput, timestamp: Date(), detail: "Done")
            ],
            toolCalls: [
                ToolCallRecord(
                    toolName: "read_file",
                    rawPayload: "{}",
                    startedAt: Date(),
                    completedAt: Date(),
                    succeeded: true,
                    responseRawPayload: "{}"
                )
            ],
            finalContent: "Test output",
            succeeded: true,
            errorMessage: nil,
            provider: "test_provider",
            model: "test_model",
            effort: "high"
        )

        // Should produce receipt and transcript
        #expect(receipt.keys.contains("test_agent_receipt.json"))
        #expect(receipt.keys.contains("test_agent_transcript.md"))

        // Receipt JSON should be valid
        if let data = receipt["test_agent_receipt.json"] {
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let decoded = try decoder.decode(ExecutionReceipt.self, from: data)
            #expect(decoded.agentID == "test_agent")
            #expect(decoded.sessionID == "session-123")
            #expect(decoded.succeeded)
            #expect(decoded.toolCallCount == 1)
            #expect(decoded.eventCount == 2)
        }

        // Transcript should be non-empty markdown
        if let data = receipt["test_agent_transcript.md"],
           let text = String(data: data, encoding: .utf8) {
            #expect(text.contains("# Execution Transcript"))
            #expect(text.contains("test_agent"))
            #expect(text.contains("session-123"))
        }
    }

    // MARK: - Event Bridge Tests

    @Test("Event bridge processes SSE stream correctly")
    func eventBridgeProcessesStream() async throws {
        let bridge = ExecutionEventBridge()

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.sessionStarted(raw: "{}"))
            continuation.yield(.textChunk(text: "Hello "))
            continuation.yield(.textChunk(text: "World"))
            continuation.yield(.toolCallStarted(toolName: "test_tool", raw: "{}"))
            continuation.yield(.toolCallFinished(toolName: "test_tool", raw: "{}"))
            continuation.yield(.finalOutput(content: "Done"))
            continuation.yield(.sessionClosed(raw: "{}"))
            continuation.finish()
        }

        var events: [ExecutionEvent] = []
        let result = try await bridge.processStream(stream) { event in
            events.append(event)
        }

        #expect(result.succeeded)
        #expect(result.accumulatedText == "Hello World")
        #expect(result.toolCalls.count == 1)
        #expect(result.toolCalls.first?.toolName == "test_tool")
        #expect(result.finalContent == "Done")
        #expect(events.count >= 6)
    }

    @Test("Transcript preserves full text chunk detail instead of truncating to preview")
    func transcriptPreservesFullTextChunkDetail() async throws {
        let bridge = ExecutionEventBridge()
        let longChunk = String(repeating: "refine-proposal-context ", count: 20)

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.sessionStarted(raw: "{}"))
            continuation.yield(.textChunk(text: longChunk))
            continuation.yield(.sessionClosed(raw: "{}"))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }

        let receipt = ExecutionReceiptBuilder.buildReceipt(
            agentID: "proposal_writer",
            sessionID: "session-123",
            stageID: "state_5_proposal_refined",
            iteration: 1,
            attemptNumber: 1,
            startedAt: Date(),
            completedAt: Date().addingTimeInterval(1),
            events: bridge.eventLog,
            toolCalls: bridge.toolCalls,
            finalContent: nil,
            succeeded: true,
            errorMessage: nil,
            provider: "test_provider",
            model: "test_model",
            effort: "high"
        )

        let transcriptData = try #require(receipt["proposal_writer_transcript.md"])
        let transcriptText = try #require(String(data: transcriptData, encoding: .utf8))

        #expect(transcriptText.contains(longChunk))
    }

    @Test("Event bridge coalesces consecutive text chunks in event log while preserving accumulated text")
    func eventBridgeCoalescesConsecutiveTextChunks() async throws {
        let bridge = ExecutionEventBridge()

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.sessionStarted(raw: "{}"))
            continuation.yield(.textChunk(text: "Hel"))
            continuation.yield(.textChunk(text: "lo"))
            continuation.yield(.textChunk(text: " "))
            continuation.yield(.textChunk(text: "world"))
            continuation.yield(.toolCallStarted(toolName: "read", raw: "{}"))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }

        #expect(bridge.accumulatedText == "Hello world")
        let textEvents = bridge.eventLog.filter { $0.type == .textChunk }
        #expect(textEvents.count == 1)
        #expect(textEvents.first?.detail == "Hello world")
    }

    @Test("Event bridge resolves tool name from raw payload when start event name is unknown")
    func eventBridgeResolvesToolNameFromRawPayload() async throws {
        let bridge = ExecutionEventBridge()

        let startRaw = """
        {"type":"toolRequest","id":"call_1","tool":{"name":"read_workspace","arguments":{"path":"/tmp"}}}
        """
        let finishRaw = """
        {"type":"toolResponse","id":"call_1","tool_result":"ok"}
        """

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.toolCallStarted(toolName: "unknown", raw: startRaw))
            continuation.yield(.toolCallFinished(toolName: "call_1", raw: finishRaw))
            continuation.finish()
        }

        var events: [ExecutionEvent] = []
        _ = try await bridge.processStream(stream) { event in
            events.append(event)
        }

        #expect(events.count == 2)
        #expect(events[0].detail == "Tool: read_workspace")
        #expect(events[0].toolName == "read_workspace")
        #expect(events[1].detail == "Tool completed: read_workspace")
        #expect(events[1].toolName == "read_workspace")
        #expect(bridge.toolCalls.count == 1)
        #expect(bridge.toolCalls.first?.toolName == "read_workspace")
    }

    @Test("Event bridge correlates finish event id back to started tool name")
    func eventBridgeCorrelatesToolFinishByCallID() async throws {
        let bridge = ExecutionEventBridge()

        let startRaw = """
        {"type":"toolRequest","id":"call_ZtXPFtI6eGoipm7jvqlq0svN","tool_name":"inspect_repo_context"}
        """
        let finishRaw = """
        {"type":"toolResponse","id":"call_ZtXPFtI6eGoipm7jvqlq0svN","tool_result":{"status":"ok"}}
        """

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.toolCallStarted(toolName: "unknown", raw: startRaw))
            continuation.yield(.toolCallFinished(toolName: "call_ZtXPFtI6eGoipm7jvqlq0svN", raw: finishRaw))
            continuation.finish()
        }

        var events: [ExecutionEvent] = []
        _ = try await bridge.processStream(stream) { event in
            events.append(event)
        }

        #expect(events.count == 2)
        #expect(events[0].toolName == "inspect_repo_context")
        #expect(events[1].toolName == "inspect_repo_context")
        #expect(events[1].detail == "Tool completed: inspect_repo_context")
    }

    @Test("Event bridge captures Codex usage telemetry from usage_update events")
    func eventBridgeCapturesCodexUsageTelemetry() async throws {
        let bridge = ExecutionEventBridge()
        let usageRaw = """
        {
          "sessionUpdate": "usage_update",
          "usage": {
            "input_tokens": 808763,
            "cached_input_tokens": 729088,
            "output_tokens": 2142,
            "model_context_window": 258400
          }
        }
        """

        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.promptSubmitted(raw: #"{"session_id":"session-telemetry"}"#))
            continuation.yield(.toolCallStarted(toolName: "search", raw: "{}"))
            continuation.yield(.unknown(type: "usage_update", data: usageRaw))
            continuation.yield(.toolCallStarted(toolName: "write", raw: "{}"))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }
        let telemetry = try #require(bridge.codexTelemetry(runtimeHomeBytes: 4096))

        #expect(telemetry.inputTokens == 808763)
        #expect(telemetry.cachedInputTokens == 729088)
        #expect(telemetry.outputTokens == 2142)
        #expect(telemetry.modelContextWindow == 258400)
        #expect(telemetry.usageUpdateCount == 1)
        #expect(telemetry.discoveryToolCallCount == 1)
        #expect(telemetry.toolCallCount == 2)
        #expect(telemetry.promptSubmittedAt != nil)
        #expect(telemetry.firstEditAt != nil)
        #expect(telemetry.lastMutatingToolName == "write")
        #expect(telemetry.runtimeHomeBytes == 4096)
    }

    @Test("Receipt builder includes Codex telemetry block")
    func receiptBuilderIncludesCodexTelemetryBlock() throws {
        let startedAt = Date()
        let completedAt = startedAt.addingTimeInterval(5)
        let receipt = ExecutionReceiptBuilder.buildReceipt(
            agentID: "code_writer",
            sessionID: "session-telemetry",
            stageID: "state_8_implementation_continued",
            iteration: 1,
            attemptNumber: 1,
            startedAt: startedAt,
            completedAt: completedAt,
            events: [],
            toolCalls: [],
            finalContent: "done",
            succeeded: true,
            errorMessage: nil,
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            codexTelemetry: CodexReceiptTelemetry(
                inputTokens: 808763,
                cachedInputTokens: 729088,
                outputTokens: 2142,
                modelContextWindow: 258400,
                usageUpdateCount: 4,
                toolCallCount: 11,
                discoveryToolCallCount: 8,
                promptSubmittedAt: startedAt,
                firstEditAt: startedAt.addingTimeInterval(3),
                lastMeaningfulProgressAt: completedAt,
                lastToolName: "edit",
                lastMutatingToolName: "edit",
                accumulatedTextBytes: 8192,
                rawToolPayloadBytes: 16384,
                runtimeHomeBytes: 65536,
                runawayReason: nil,
                supervisionClassification: .idleHangAfterFirstEdit,
                silenceDurationSeconds: 120,
                automaticRetryConsumed: true,
                mutationVerificationAttempted: true,
                mutationSideEffectObserved: false,
                firstVerifiedMutatedPath: nil
            )
        )

        let data = try #require(receipt["code_writer_receipt.json"])
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(ExecutionReceipt.self, from: data)

        #expect(decoded.codexTelemetry?.inputTokens == 808763)
        #expect(decoded.codexTelemetry?.cachedInputTokens == 729088)
        #expect(decoded.codexTelemetry?.discoveryToolCallCount == 8)
        #expect(decoded.codexTelemetry?.lastToolName == "edit")
        #expect(decoded.codexTelemetry?.supervisionClassification == .idleHangAfterFirstEdit)
        #expect(decoded.codexTelemetry?.automaticRetryConsumed == true)
        #expect(decoded.codexTelemetry?.mutationVerificationAttempted == true)
        #expect(decoded.codexTelemetry?.mutationSideEffectObserved == false)
        #expect(decoded.codexTelemetry?.runtimeHomeBytes == 65536)
    }
}
