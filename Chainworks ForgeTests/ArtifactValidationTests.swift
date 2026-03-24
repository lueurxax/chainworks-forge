import XCTest
import SwiftData
@testable import Chainworks_Forge

// MARK: - ArtifactValidationTests
//
// Negative and edge-case tests for artifact validation.
// These tests verify behavior when artifacts are missing, empty, or malformed —
// the exact failure modes that the ArtifactTest proposal (Phase 1) aims to catch.
//
// Improvement #7: Negative test — pipeline blocks when required artifact is missing.
// Improvement #8: Empty artifact rejection test.

@MainActor
final class ArtifactValidationTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var tempDir: URL!

    override func setUp() async throws {
        let (c, ctx) = try makeTestModelContainer()
        container = c
        context = ctx

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ArtifactValidationTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let dir = tempDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    // MARK: - Helpers

    private func makeWorkspace() -> RunWorkspace {
        makeTestWorkspace(tempDir: tempDir)
    }

    private func makeRun(workspace: RunWorkspace) -> Run {
        makeTestRun(workspace: workspace, context: context)
    }

    // MARK: - #7: Pipeline blocks when required artifact is missing

    /// When an agent produces no outputs for a declared output contract,
    /// the orchestrator should detect the failure and block/fail the run.
    func testPipelineBlocksWhenRequiredArtifactIsMissing() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        // Agent declares "required_report" as output but produces nothing
        let failingAgent = makeTestAgent(
            id: "empty_agent",
            outputs: ["required_report"]
        )

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "empty_agent",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([makeTestTask(agent: "empty_agent", task: "produce_report")])
                    ]),
                    runAfterApproval: nil,
                    transitions: [
                        ExecutableTransition(to: "end", condition: .artifactExists("required_report"))
                    ],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "empty_agent", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["empty_agent": failingAgent],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(
                onError: "pause_and_require_human",
                onLoopBudgetExhausted: "pause_and_require_human",
                preserveArtifacts: true
            ),
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        // Use a failing executor so no artifacts are produced
        let executor = SimulatedAgentExecutor()
        executor.failingAgentIDs = ["empty_agent"]

        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        // The run should NOT be completed — it should be blocked because the agent failed
        XCTAssertNotEqual(run.status, .completed,
                          "Run must not complete when required artifact agent fails")
        XCTAssertTrue(
            run.status == .blocked || run.status == .failed,
            "Run should be blocked or failed when agent cannot produce required artifact, got: \(run.status.rawValue)"
        )
    }

    /// When a transition requires an artifact that was never produced,
    /// the pipeline should not advance to the next state.
    func testTransitionBlockedWhenArtifactNeverProduced() async throws {
        let workspace = makeWorkspace()
        let run = makeRun(workspace: workspace)

        let agent = makeTestAgent(id: "a1", outputs: ["some_other_output"])

        let plan = RunPlan(
            workflowID: "wf", workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start", label: "Start", type: .start,
                    ownerAgentID: "a1",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([makeTestTask(agent: "a1", task: "work")])
                    ]),
                    runAfterApproval: nil,
                    transitions: [
                        // This transition requires "missing_artifact" which a1 does NOT produce
                        ExecutableTransition(to: "end", condition: .artifactExists("missing_artifact"))
                    ],
                    approvalRequired: false, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["a1": agent],
            variables: [:],
            scoring: nil, failurePolicy: nil,
            workflowSnapshotHash: "h1", catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run, plan: plan, workspace: workspace,
            executor: executor, modelContext: context
        )

        await orchestrator.start()

        // The agent runs, but the transition condition (artifactExists("missing_artifact")) should fail
        // because the agent only produces "some_other_output", not "missing_artifact".
        XCTAssertNotEqual(run.status, .completed,
                          "Run must not complete when required transition artifact is missing")
        // Agent should have executed
        XCTAssertFalse(executor.executedTasks.isEmpty, "Agent should have executed")
    }

    // MARK: - #8: Empty artifact rejection

    /// ArtifactStorage should reject writes with empty data or detect zero-byte artifacts.
    func testEmptyArtifactDetected() throws {
        let workspace = makeWorkspace()
        let emptyData = Data()

        // Write an empty artifact — this should either throw or produce a zero-byte artifact
        // that can be detected by validation.
        let result = try ArtifactStorage.write(
            data: emptyData,
            name: "empty_output",
            stageID: "s1",
            iteration: 1,
            agentID: "agent_1",
            attemptNumber: 1,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        // Whether the system allows empty writes or not, we verify detectability:
        XCTAssertEqual(result.sizeBytes, 0,
                       "Empty artifact should report 0 bytes, enabling downstream validation to catch it")

        // Verify the file exists but is empty
        let data = try Data(contentsOf: URL(fileURLWithPath: result.filePath))
        XCTAssertTrue(data.isEmpty, "Written file should be empty")
    }

    /// Verify that ArtifactManager correctly records zero-byte artifacts,
    /// enabling validation gates to reject them.
    func testArtifactManagerRecordsEmptyArtifactSize() throws {
        let workspace = makeWorkspace()
        let agent = makeTestAgent(id: "empty_agent")

        let stageExec = StageExecution(stageID: "s1", label: "Test Stage")
        context.insert(stageExec)

        let agentExec = AgentExecution(
            agentID: "empty_agent", agentTitle: "Empty Agent",
            taskName: "produce_nothing", provider: "claude_code", effort: "high"
        )
        agentExec.stageExecution = stageExec
        context.insert(agentExec)

        let manager = ArtifactManager(modelContext: context)
        let artifacts = try manager.persistOutputs(
            outputs: ["empty_report": Data()],
            agent: agent,
            agentExecution: agentExec,
            workspace: workspace,
            stageID: "s1",
            iteration: 1,
            attemptNumber: 1
        )

        XCTAssertEqual(artifacts.count, 1, "Empty data should still create an artifact record")
        XCTAssertEqual(artifacts[0].sizeBytes, 0, "Empty artifact must report 0 bytes for validation")
        XCTAssertNotNil(artifacts[0].checksumSHA256, "Even empty artifacts should have a checksum")
    }

    // MARK: - Transition evaluator with empty artifact set

    /// TransitionEvaluator should return false for artifactExists when no artifacts produced.
    func testTransitionEvaluatorRejectsEmptyArtifactSet() {
        let ctx = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: [],
            approvalGranted: false,
            variables: [:],
            artifactFields: [:]
        )

        XCTAssertFalse(
            TransitionEvaluator.evaluate(.artifactExists("any_artifact"), context: ctx),
            "artifactExists must return false when no artifacts exist"
        )
    }

    /// TransitionEvaluator evaluateFirst should return nil when no transitions match (no artifacts).
    func testEvaluateFirstReturnsNilWhenAllRequireArtifacts() {
        let transitions = [
            ExecutableTransition(to: "a", condition: .artifactExists("missing_1")),
            ExecutableTransition(to: "b", condition: .artifactExists("missing_2")),
        ]
        let ctx = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: [],
            approvalGranted: false,
            variables: [:],
            artifactFields: [:]
        )

        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        XCTAssertNil(result, "No transition should match when all require missing artifacts")
    }
}
