import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("ArtifactManager", .tags(.fast))
struct ArtifactManagerTests {
    let container: ModelContainer
    let context: ModelContext
    let manager: ArtifactManager
    let tempDir: URL

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        manager = ArtifactManager(modelContext: context)

        // Create temp directory for artifact storage
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ArtifactManagerTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    // MARK: - Helpers

    private func makeWorkspace() -> RunWorkspace {
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        return RunWorkspace(
            runID: UUID(),
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )
    }

    private func makeAgent(
        id: String = "test_agent",
        outputContract: String? = nil
    ) -> ResolvedAgent {
        ResolvedAgent(
            id: id, title: "Test Agent", mode: "tool_use",
            provider: "claude_code", model: "opus", effort: "high",
            maxTurns: 10, temperature: 0.0, permissionProfile: "ORCH",
            skillRef: "sk1", skillRole: nil, prompt: "test",
            outputContract: outputContract, requiresHumanApproval: false,
            inputs: [], outputs: []
        )
    }

    private func makeAgentExecution(stageExec: StageExecution) -> AgentExecution {
        let agentExec = AgentExecution(
            agentID: "test_agent", agentTitle: "Test Agent",
            taskName: "test_task", provider: "claude_code", effort: "high"
        )
        agentExec.stageExecution = stageExec
        context.insert(agentExec)
        return agentExec
    }

    // MARK: - Persist Outputs

