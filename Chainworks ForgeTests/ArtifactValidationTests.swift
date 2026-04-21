import Testing
import Foundation
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
@Suite("ArtifactValidation")
struct ArtifactValidationTests {
    let container: ModelContainer
    let context: ModelContext
    let tempDir: URL

    init() throws {
        let (c, ctx) = try makeTestModelContainer()
        container = c
        context = ctx

        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ArtifactValidationTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
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
    @Test("Pipeline blocks when required artifact is missing")
    func pipelineBlocksWhenRequiredArtifactIsMissing() async throws {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "empty_agent", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status != .completed,
                "Run must not complete when required artifact agent fails")
        #expect(
            run.status == .blocked || run.status == .failed,
            "Run should be blocked or failed when agent cannot produce required artifact, got: \(run.status.rawValue)"
        )
    }

    /// When a transition requires an artifact that was never produced,
    /// the pipeline should not advance to the next state.
    @Test("Transition blocked when artifact never produced")
    func transitionBlockedWhenArtifactNeverProduced() async throws {
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
                    approvalRequired: false, approvalPolicy: nil, loop: nil
                ),
                "end": ExecutableState(
                    id: "end", label: "End", type: .end,
                    ownerAgentID: "a1", runBlock: nil, runAfterApproval: nil,
                    transitions: [], approvalRequired: false, approvalPolicy: nil, loop: nil
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
        #expect(run.status != .completed,
                "Run must not complete when required transition artifact is missing")
        // Agent should have executed
        #expect(!executor.executedTasks.isEmpty, "Agent should have executed")
    }

    // MARK: - #8: Empty artifact rejection

    /// ArtifactStorage should reject writes with empty data or detect zero-byte artifacts.
    @Test("Empty artifact detected with zero bytes")
    func emptyArtifactDetected() throws {
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
        #expect(result.sizeBytes == 0,
                "Empty artifact should report 0 bytes, enabling downstream validation to catch it")

        // Verify the file exists but is empty
        let data = try Data(contentsOf: URL(fileURLWithPath: result.filePath))
        #expect(data.isEmpty, "Written file should be empty")
    }

    /// Verify that ArtifactManager correctly records zero-byte artifacts,
    /// enabling validation gates to reject them.
    @Test("ArtifactManager records empty artifact size")
    func artifactManagerRecordsEmptyArtifactSize() throws {
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

        #expect(artifacts.count == 1, "Empty data should still create an artifact record")
        #expect(artifacts[0].sizeBytes == 0, "Empty artifact must report 0 bytes for validation")
        #expect(artifacts[0].checksumSHA256 != nil, "Even empty artifacts should have a checksum")
    }

    // MARK: - Transition evaluator with empty artifact set

    /// TransitionEvaluator should return false for artifactExists when no artifacts produced.
    @Test("TransitionEvaluator rejects empty artifact set")
    func transitionEvaluatorRejectsEmptyArtifactSet() {
        let ctx = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: [],
            approvalGranted: false,
            approvalRejected: false,
            variables: [:],
            artifactFields: [:]
        )

        #expect(
            !TransitionEvaluator.evaluate(.artifactExists("any_artifact"), context: ctx),
            "artifactExists must return false when no artifacts exist"
        )
    }

    /// TransitionEvaluator evaluateFirst should return nil when no transitions match (no artifacts).
    @Test("evaluateFirst returns nil when all require artifacts")
    func evaluateFirstReturnsNilWhenAllRequireArtifacts() {
        let transitions = [
            ExecutableTransition(to: "a", condition: .artifactExists("missing_1")),
            ExecutableTransition(to: "b", condition: .artifactExists("missing_2")),
        ]
        let ctx = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: [],
            approvalGranted: false,
            approvalRejected: false,
            variables: [:],
            artifactFields: [:]
        )

        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        #expect(result == nil, "No transition should match when all require missing artifacts")
    }
}
