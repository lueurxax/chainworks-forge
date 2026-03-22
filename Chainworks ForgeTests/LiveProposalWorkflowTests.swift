import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - LiveProposalWorkflowTests (Proposal 004, Section 12.1)

/// Tests for the live proposal loop workflow compilation, agent resolution,
/// and fan-out parallelism recording.
final class LiveProposalWorkflowTests: XCTestCase {

    // MARK: - Helpers

    private func loadLiveWorkflowAndCatalog() -> (WorkflowDefinition, AgentCatalog)? {
        let candidates: [String] = [
            "examples/workflows/proposal-loop-live.yaml",
        ]
        let catalogCandidates: [String] = [
            "examples/agents/agents.yaml",
        ]

        var workflowURL: URL?
        var catalogURL: URL?

        for path in candidates {
            let url = URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/\(path)")
            if FileManager.default.isReadableFile(atPath: url.path) {
                workflowURL = url
                break
            }
            // Try from cwd
            let cwdURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent(path)
            if FileManager.default.isReadableFile(atPath: cwdURL.path) {
                workflowURL = cwdURL
                break
            }
        }

        for path in catalogCandidates {
            let url = URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Documents/Chainworks Forge/\(path)")
            if FileManager.default.isReadableFile(atPath: url.path) {
                catalogURL = url
                break
            }
            let cwdURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent(path)
            if FileManager.default.isReadableFile(atPath: cwdURL.path) {
                catalogURL = cwdURL
                break
            }
        }

        guard let wURL = workflowURL, let cURL = catalogURL else {
            return nil
        }

        guard let workflow = try? YAMLParser.loadWorkflow(from: wURL),
              let catalog = try? YAMLParser.loadAgentCatalog(from: cURL) else {
            return nil
        }

        return (workflow, catalog)
    }

    // MARK: - Tests

    /// testLiveProposalWorkflowCompiles — Section 12.1
    func testLiveProposalWorkflowCompiles() throws {
        guard let (workflow, catalog) = loadLiveWorkflowAndCatalog() else {
            throw XCTSkip("proposal-loop-live.yaml or agents.yaml not found at expected paths")
        }

        // Workflow ID should match
        XCTAssertEqual(workflow.workflow.id, "proposal_loop_live")
        XCTAssertEqual(workflow.workflow.name, "Proposal Loop (Live)")

        // Should have 6 states
        XCTAssertEqual(workflow.states.count, 6, "Live proposal workflow should have 6 states")

        // Should have an initial state
        XCTAssertEqual(workflow.initialState, "state_1_idea_received")

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
            XCTAssertTrue(workflow.states.keys.contains(stateID), "Missing state: \(stateID)")
        }

        // Start and end states
        XCTAssertEqual(workflow.states["state_1_idea_received"]?.type, "start")
        XCTAssertEqual(workflow.states["state_6_workflow_complete"]?.type, "end")