    @Test("persistOutputs writes to disk and SwiftData")
    func persistOutputsWritesToDiskAndSwiftData() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "stage_1", label: "Test Stage")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        let testData = Data("Hello, artifact!".utf8)
        let outputs = ["test_output": testData]

        let artifacts = try manager.persistOutputs(
            outputs: outputs,
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "stage_1",
            iteration: 1,
            attemptNumber: 1
        )

        // Verify SwiftData record created
        #expect(artifacts.count == 1)
        let artifact = artifacts[0]
        #expect(artifact.name == "test_output")
        #expect(artifact.stageID == "stage_1")
        #expect(artifact.agentID == "test_agent")
        #expect(artifact.runID == workspace.runID)
        #expect(artifact.checksumSHA256 != nil)
        #expect(artifact.sizeBytes == Int64(testData.count))

        // Verify file on disk
        #expect(FileManager.default.fileExists(atPath: artifact.filePath))

        // Verify data round-trips
        let readBack = try Data(contentsOf: URL(fileURLWithPath: artifact.filePath))
        #expect(readBack == testData)
    }

    @Test("persistOutputs handles multiple outputs")
    func persistMultipleOutputs() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "stage_1", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        let outputs = [
            "output_a": Data("A".utf8),
            "output_b": Data("B".utf8),
            "output_c": Data("C".utf8),
        ]

        let artifacts = try manager.persistOutputs(
            outputs: outputs,
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "stage_1",
            iteration: 1,
            attemptNumber: 1
        )

        #expect(artifacts.count == 3)
        let names = Set(artifacts.map(\.name))
        #expect(names == Set(["output_a", "output_b", "output_c"]))
    }

    // MARK: - Path Structure

    @Test("Artifact path structure follows convention")
    func pathStructure() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent(id: "my_agent")
        let stageExec = StageExecution(stageID: "s5", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        let artifacts = try manager.persistOutputs(
            outputs: ["result.json": Data("{}".utf8)],
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "s5",
            iteration: 2,
            attemptNumber: 3
        )

        let path = artifacts[0].filePath
        // Expected: {artifactRoot}/s5.2/my_agent/3/result.json
        #expect(path.contains("s5.2"), "Path should contain stageID.iteration")
        #expect(path.contains("my_agent"), "Path should contain agentID")
        #expect(path.contains("/3/"), "Path should contain attemptNumber")
        #expect(path.hasSuffix("result.json"))
    }

    // MARK: - Path Security

    @Test("Rejects path outside workspace boundary")
    func rejectsPathOutsideBoundary() {
        // Use a very deep traversal to escape the workspace root entirely
        let workspace = makeWorkspace()

        // Attempt to read from a path outside the workspace
        #expect {
            try ArtifactStorage.read(
                filePath: "/etc/passwd",
                workspaceRoot: workspace.workspaceRoot
            )
        } throws: { error in
            guard let storageError = error as? ArtifactStorageError else { return false }
            if case .pathOutsideBoundary = storageError { return true }
            return false
        }

        // Also test that write rejects absolute external paths via crafted stageID
        // The name ".." would try to traverse up, but after path normalization
        // the check catches it.
        #expect {
            try ArtifactStorage.write(
                data: Data("malicious".utf8),
                name: "test",
                stageID: "stage_1",
                iteration: 1,
                agentID: "agent",
                attemptNumber: 1,
                artifactRoot: URL(fileURLWithPath: "/tmp/other"),
                workspaceRoot: workspace.workspaceRoot
            )
        } throws: { error in
            guard let storageError = error as? ArtifactStorageError else { return false }
            if case .pathOutsideBoundary = storageError { return true }
            return false
        }
    }

    // MARK: - Read Artifact

    @Test("readArtifact returns persisted data")
    func readArtifact() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "s1", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        let originalData = Data("important data".utf8)
        let artifacts = try manager.persistOutputs(
            outputs: ["doc": originalData],
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "s1",
            iteration: 1,
            attemptNumber: 1
        )

        let readData = try manager.readArtifact(artifacts[0], workspace: workspace)
        #expect(readData == originalData)
    }

    // MARK: - Query Artifacts

    @Test("Query artifacts by run ID")
    func queryArtifactsByRunID() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "s1", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        try manager.persistOutputs(
            outputs: ["a": Data("1".utf8), "b": Data("2".utf8)],
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "s1",
            iteration: 1,
            attemptNumber: 1
        )

        let found = try manager.artifacts(forRunID: workspace.runID)
        #expect(found.count == 2)
    }

    @Test("Query artifacts by stage")
    func queryArtifactsByStage() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()

        let stageExec1 = StageExecution(stageID: "s1", label: "Stage 1")
        context.insert(stageExec1)
        let agentExec1 = makeAgentExecution(stageExec: stageExec1)

        let stageExec2 = StageExecution(stageID: "s2", label: "Stage 2")
        context.insert(stageExec2)
        let agentExec2 = AgentExecution(
            agentID: "test_agent", agentTitle: "Test",
            taskName: "task", provider: "claude_code", effort: "high"
        )
        agentExec2.stageExecution = stageExec2
        context.insert(agentExec2)

        try manager.persistOutputs(
            outputs: ["a": Data("1".utf8)],
            agent: agent, agentExecution: agentExec1,
            workspace: workspace, stageID: "s1", iteration: 1, attemptNumber: 1
        )
        try manager.persistOutputs(
            outputs: ["b": Data("2".utf8)],
            agent: agent, agentExecution: agentExec2,
            workspace: workspace, stageID: "s2", iteration: 1, attemptNumber: 1
        )

        let s1Artifacts = try manager.artifacts(forRunID: workspace.runID, stageID: "s1")
        #expect(s1Artifacts.count == 1)
        #expect(s1Artifacts[0].name == "a")

        let s2Artifacts = try manager.artifacts(forRunID: workspace.runID, stageID: "s2")
        #expect(s2Artifacts.count == 1)
        #expect(s2Artifacts[0].name == "b")
    }

    // MARK: - Produced Artifact Names

    @Test("Produced artifact names returns correct set")
    func producedArtifactNames() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "s1", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        try manager.persistOutputs(
            outputs: ["proposal_current": Data("md".utf8), "idea_brief": Data("md".utf8)],
            agent: agent, agentExecution: agentExec,
            workspace: workspace, stageID: "s1", iteration: 1, attemptNumber: 1
        )

        let names = try manager.producedArtifactNames(forRunID: workspace.runID)
        #expect(names == Set(["proposal_current", "idea_brief"]))
    }

    // MARK: - SHA256 Checksum

    @Test("Checksum consistency across writes of same data")
    func checksumConsistency() throws {
        let workspace = makeWorkspace()
        let agent = makeAgent()
        let stageExec = StageExecution(stageID: "s1", label: "Test")
        context.insert(stageExec)
        let agentExec = makeAgentExecution(stageExec: stageExec)

        let data = Data("consistent data".utf8)
        let artifacts = try manager.persistOutputs(
            outputs: ["test": data],
            agent: agent, agentExecution: agentExec,
            workspace: workspace, stageID: "s1", iteration: 1, attemptNumber: 1
        )

        #expect(artifacts[0].checksumSHA256 != nil)
        #expect(!artifacts[0].checksumSHA256!.isEmpty)

        // Write same data again with different location
        let workspace2 = RunWorkspace(
            runID: workspace.runID,
            workspaceRoot: tempDir,
            artifactRoot: tempDir.appendingPathComponent("artifacts2", isDirectory: true),
            worktreeRoot: nil
        )
        let stageExec2 = StageExecution(stageID: "s2", label: "Test2")
        context.insert(stageExec2)
        let agentExec2 = makeAgentExecution(stageExec: stageExec2)

        let artifacts2 = try manager.persistOutputs(
            outputs: ["test": data],
            agent: agent, agentExecution: agentExec2,
            workspace: workspace2, stageID: "s2", iteration: 1, attemptNumber: 1
        )

        // Same data should produce same checksum
        #expect(artifacts[0].checksumSHA256 == artifacts2[0].checksumSHA256)
    }
}
