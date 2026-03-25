import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseSessionBridgeTests (Proposal 004, Section 12.1)

/// Unit tests for GooseSessionBridge.
/// Tests workspace validation, packet construction, and session isolation.
@MainActor
@Suite("GooseSessionBridge")
struct GooseSessionBridgeTests {

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
    @Test("Session bridge binds workspace explicitly")
    func sessionBridgeBindsWorkspaceExplicitly() throws {
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        // Should not throw for a valid workspace
        #expect(throws: Never.self) { try GooseSessionBridge.validateWorkspace(workspace) }
    }

    /// testSessionBridgeRejectsImplicitCWD — Section 12.1
    @Test("Session bridge rejects implicit CWD")
    func sessionBridgeRejectsImplicitCWD() {
        // Workspace with cwd as root should be rejected
        let cwdWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: FileManager.default.currentDirectoryPath),
            artifactRoot: URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("artifacts"),
            worktreeRoot: nil
        )

        #expect(throws: GooseSessionBridgeError.self) {
            try GooseSessionBridge.validateWorkspace(cwdWorkspace)
        }
    }

    /// testSessionBridgeRejectsRootPath
    @Test("Session bridge rejects root path")
    func sessionBridgeRejectsRootPath() {
        let rootWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: "/"),
            artifactRoot: URL(fileURLWithPath: "/artifacts"),
            worktreeRoot: nil
        )

        #expect(throws: GooseSessionBridgeError.self) {
            try GooseSessionBridge.validateWorkspace(rootWorkspace)
        }
    }

    /// testSessionBridgeRejectsEmptyPath
    @Test("Session bridge rejects empty path")
    func sessionBridgeRejectsEmptyPath() {
        let emptyWorkspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: URL(fileURLWithPath: ""),
            artifactRoot: URL(fileURLWithPath: ""),
            worktreeRoot: nil
        )

        #expect(throws: (any Error).self) {
            try GooseSessionBridge.validateWorkspace(emptyWorkspace)
        }
    }

    // MARK: - Execution Packet Tests

    /// testSessionBridgeUsesOneSessionPerExecution — Section 12.1
    @Test("Packet contains system prompt")
    func packetContainsSystemPrompt() {
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
            ideaBody: "Test idea",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // System prompt should contain agent info
        #expect(packet.systemPrompt.contains("Test Agent"))
        #expect(packet.systemPrompt.contains("test_agent"))
        #expect(packet.systemPrompt.contains("autonomous"))

        // System prompt should contain boundaries
        #expect(packet.systemPrompt.contains("Do not perform any git operations"))
        #expect(packet.systemPrompt.contains("Do not modify files outside the workspace root"))
    }

    @Test("Packet can disable Xcode MCP via environment")
    func packetCanDisableXcodeMCPViaEnvironment() {
        setenv("CHAINWORKS_DISABLE_XCODE_MCP", "1", 1)
        defer { unsetenv("CHAINWORKS_DISABLE_XCODE_MCP") }

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
            ideaBody: "Test idea",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        #expect(packet.systemPrompt.contains("Do not call xcode_mcp"),
                "Test env should append explicit xcode_mcp suppression to the system prompt")
    }

    @Test("Packet contains task directive")
    func packetContainsTaskDirective() {
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
            ideaBody: "Build a great feature",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // Task directive should contain task name
        #expect(packet.taskDirective.contains("test_task"))

        // Should reference expected outputs
        #expect(packet.taskDirective.contains("output_artifact"))

        // Context attachments should include workspace context
        #expect(packet.contextAttachments.contains { $0.name == "workspace_context" })

        // Context attachments should include input artifacts
        #expect(packet.contextAttachments.contains { $0.name == "input_artifact" })

        // Context attachments should include idea body
        #expect(packet.contextAttachments.contains { $0.name == "idea_body" })
    }

    @Test("Packet without input artifacts")
    func packetWithoutInputArtifacts() {
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
            ideaBody: "",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        // Should still have workspace context
        #expect(packet.contextAttachments.contains { $0.name == "workspace_context" })

        // Should not have artifact or idea attachments
        #expect(!packet.contextAttachments.contains { $0.type == "artifact" })
        #expect(!packet.contextAttachments.contains { $0.name == "idea_body" })
    }

    @Test("Execution request carries read-only policy")
    func sessionBridgeExecutionRequestCarriesReadOnlyPolicy() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "bridge-session",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            )
        )

        let bridge = GooseSessionBridge(transport: transport)
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
            ideaBody: "Test idea",
            providerBinding: nil
        )

        _ = try await bridge.executeInIsolatedSession(
            agent: agent,
            task: task,
            context: context,
            override: nil
        )

        let lastRequest = await transport.lastSessionRequest
        #expect(lastRequest?.executionPolicy?.permissionProfileID == "read_only")
        #expect(lastRequest?.executionPolicy?.workspaceMode == "read_only")
        #expect(lastRequest?.executionPolicy?.gitOperationsAllowed == false)
        #expect(lastRequest?.executionPolicy?.releaseOperationsAllowed == false)
        #expect(lastRequest?.executionPolicy?.repoWritesAllowed == false)
    }

    // MARK: - LiveExecutionOverride Tests

    @Test("LiveExecutionOverride encoding round-trips")
    func liveExecutionOverrideEncoding() throws {
        let override = LiveExecutionOverride(
            enabled: true,
            provider: "claude_code",
            model: "claude-sonnet-4-20250514",
            effort: "high"
        )

        let data = try JSONEncoder().encode(override)
        let decoded = try JSONDecoder().decode(LiveExecutionOverride.self, from: data)

        #expect(decoded.enabled == true)
        #expect(decoded.provider == "claude_code")
        #expect(decoded.model == "claude-sonnet-4-20250514")
        #expect(decoded.effort == "high")
    }
}