        // Approval gate
        XCTAssertEqual(workflow.states["state_5_proposal_approval"]?.approval, "required")
    }

    /// testLiveProposalWorkflowUsesExpectedAgents — Section 12.1
    func testLiveProposalWorkflowUsesExpectedAgents() throws {
        guard let (workflow, _) = loadLiveWorkflowAndCatalog() else {
            throw XCTSkip("proposal-loop-live.yaml or agents.yaml not found at expected paths")
        }

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
            XCTAssertTrue(referencedAgents.contains(agent),
                         "Workflow should reference agent: \(agent)")
        }
    }

    /// testReviewFanoutParallelismIsRecordedCorrectly — Section 12.1
    func testReviewFanoutParallelismIsRecordedCorrectly() throws {
        guard let (workflow, _) = loadLiveWorkflowAndCatalog() else {
            throw XCTSkip("proposal-loop-live.yaml or agents.yaml not found at expected paths")
        }

        // state_3_proposal_reviewed should have a parallel block with 4 reviewers
        guard let reviewState = workflow.states["state_3_proposal_reviewed"] else {
            XCTFail("Missing state: state_3_proposal_reviewed")
            return
        }

        let parallelTasks = reviewState.run?.parallel ?? []
        XCTAssertEqual(parallelTasks.count, 4,
                      "Review fan-out should have 4 parallel reviewer tasks")

        let parallelAgents = Set(parallelTasks.map(\.agent))
        XCTAssertTrue(parallelAgents.contains("proposal_reviewer_product_owner"))
        XCTAssertTrue(parallelAgents.contains("proposal_reviewer_ux"))
        XCTAssertTrue(parallelAgents.contains("proposal_reviewer_ui"))
        XCTAssertTrue(parallelAgents.contains("proposal_reviewer_architect"))

        // Should also have a 'then' block with the lead_orchestrator aggregation
        let thenTasks = reviewState.run?.then ?? []
        XCTAssertEqual(thenTasks.count, 1, "Should have one aggregation task after parallel")
        XCTAssertEqual(thenTasks.first?.agent, "lead_orchestrator")
    }

    /// testLiveWorkflowHasLoopConfig
    func testLiveWorkflowHasLoopConfig() throws {
        guard let (workflow, _) = loadLiveWorkflowAndCatalog() else {
            throw XCTSkip("proposal-loop-live.yaml or agents.yaml not found at expected paths")
        }

        // state_3 should have a loop config
        guard let reviewState = workflow.states["state_3_proposal_reviewed"] else {
            XCTFail("Missing state: state_3_proposal_reviewed")
            return
        }

        XCTAssertNotNil(reviewState.loop, "Review state should have a loop config")
        XCTAssertEqual(reviewState.loop?.counter, "proposal_revision_count")
        XCTAssertEqual(reviewState.loop?.max, "vars.max_proposal_revision_cycles")
    }

    /// testLiveWorkflowVariables
    func testLiveWorkflowVariables() throws {
        guard let (workflow, _) = loadLiveWorkflowAndCatalog() else {
            throw XCTSkip("proposal-loop-live.yaml or agents.yaml not found at expected paths")
        }

        let variables = workflow.variables ?? [:]
        XCTAssertNotNil(variables["max_proposal_revision_cycles"])
        XCTAssertNotNil(variables["proposal_score_target"])
        XCTAssertNotNil(variables["min_individual_proposal_score"])
    }

    // MARK: - Receipt Builder Tests

    func testReceiptBuilderProducesValidJSON() throws {
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
                ToolCallRecord(toolName: "read_file", startedAt: Date(), completedAt: Date(), succeeded: true)
            ],
            finalContent: "Test output",
            succeeded: true,
            errorMessage: nil,
            provider: "test_provider",
            model: "test_model",
            effort: "high"
        )

        // Should produce receipt and transcript
        XCTAssertTrue(receipt.keys.contains("test_agent_receipt.json"))
        XCTAssertTrue(receipt.keys.contains("test_agent_transcript.md"))

        // Receipt JSON should be valid
        if let data = receipt["test_agent_receipt.json"] {
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let decoded = try decoder.decode(ExecutionReceipt.self, from: data)
            XCTAssertEqual(decoded.agentID, "test_agent")
            XCTAssertEqual(decoded.sessionID, "session-123")
            XCTAssertTrue(decoded.succeeded)
            XCTAssertEqual(decoded.toolCallCount, 1)
            XCTAssertEqual(decoded.eventCount, 2)
        }

        // Transcript should be non-empty markdown
        if let data = receipt["test_agent_transcript.md"],
           let text = String(data: data, encoding: .utf8) {
            XCTAssertTrue(text.contains("# Execution Transcript"))
            XCTAssertTrue(text.contains("test_agent"))
            XCTAssertTrue(text.contains("session-123"))
        }
    }

    // MARK: - Event Bridge Tests

    func testEventBridgeProcessesStream() async throws {
        let bridge = ExecutionEventBridge()

        let stream = AsyncThrowingStream<GooseStreamEvent, Error> { continuation in
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

        XCTAssertTrue(result.succeeded)
        XCTAssertEqual(result.accumulatedText, "Hello World")
        XCTAssertEqual(result.toolCalls.count, 1)
        XCTAssertEqual(result.toolCalls.first?.toolName, "test_tool")
        XCTAssertEqual(result.finalContent, "Done")
        XCTAssertTrue(events.count >= 6)
    }
}
