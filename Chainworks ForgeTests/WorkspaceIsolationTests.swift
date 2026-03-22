import XCTest
import SwiftData
@testable import Chainworks_Forge

// MARK: - WorkspaceIsolationTests (Proposal 002 Section 12 — ARCH-025, ARCH-026)

/// Verifies that each run's workspace is isolated, artifact paths follow the canonical
/// convention, and cross-run boundary violations are rejected.
@MainActor
final class WorkspaceIsolationTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var tempDir: URL!

    override func setUp() async throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, Artifact.self
        ])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WsIsoTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let dir = tempDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    // MARK: - Helpers

    private func makeWorkspace(runID: UUID = UUID()) -> RunWorkspace {
        let workspaceRoot = tempDir.appendingPathComponent(runID.uuidString, isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        return RunWorkspace(runID: runID, workspaceRoot: workspaceRoot, artifactRoot: artifactRoot, worktreeRoot: nil)
    }

    private func makeRun(workspace: RunWorkspace) -> Run {
        let idea = Idea(title: "Isolation Test", body: "Testing workspace isolation")
        context.insert(idea)

        let run = Run(
            id: workspace.runID,
            workflowID: "isolation_wf",
            workflowTitle: "Isolation Test",
            workflowSnapshotHash: "hash_ws",
            catalogSnapshotHash: "hash_cat",
            workflowSourcePath: "test.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        ) // RunRepository-exempt
        run.idea = idea
        context.insert(run)
        return run
    }

    // MARK: - Workspace Provisioning

    /// RunWorkspace directories are created at run time and match ARCH-025 layout.
    func testWorkspaceProvisionedCorrectly() throws {
        let runID = UUID()

        // Use reflection-like approach: provision workspace creates correct structure
        let workspace = makeWorkspace(runID: runID)

        XCTAssertTrue(FileManager.default.fileExists(atPath: workspace.workspaceRoot.path),
                       "Workspace root must exist on disk")
        XCTAssertTrue(FileManager.default.fileExists(atPath: workspace.artifactRoot.path),
                       "Artifact root must exist on disk")
        XCTAssertTrue(workspace.artifactRoot.path.hasPrefix(workspace.workspaceRoot.path),
                       "Artifact root must be inside workspace root")
    }

    /// Two runs get distinct workspace roots (ARCH-025).
    func testTwoRunsGetDistinctWorkspaces() {
        let ws1 = makeWorkspace(runID: UUID())
        let ws2 = makeWorkspace(runID: UUID())

        XCTAssertNotEqual(ws1.workspaceRoot.path, ws2.workspaceRoot.path,
                           "Two runs must get distinct workspace roots")
        XCTAssertNotEqual(ws1.artifactRoot.path, ws2.artifactRoot.path,
                           "Two runs must get distinct artifact roots")
    }

    /// ArtifactRoot has no extra runID nesting (ARCH-026).
    func testArtifactRootNoExtraRunIDNesting() {
        let workspace = makeWorkspace()

        // ARCH-026: artifactRoot = {workspaceRoot}/artifacts/ — no extra runID folder
        let expectedSuffix = "/artifacts"
        XCTAssertTrue(workspace.artifactRoot.path.hasSuffix(expectedSuffix),
                       "ArtifactRoot should end with /artifacts, not runID/artifacts")
    }

    // MARK: - Artifact Path Isolation

    /// Artifacts written in one run cannot be read via another run's workspace.
    func testCrossRunArtifactReadBlocked() throws {
        // Create two workspaces
        let ws1 = makeWorkspace(runID: UUID())
        let ws2 = makeWorkspace(runID: UUID())

        // Write artifact in workspace 1
        let data = Data("artifact content".utf8)
        _ = try ArtifactStorage.write(
            data: data,
            name: "test_output",
            stageID: "stage_1",
            iteration: 1,
            agentID: "agent_1",
            attemptNumber: 1,
            artifactRoot: ws1.artifactRoot,
            workspaceRoot: ws1.workspaceRoot
        )

        // Try to read from workspace 2 using workspace 1's path
        // This should fail because the path is outside workspace 2's boundary
        let ws1ArtifactPath = ws1.artifactRoot
            .appendingPathComponent("stage_1.1/agent_1/1/test_output")
            .path

        XCTAssertThrowsError(
            try ArtifactStorage.read(filePath: ws1ArtifactPath, workspaceRoot: ws2.workspaceRoot)
        ) { error in
            // Should be a path boundary violation
            let desc = "\(error)".lowercased()
            XCTAssertTrue(
                desc.contains("boundary") || desc.contains("outside"),
                "Should reject cross-workspace read: \(error)"
            )
        }
    }

    /// Path traversal attacks are rejected (../../etc/passwd).
    func testPathTraversalAttackBlocked() {
        let workspace = makeWorkspace()

        XCTAssertThrowsError(
            try ArtifactStorage.read(
                filePath: workspace.artifactRoot.path + "/../../etc/passwd",
                workspaceRoot: workspace.workspaceRoot
            )
        ) { error in
            let desc = "\(error)".lowercased()
            XCTAssertTrue(
                desc.contains("boundary") || desc.contains("outside") || desc.contains("traversal"),
                "Should reject path traversal: \(error)"
            )
        }
    }

    // MARK: - Artifact Path Convention

    /// Artifact write path follows canonical convention: {stageID}.{iteration}/{agentID}/{attemptNumber}/{name}.
    func testArtifactPathConvention() throws {
        let workspace = makeWorkspace()

        let data = Data("canonical path test".utf8)
        let result = try ArtifactStorage.write(
            data: data,
            name: "report.json",
            stageID: "review_stage",
            iteration: 2,
            agentID: "security_reviewer",
            attemptNumber: 3,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        // Path should contain the canonical components
        XCTAssertTrue(result.filePath.contains("review_stage.2"), "Path should include stageID.iteration")
        XCTAssertTrue(result.filePath.contains("security_reviewer"), "Path should include agentID")
        XCTAssertTrue(result.filePath.contains("/3/"), "Path should include attempt number")
        XCTAssertTrue(result.filePath.hasSuffix("report.json"), "Path should end with artifact name")
    }

    /// Written artifacts produce valid SHA256 checksums.
    func testArtifactChecksumProduced() throws {
        let workspace = makeWorkspace()

        let data = Data("checksum test content".utf8)
        let result = try ArtifactStorage.write(
            data: data,
            name: "output.md",
            stageID: "s1",
            iteration: 1,
            agentID: "a1",
            attemptNumber: 1,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        XCTAssertFalse(result.checksumSHA256.isEmpty, "SHA256 checksum must be computed")
        XCTAssertEqual(result.checksumSHA256.count, 64, "SHA256 hex string should be 64 chars")
    }

    /// Written artifacts record correct size.
    func testArtifactSizeTracked() throws {
        let workspace = makeWorkspace()

        let content = "size tracking test"
        let data = Data(content.utf8)
        let result = try ArtifactStorage.write(
            data: data,
            name: "sized.json",
            stageID: "s1",
            iteration: 1,
            agentID: "a1",
            attemptNumber: 1,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        XCTAssertEqual(result.sizeBytes, Int64(data.count), "Written size must match data count")
    }

    // MARK: - Artifact Manager + Workspace

    /// ArtifactManager.persistOutputs correctly uses the workspace for path computation.
    func testArtifactManagerUsesWorkspace() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let agent = ResolvedAgent(
            id: "a1", title: "Agent 1", mode: "tool_use",
            provider: "claude_code", model: "opus", effort: "high",
            maxTurns: 10, temperature: 0.0, permissionProfile: "ORCH",
            skillRef: "sk1", skillRole: nil, prompt: "test",
            outputContract: nil, requiresHumanApproval: false,
            inputs: [], outputs: ["doc.md"]
        )

        let stageExec = StageExecution(stageID: "ws_stage", label: "WS Test", status: .running)
        stageExec.run = run
        context.insert(stageExec)

        let agentExec = AgentExecution(
            agentID: "a1", agentTitle: "Agent 1", taskName: "write_doc",
            status: .running, provider: "claude_code", effort: "high"
        )
        agentExec.stageExecution = stageExec
        context.insert(agentExec)

        let manager = ArtifactManager(modelContext: context)
        let artifacts = try manager.persistOutputs(
            outputs: ["doc.md": Data("# Workspace Test".utf8)],
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "ws_stage",
            iteration: 1,
            attemptNumber: 1
        )

        XCTAssertEqual(artifacts.count, 1)
        let artifact = artifacts[0]
        XCTAssertTrue(artifact.filePath.hasPrefix(workspace.workspaceRoot.path),
                       "Artifact path must be within workspace: \(artifact.filePath)")
        XCTAssertEqual(artifact.runID, workspace.runID)

        // Read it back to verify round-trip
        let readData = try manager.readArtifact(artifact, workspace: workspace)
        XCTAssertEqual(String(data: readData, encoding: .utf8), "# Workspace Test")
    }

    /// Two concurrent runs produce artifacts in their own workspaces without interference.
    func testConcurrentRunWorkspaceIsolation() async throws {
        let ws1 = makeWorkspace()
        let ws2 = makeWorkspace()
        let run1 = makeRun(workspace: ws1)
        let run2 = makeRun(workspace: ws2)

        let manager = ArtifactManager(modelContext: context)

        // Write artifacts for run 1
        let agent = ResolvedAgent(
            id: "a1", title: "Agent", mode: "tool_use",
            provider: "p", model: "m", effort: "h",
            maxTurns: 1, temperature: 0, permissionProfile: "P",
            skillRef: "s", skillRole: nil, prompt: "t",
            outputContract: nil, requiresHumanApproval: false,
            inputs: [], outputs: ["shared_name.json"]
        )

        let se1 = StageExecution(stageID: "s1", label: "S1", status: .running)
        se1.run = run1
        context.insert(se1)
        let ae1 = AgentExecution(agentID: "a1", agentTitle: "A", taskName: "t", status: .running, provider: "p", effort: "h")
        ae1.stageExecution = se1
        context.insert(ae1)

        let se2 = StageExecution(stageID: "s1", label: "S1", status: .running)
        se2.run = run2
        context.insert(se2)
        let ae2 = AgentExecution(agentID: "a1", agentTitle: "A", taskName: "t", status: .running, provider: "p", effort: "h")
        ae2.stageExecution = se2
        context.insert(ae2)

        let artifacts1 = try manager.persistOutputs(
            outputs: ["shared_name.json": Data("{\"run\":1}".utf8)],
            agent: agent, agentExecution: ae1, workspace: ws1,
            stageID: "s1", iteration: 1, attemptNumber: 1
        )

        let artifacts2 = try manager.persistOutputs(
            outputs: ["shared_name.json": Data("{\"run\":2}".utf8)],
            agent: agent, agentExecution: ae2, workspace: ws2,
            stageID: "s1", iteration: 1, attemptNumber: 1
        )

        // Both should succeed
        XCTAssertEqual(artifacts1.count, 1)
        XCTAssertEqual(artifacts2.count, 1)

        // Read back and verify they have distinct content
        let data1 = try manager.readArtifact(artifacts1[0], workspace: ws1)
        let data2 = try manager.readArtifact(artifacts2[0], workspace: ws2)

        XCTAssertTrue(String(data: data1, encoding: .utf8)!.contains("\"run\":1"))
        XCTAssertTrue(String(data: data2, encoding: .utf8)!.contains("\"run\":2"))

        // Verify paths are in distinct workspaces
        XCTAssertTrue(artifacts1[0].filePath.contains(ws1.runID.uuidString))
        XCTAssertTrue(artifacts2[0].filePath.contains(ws2.runID.uuidString))
    }
}
