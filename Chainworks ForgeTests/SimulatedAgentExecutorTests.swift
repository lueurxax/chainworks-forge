import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("SimulatedAgentExecutor")
struct SimulatedAgentExecutorTests {

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
            ideaBody: "Test idea",
            providerBinding: nil
        )
    }

    private func makeTask(agent: String = "test_agent", task: String = "do_work") -> AgentTask {
        AgentTask(agent: agent, task: task, inputs: nil, outputs: nil)
    }

    // MARK: - Basic Execution

    @Test("Successful execution")
    func successfulExecution() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent()
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        #expect(result.succeeded)
        #expect(result.errorMessage == nil)
        #expect(!result.outputs.isEmpty)
        #expect(result.logSnippet != nil)
    }

    @Test("Outputs generated for declared outputs")
    func outputsGeneratedForDeclaredOutputs() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(outputs: ["proposal_current", "idea_brief"])
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        #expect(result.succeeded)
        #expect(result.outputs.count == 2)
        #expect(result.outputs["proposal_current"] != nil)
        #expect(result.outputs["idea_brief"] != nil)
    }

    @Test("Default output when no outputs declared")
    func defaultOutputWhenNoOutputsDeclared() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent(outputs: [])
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        #expect(result.succeeded)
        #expect(result.outputs.count == 1)
        #expect(result.outputs["test_agent_output"] != nil)
    }

    // MARK: - Contract-Aware Output

    @Test("Contract-aware output")
    func contractAwareOutput() async throws {
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
        #expect(result.succeeded)

        // Verify JSON is valid and has required fields
        let data = result.outputs["review"]!
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        #expect(json["agent_id"] != nil)
        #expect(json["score"] != nil)
        #expect(json["verdict"] != nil)
    }

    // MARK: - Failure Injection

    @Test("Failing agent")
    func failingAgent() async throws {
        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["bad_agent"]
        let agent = makeAgent(id: "bad_agent")
        let result = try await executor.execute(
            task: makeTask(agent: "bad_agent"),
            agent: agent,
            context: makeContext()
        )
        #expect(!result.succeeded)
        #expect(result.errorMessage != nil)
        #expect(result.outputs.isEmpty)
    }

    // MARK: - Execution Tracking

    @Test("Execution tracking")
    func executionTracking() async throws {
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

        #expect(executor.executedTasks.count == 2)
        #expect(executor.executedTasks[0].task == "task_1")
        #expect(executor.executedTasks[1].stageID == "stage_2")
    }

    @Test("Reset")
    func reset() async throws {
        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["a"]
        let agent = makeAgent()
        _ = try await executor.execute(task: makeTask(), agent: agent, context: makeContext())

        executor.reset()
        #expect(executor.executedTasks.isEmpty)
        #expect(executor.failingAgentIDs.isEmpty)
    }

    // MARK: - OutputContractTemplates Coverage

    @Test("All contract templates produce valid JSON")
    func allContractTemplatesProduceValidJSON() throws {
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
            #expect(format == .json, "Contract \(contractID) should be JSON")
            // Verify valid JSON
            let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            #expect(json != nil, "Contract \(contractID) should produce valid JSON dictionary")
        }
    }

    @Test("Unknown contract produces markdown")
    func unknownContractProducesMarkdown() {
        let (data, format) = OutputContractTemplates.generate(
            contractID: "unknown_contract",
            agentID: "test",
            stageID: "stage"
        )
        #expect(format == .markdown)
        let text = String(data: data, encoding: .utf8)!
        #expect(text.contains("Simulated Output"))
    }

    @Test("Cost tracking")
    func costTracking() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent()
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        // §6.2: default 100 cents per execution
        #expect(result.costCents != nil)
        #expect(result.costCents == 100)
    }

    // MARK: - AgentResult Fields (§6.1)

    @Test("AgentResult includes sessionID and duration")
    func agentResultIncludesSessionIDAndDuration() async throws {
        let executor = SimulatedAgentExecutor()
        let agent = makeAgent()
        let result = try await executor.execute(
            task: makeTask(),
            agent: agent,
            context: makeContext()
        )
        #expect(result.sessionID != nil, "AgentResult.sessionID must be set (§6.1)")
        #expect(result.sessionID!.hasPrefix("sim-"), "Simulated sessions use 'sim-' prefix")
        #expect(result.durationSeconds >= 0, "AgentResult.durationSeconds must be non-negative (§6.1)")
    }
}
