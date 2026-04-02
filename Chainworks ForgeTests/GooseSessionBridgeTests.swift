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

    private func makeWriteAgent(id: String = "write_agent") -> ResolvedAgent {
        ResolvedAgent(
            id: id,
            title: "Write Agent",
            mode: "autonomous",
            provider: "test_provider",
            model: "test_model",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "read_write",
            skillRef: "test_skill",
            skillRole: nil,
            prompt: "You are a write agent.",
            outputContract: "test_contract",
            requiresHumanApproval: false,
            inputs: ["input_artifact"],
            outputs: ["output_artifact"],
            worktreeWriteEnabled: true
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
            ownerExecutionLineageID: UUID(),
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
        #expect(packet.systemPrompt.contains("Do not rely on implicit working directory"))
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
            ownerExecutionLineageID: UUID(),
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
            projectRoot: URL(fileURLWithPath: "/tmp/project-root", isDirectory: true),
            stageID: "state_2",
            ownerExecutionLineageID: UUID(),
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
        let workspaceContext = packet.contextAttachments.first { $0.name == "workspace_context" }
        #expect(workspaceContext?.content?.contains("Project Root: /tmp/project-root") == true)
        #expect(workspaceContext?.content?.contains("Ignore any unexpected server cwd drift") == true)

        // Context attachments should include input artifacts
        #expect(packet.contextAttachments.contains { $0.name == "input_artifact" })

        // Context attachments should include idea body
        #expect(packet.contextAttachments.contains { $0.name == "idea_body" })
    }

    @Test("Packet includes strategy handoff artifacts and lazy references")
    func packetIncludesStrategyHandoffArtifactsAndLazyRefs() throws {
        let agent = makeAgent(id: "proposal_writer")
        let task = AgentTask(
            agent: "proposal_writer",
            task: "refine_proposal",
            inputs: [
                "idea_brief",
                "proposal_current",
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect",
                "proposal_review_summary",
                "score_lift_backlog",
                "security_audit_raw"
            ],
            outputs: ["proposal_current"]
        )
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let stewardProfile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]
        )
        let profileData = try JSONEncoder().encode(stewardProfile)
        let strategyProfile = try JSONDecoder().decode(ContextStrategyProfile.self, from: profileData)

        let inputArtifacts: [String: Data] = [
            "idea_brief": Data("short idea".utf8),
            "proposal_current": Data("full proposal body".utf8),
            "proposal_review_po": Data("{}".utf8),
            "proposal_review_ux": Data("{}".utf8),
            "proposal_review_ui": Data("{}".utf8),
            "proposal_review_architect": Data("{}".utf8),
            "proposal_review_summary": Data(String(repeating: "review ", count: 80).utf8),
            "score_lift_backlog": Data("{}".utf8),
            "security_audit_raw": Data("sensitive raw audit".utf8)
        ]
        let lazyArtifactPath = workspace.artifactRoot
            .appendingPathComponent("persisted-security-audit-raw.txt")
        try Data("sensitive raw audit".utf8).write(to: lazyArtifactPath)

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_2",
            ownerExecutionLineageID: UUID(),
            iteration: 1,
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            inputArtifactPaths: ["security_audit_raw": lazyArtifactPath.path],
            variables: [:],
            ideaBody: "Build a great refined proposal",
            providerBinding: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            contextStrategyProfile: strategyProfile,
            handoffPacket: HandoffCompiler().compile(
                profileID: "selective_compression_and_escalation",
                profile: strategyProfile,
                agent: agent,
                task: task,
                context: .init(
                    workspace: workspace,
                    stageID: "state_2",
                    ownerExecutionLineageID: UUID(),
                    iteration: 1,
                    attemptNumber: 1,
                    inputArtifacts: inputArtifacts,
                    inputArtifactPaths: ["security_audit_raw": lazyArtifactPath.path],
                    variables: [:],
                    ideaBody: "Build a great refined proposal",
                    providerBinding: nil
                )
            )
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        #expect(packet.taskDirective.contains("Profile: selective_compression_and_escalation"))
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "idea_brief" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_current" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_review_po" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_review_ux" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_review_ui" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_review_architect" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "proposal_review_summary" })
        #expect(packet.contextAttachments.contains { $0.type == "artifact" && $0.name == "score_lift_backlog" })
        let lazyTool = try #require(packet.contextAttachments.first { $0.name == "LazyEvidenceTool" })
        #expect(lazyTool.type == "file")
        let lazyToolPath = try #require(lazyTool.path)
        #expect(URL(fileURLWithPath: lazyToolPath).lastPathComponent == "get_lazy_artifact")
        let lazyToolManifest = try #require(packet.contextAttachments.first { $0.name == "lazy_evidence_manifest" })
        #expect(lazyToolManifest.type == "text")
        #expect(lazyToolManifest.content?.contains("\"toolName\" : \"get_lazy_artifact\"") == true)
        #expect(lazyToolManifest.content?.contains("\"owner\" : \"LazyEvidenceTool\"") == true)
        #expect(lazyToolManifest.content?.contains("\"security_audit_raw\"") == true)
        let lazyAttachment = try #require(packet.contextAttachments.first { $0.name == "lazy_security_audit_raw" })
        #expect(lazyAttachment.type == "file")
        #expect(lazyAttachment.path == lazyArtifactPath.path)
        #expect(packet.taskDirective.contains("Use the executable `get_lazy_artifact` helper attached as LazyEvidenceTool for canonical on-demand evidence retrieval."))
        #expect(packet.taskDirective.contains("Run: \(lazyToolPath) <artifact_name>"))
        #expect(packet.taskDirective.contains("Load lazy artifacts on demand from the attached file paths only when they become necessary."))
        #expect(packet.contextAttachments.contains { $0.name == "strategy_fingerprint" })

        let process = Process()
        let stdout = Pipe()
        process.executableURL = URL(fileURLWithPath: lazyToolPath)
        process.arguments = ["security_audit_raw"]
        process.standardOutput = stdout
        try process.run()
        process.waitUntilExit()

        let helperOutput = String(
            data: stdout.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        )
        #expect(process.terminationStatus == 0)
        #expect(helperOutput == "sensitive raw audit")
    }

    @Test("Proposal review packet requires exact JSON artifact names")
    func proposalReviewPacketRequiresExactJSONArtifactNames() {
        let agent = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Proposal Reviewer / Architect",
            mode: "review",
            provider: "codex",
            model: "gpt-5.4",
            effort: "high",
            maxTurns: 16,
            temperature: 0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: nil,
            prompt: "Review the proposal.",
            outputContract: "proposal_review_v1",
            requiresHumanApproval: false,
            inputs: ["proposal_current"],
            outputs: ["proposal_review_architect"]
        )
        let task = AgentTask(agent: agent.id, task: "review_proposal", inputs: ["proposal_current"], outputs: ["proposal_review_architect"])
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_4_proposal_reviewed",
            ownerExecutionLineageID: UUID(),
            iteration: 2,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        #expect(packet.taskDirective.contains("proposal_review_architect"))
        #expect(packet.taskDirective.contains("exact filenames"))
        #expect(packet.taskDirective.contains("Do not add file extensions"))
        #expect(packet.taskDirective.contains("top-level JSON object"))
        #expect(packet.taskDirective.contains("Do not write markdown"))
        #expect(packet.taskDirective.contains("Required Fields for proposal_review_v1:"))
        #expect(packet.taskDirective.contains("agent_id: String (use 'proposal_reviewer_architect')"))
        #expect(packet.taskDirective.contains("score: Number (0-10)"))
    }

    @Test("Aggregate review summary packet requires strict JSON artifact names even without agent outputContract")
    func aggregateReviewSummaryPacketRequiresStrictJSONArtifactNamesWithoutAgentOutputContract() {
        let agent = ResolvedAgent(
            id: "lead_orchestrator",
            title: "Lead / Orchestrator",
            mode: "orchestration",
            provider: "claude",
            model: "claude-opus-4.6",
            effort: "high",
            maxTurns: 16,
            temperature: 0,
            permissionProfile: "ORCH",
            skillRef: "orchestrator_core",
            skillRole: nil,
            prompt: "Aggregate only structured outputs.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect"
            ],
            outputs: ["proposal_review_summary", "run_state", "orchestrator_summary"]
        )
        let task = AgentTask(
            agent: agent.id,
            task: "aggregate_proposal_reviews",
            inputs: [
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect"
            ],
            outputs: ["proposal_review_summary"]
        )
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_4_proposal_reviewed",
            ownerExecutionLineageID: UUID(),
            iteration: 4,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        #expect(packet.taskDirective.contains("proposal_review_summary"))
        #expect(packet.taskDirective.contains("exact filenames"))
        #expect(packet.taskDirective.contains("Do not add file extensions"))
        #expect(packet.taskDirective.contains("top-level JSON object"))
        #expect(packet.taskDirective.contains("Do not write markdown"))
        #expect(packet.taskDirective.contains("Required Fields for proposal_review_summary_v1:"))
        #expect(packet.taskDirective.contains("pass: Boolean"))
        #expect(packet.taskDirective.contains("average_score: Number"))
        #expect(packet.taskDirective.contains("decision: String"))
    }

    @Test("Markdown output packet requires file existence verification before stop")
    func markdownOutputPacketRequiresFileExistenceVerificationBeforeStop() {
        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "authoring",
            provider: "claude",
            model: "claude-opus-4.6",
            effort: "high",
            maxTurns: 16,
            temperature: 0,
            permissionProfile: "RW_META",
            skillRef: "proposal_writer",
            skillRole: nil,
            prompt: "Refine the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: ["proposal_current"],
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = AgentTask(
            agent: agent.id,
            task: "refine_proposal",
            inputs: ["proposal_current"],
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_5_proposal_refined",
            ownerExecutionLineageID: UUID(),
            iteration: 6,
            attemptNumber: 1,
            inputArtifacts: [:],
            variables: [:],
            ideaBody: "Test idea",
            providerBinding: nil
        )

        let packet = GooseSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)

        #expect(packet.taskDirective.contains("proposal_current"))
        #expect(packet.taskDirective.contains("proposal_revision_summary"))
        #expect(packet.taskDirective.contains("verify that every required output file exists on disk"))
        #expect(packet.taskDirective.contains("If any required output file is missing or empty, continue working"))
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
            ownerExecutionLineageID: UUID(),
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
            ),
            sessionError: nil,
            events: []
        )

        let bridge = GooseSessionBridge(transport: transport)
        let agent = makeAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: workspace,
            stageID: "state_1",
            ownerExecutionLineageID: UUID(),
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

    @Test("Read-only repo-backed execution uses project root as working directory")
    func sessionBridgeUsesProjectRootForReadOnlyRepoBackedExecution() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "bridge-project-root",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-project-root",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: []
        )

        let bridge = GooseSessionBridge(transport: transport)
        let agent = makeAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let projectRoot = URL(fileURLWithPath: "/tmp/cryptosavingstracker", isDirectory: true)
        let context = ExecutionContext(
            workspace: workspace,
            projectRoot: projectRoot,
            stageID: "state_1",
            ownerExecutionLineageID: UUID(),
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
        #expect(lastRequest?.workingDirectory == projectRoot.path)
        #expect(lastRequest?.executionPolicy?.workspaceMode == "read_only")
    }

    @Test("Writable execution prefers worktree root over project root")
    func sessionBridgePrefersWorktreeRootForWritableExecution() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "bridge-worktree-root",
                status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true,
                    capabilityToken: "mock-worktree-root",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            sessionError: nil,
            events: []
        )

        let bridge = GooseSessionBridge(transport: transport)
        let agent = makeWriteAgent()
        let task = makeTask()
        let workspace = makeWorkspace()
        let worktreeRoot = workspace.workspaceRoot.appendingPathComponent("worktree", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)
        let writableWorkspace = RunWorkspace(
            runID: workspace.runID,
            workspaceRoot: workspace.workspaceRoot,
            artifactRoot: workspace.artifactRoot,
            worktreeRoot: worktreeRoot
        )
        defer { try? FileManager.default.removeItem(at: workspace.workspaceRoot) }

        let context = ExecutionContext(
            workspace: writableWorkspace,
            projectRoot: URL(fileURLWithPath: "/tmp/cryptosavingstracker", isDirectory: true),
            stageID: "state_7_implementation_started",
            ownerExecutionLineageID: UUID(),
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
        #expect(lastRequest?.workingDirectory == worktreeRoot.path)
        #expect(lastRequest?.executionPolicy?.workspaceMode == "read_write")
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
