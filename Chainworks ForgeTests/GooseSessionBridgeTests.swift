import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseSessionBridgeTests (Proposal 004, Section 12.1)

/// Unit tests for GooseSessionBridge.
/// Tests workspace validation, packet construction, and session isolation.
@MainActor
final class GooseSessionBridgeTests: XCTestCase {

    // MARK: - Helpers

    private func makeAgent(id: String = "test_agent") -> ResolvedAgent {
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
            inputs: ["input_artifact"],
            outputs: ["output_artifact"]
        )
    }

    private func makeTask() -> AgentTask {
        AgentTask(agent: "test_agent", task: "test_task", inputs: ["input_artifact"], outputs: ["output_artifact"])
    }

    private func makeWorkspace(runID: UUID = UUID()) -> RunWorkspace {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("test-bridge-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        return RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )
    }

    // MARK: - Workspace Validation Tests

    /// testSessionBridgeBindsWorkspaceExplicitly — Section 12.1
    func testSessionBridgeBindsWorkspaceExplicitly() throws {
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        // Should not throw for a valid workspace
        XCTAssertNoThrow(try GooseSessionBridge.validateWorkspace(workspace))
    }

    /// testSessionBridgeRejectsImplicitCWD — Section 12.1
    func testSessionBridgeRejectsImplicitCWD() {
        // Workspace with cwd as root should be rejected
        let cwdWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: FileManager.default.currentDirectoryPath),
            artifactRoot: URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("artifacts"),
            worktreeRoot: nil
        )

        XCTAssertThrowsError(try GooseSessionBridge.validateWorkspace(cwdWorkspace)) { error in
            XCTAssertTrue(error is GooseSessionBridgeError)
        }
    }

    /// testSessionBridgeRejectsRootPath
    func testSessionBridgeRejectsRootPath() {
        let rootWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: "/"),
            artifactRoot: URL(fileURLWithPath: "/artifacts"),
            worktreeRoot: nil
        )

        XCTAssertThrowsError(try GooseSessionBridge.validateWorkspace(rootWorkspace)) { error in
            XCTAssertTrue(error is GooseSessionBridgeError)
        }
    }

    /// testSessionBridgeRejectsEmptyPath
    func testSessionBridgeRejectsEmptyPath() {
        let emptyWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: ""),
            artifactRoot: URL(fileURLWithPath: ""),
            worktreeRoot: nil
        )

        XCTAssertThrowsError(try GooseSessionBridge.validateWorkspace(emptyWorkspace))
    }

    // MARK: - Execution Packet Tests

    /// testSessionBridgeUsesOneSessionPerExecution — Section 12.1
    func testPacketContainsSystemPrompt() {
        let agent = makeAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_1",
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea"
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // System prompt should contain agent info
        XCTAssertTrue(packet.systemPrompt.contains("Test Agent"))
        XCTAssertTrue(packet.systemPrompt.contains("test_agent"))
        XCTAssertTrue(packet.systemPrompt.contains("autonomous"))

        // System prompt should contain boundaries
        XCTAssertTrue(packet.systemPrompt.contains("Do not perform any git operations"))
        XCTAssertTrue(packet.systemPrompt.contains("Do not modify files outside the workspace root"))
    }

    func testPacketContainsTaskDirective() {
        let agent = makeAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: ["input_artifact": Data("test input data".utf8)],
            variables: [:],
            ideaBody: "Build a great feature"
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // Task directive should contain task name
        XCTAssertTrue(packet.taskDirective.contains("test_task"))

        // Should reference expected outputs
        XCTAssertTrue(packet.taskDirective.contains("output_artifact"))

        // Context attachments should include workspace context
        XCTAssertTrue(packet.contextAttachments.contains { $0.name == "workspace_context" })

        // Context attachments should include input artifacts
        XCTAssertTrue(packet.contextAttachments.contains { $0.name == "input_artifact" })

        // Context attachments should include idea body
        XCTAssertTrue(packet.contextAttachments.contains { $0.name == "idea_body" })
    }

    func testPacketWithoutInputArtifacts() {
        let agent = makeAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_1",
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: ""
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // Should still have workspace context
        XCTAssertTrue(packet.contextAttachments.contains { $0.name == "workspace_context" })

        // Should not have artifact or idea attachments
        XCTAssertFalse(packet.contextAttachments.contains { $0.type == "artifact" })
        XCTAssertFalse(packet.contextAttachments.contains { $0.name == "idea_body" })
    }

    // MARK: - LiveExecutionOverride Tests

    func testLiveExecutionOverrideEncoding() throws {
        let override = LiveExecutionOverride(
            enabled: true,
            provider: "claude_code",
            model: "claude-sonnet-4-20250514",
            effort: "high"
        )

        let data = try JSONEncoder().encode(override)
        let decoded = try JSONDecoder().decode(LiveExecutionOverride.self, from: data)

        XCTAssertEqual(decoded.enabled, true)
        XCTAssertEqual(decoded.provider, "claude_code")
        XCTAssertEqual(decoded.model, "claude-sonnet-4-20250514")
        XCTAssertEqual(decoded.effort, "high")
    }
}
