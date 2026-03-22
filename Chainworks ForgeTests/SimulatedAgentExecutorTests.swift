import XCTest
@testable import Chainworks_Forge

final class SimulatedAgentExecutorTests: XCTestCase {

    // MARK: - Helpers

    private func makeAgent(
        id: String = "test_agent",
        outputs: [String] = ["test_output"],
        outputContract: String? = nil
    ) -> ResolvedAgent {
        ResolvedAgent(
            id: id, title: "Test Agent", mode: "tool_use",
            provider: "claude_code", model: "opus", effort: "high",
            maxTurns: 10, temperature: 0.0, permissionProfile: "ORCH",
            skillRef: "sk1", skillRole: nil, prompt: "test prompt",
            outputContract: outputContract, requiresHumanApproval: false,
            inputs: [], outputs: outputs
        )
    }

    private func makeContext(stageID: String = "stage_1") -> ExecutionContext {
        ExecutionContext(
            workspace: RunWorkspace(
                runID: UUID(),
                workspaceRoot: URL(fileURLWithPath: "/tmp/test"),
                artifactRoot: URL(fileURLWithPath: "/tmp/test/artifacts"),
                worktreeRoot: nil
            ),
            stageID: stageID,
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea"
        )
    }

    private func makeTask(agent: String = "test_agent", task: String = "do_work") -> AgentTask {
        AgentTask(agent: agent, task: task, inputs: nil, outputs: nil)
    }

    // MARK: - Basic Execution

    func testSuccessfulExecution() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent()
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        XCTAssertTrue(result.succeeded)
        XCTAssertNil(result.errorMessage)
        XCTAssertFalse(result.outputs.isEmpty)
        XCTAssertNotNil(result.logSnippet)
    }

    func testOutputsGeneratedForDeclaredOutputs() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(outputs: ["proposal_current", "idea_brief"])
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        XCTAssertTrue(result.succeeded)
        XCTAssertEqual(result.outputs.count, 2)
        XCTAssertNotNil(result.outputs["proposal_current"])
        XCTAssertNotNil(result.outputs["idea_brief"])
    }

    func testDefaultOutputWhenNoOutputsDeclared() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(outputs: [])
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        XCTAssertTrue(result.succeeded)
        XCTAssertEqual(result.outputs.count, 1)
        XCTAssertNotNil(result.outputs["test_agent_output"])
    }

    // MARK: - Contract-Aware Output

    func testContractAwareOutput() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(
            id: "reviewer",
            outputs: ["review"],
            outputContract: "proposal_review_v1"
        )
        let result = try await executor.execute(
            task: makeTask(agent: "reviewer"),
            agent: agent,
            context: makeContext()
        )
        XCTAssertTrue(result.succeeded)

        // Verify JSON is valid and has required fields
        let data = result.outputs["review"]!
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertNotNil(json["agent_id"])
        XCTAssertNotNil(json["score"])
        XCTAssertNotNil(json["verdict"])
    }

    // MARK: - Failure Injection

    func testFailingAgent() async throws {
        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["bad_agent"]
        let agent = makeAgent(id: "bad_agent")
        let result = try await executor.execute(
            task: makeTask(agent: "bad_agent"),
            agent: agent,
            context: makeContext()
        )
        XCTAssertFalse(result.succeeded)
        XCTAssertNotNil(result.errorMessage)
        XCTAssertTrue(result.outputs.isEmpty)
    }

    // MARK: - Execution Tracking

    func testExecutionTracking() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(id: "tracked_agent")

        _ = try await executor.execute(
            task: makeTask(agent: "tracked_agent", task: "task_1"),
            agent: agent,
            context: makeContext(stageID: "stage_1")
        )
        _ = try await executor.execute(
            task: makeTask(agent: "tracked_agent", task: "task_2"),
            agent: agent,
            context: makeContext(stageID: "stage_2")
        )

        XCTAssertEqual(executor.executedTasks.count, 2)
        XCTAssertEqual(executor.executedTasks[0].task, "task_1")
        XCTAssertEqual(executor.executedTasks[1].stageID, "stage_2")
    }

    func testReset() async throws {
        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["a"]
        let agent = makeAgent()
        _ = try await executor.execute(task: makeTask(), agent: agent, context: makeContext())

        executor.reset()
        XCTAssertTrue(executor.executedTasks.isEmpty)
        XCTAssertTrue(executor.failingAgentIDs.isEmpty)
    }

    // MARK: - OutputContractTemplates Coverage

    func testAllContractTemplatesProduceValidJSON() throws {
        let contractIDs = [
            "proposal_review_v1",
            "proposal_review_summary_v1",
            "implementation_self_assessment_v1",
            "audit_report_v1",
            "security_report_v1",
            "prepush_review_v1",
            "implementation_review_summary_v1",
            "docs_report_v1",
            "git_push_receipt_v1",
            "connect_upload_receipt_v1"
        ]

        for contractID in contractIDs {
            let (data, format) = OutputContractTemplates.generate(
                contractID: contractID,
                agentID: "test",
                stageID: "stage"
            )
            XCTAssertEqual(format, .json, "Contract \(contractID) should be JSON")
            // Verify valid JSON
            let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            XCTAssertNotNil(json, "Contract \(contractID) should produce valid JSON dictionary")
        }
    }

    func testUnknownContractProducesMarkdown() {
        let (data, format) = OutputContractTemplates.generate(
            contractID: "unknown_contract",
            agentID: "test",
            stageID: "stage"
        )
        XCTAssertEqual(format, .markdown)
        let text = String(data: data, encoding: .utf8)!
        XCTAssertTrue(text.contains("Simulated Output"))
    }

    func testCostTracking() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent()
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        XCTAssertNotNil(result.costCents)
        XCTAssertTrue(result.costCents! >= 5 && result.costCents! <= 50)
    }
}
