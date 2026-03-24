import XCTest
import SwiftData
@testable import Chainworks_Forge

// MARK: - Shared Test Support
//
// Centralised test helpers extracted from duplicated code across 5+ test files.
// Provides consistent factory methods for RunWorkspace, ResolvedAgent, ExecutionContext,
// AgentTask, ModelContext, fixture loading, and custom assertion helpers.
//
// Usage: call these free functions from any XCTestCase or Swift Testing @Test.

// MARK: - ModelContext Factory

/// Creates an in-memory ModelContext suitable for unit testing.
/// Includes all Chainworks Forge model types.
@MainActor
func makeTestModelContext() throws -> ModelContext {
    let schema = Schema([
        Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self
    ])
    let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
    let container = try ModelContainer(for: schema, configurations: [config])
    return container.mainContext
}

/// Creates an in-memory ModelContainer + ModelContext pair suitable for XCTestCase setUp.
@MainActor
func makeTestModelContainer() throws -> (container: ModelContainer, context: ModelContext) {
    let schema = Schema([
        Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self
    ])
    let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
    let container = try ModelContainer(for: schema, configurations: [config])
    return (container, container.mainContext)
}

// MARK: - RunWorkspace Factory

/// Creates a RunWorkspace backed by a temporary directory.
/// The caller is responsible for cleanup via `cleanupWorkspace(_:)`.
func makeTestWorkspace(
    runID: UUID = UUID(),
    tempDir: URL? = nil
) -> RunWorkspace {
    let base = tempDir ?? FileManager.default.temporaryDirectory
        .appendingPathComponent("TestWorkspace-\(UUID().uuidString)", isDirectory: true)
    let workspaceRoot = base.appendingPathComponent(runID.uuidString, isDirectory: true)
    let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
    try? FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
    return RunWorkspace(runID: runID, workspaceRoot: workspaceRoot, artifactRoot: artifactRoot, worktreeRoot: nil)
}

/// Cleans up a workspace's temp directory.
func cleanupWorkspace(_ workspace: RunWorkspace) {
    if FileManager.default.fileExists(atPath: workspace.workspaceRoot.path) {
        try? FileManager.default.removeItem(at: workspace.workspaceRoot)
    }
}

// MARK: - ResolvedAgent Factory

/// Creates a ResolvedAgent with sensible defaults for testing.
/// Override only the parameters you care about.
func makeTestAgent(
    id: String = "test_agent",
    title: String? = nil,
    mode: String = "tool_use",
    backendProfileID: String? = nil,
    provider: String = "claude_code",
    model: String = "opus",
    effort: String = "high",
    maxTurns: Int = 10,
    temperature: Double = 0.0,
    permissionProfile: String = "ORCH",
    skillRef: String = "sk1",
    skillRole: String? = nil,
    prompt: String = "test prompt",
    outputContract: String? = nil,
    requiresHumanApproval: Bool = false,
    inputs: [String] = [],
    outputs: [String] = ["output_1"]
) -> ResolvedAgent {
    ResolvedAgent(
        id: id,
        title: title ?? "Agent \(id)",
        mode: mode,
        backendProfileID: backendProfileID,
        provider: provider,
        model: model,
        effort: effort,
        maxTurns: maxTurns,
        temperature: temperature,
        permissionProfile: permissionProfile,
        skillRef: skillRef,
        skillRole: skillRole,
        prompt: prompt,
        outputContract: outputContract,
        requiresHumanApproval: requiresHumanApproval,
        inputs: inputs,
        outputs: outputs
    )
}

// MARK: - ExecutionContext Factory

/// Creates an ExecutionContext with sensible defaults for testing.
func makeTestExecutionContext(
    runID: UUID = UUID(),
    stageID: String = "stage_1",
    iteration: Int = 1,
    attemptNumber: Int = 1,
    inputArtifacts: [String: Data] = [:],
    variables: [String: AnyCodableValue] = [:],
    ideaBody: String = "Test idea body",
    workspace: RunWorkspace? = nil
) -> ExecutionContext {
    let ws = workspace ?? makeTestWorkspace(runID: runID)
    return ExecutionContext(
        workspace: ws,
        stageID: stageID,
        iteration: iteration,
        attemptNumber: attemptNumber,
        inputArtifacts: inputArtifacts,
        variables: variables,
        ideaBody: ideaBody,
        providerBinding: nil
    )
}

// MARK: - AgentTask Factory

/// Creates an AgentTask with sensible defaults for testing.
func makeTestTask(
    agent: String = "test_agent",
    task: String = "do_work",
    inputs: [String]? = nil,
    outputs: [String]? = nil
) -> AgentTask {
    AgentTask(agent: agent, task: task, inputs: inputs, outputs: outputs)
}

// MARK: - Run Factory (requires ModelContext)

/// Creates a Run with an associated Idea in the given ModelContext.
/// Uses RunWorkspace for path configuration.
@MainActor
func makeTestRun(
    workspace: RunWorkspace,
    context: ModelContext,
    ideaTitle: String = "Test Idea",
    ideaBody: String = "Test body",
    workflowID: String = "test_wf",
    workflowTitle: String = "Test Workflow"
) -> Run {
    let idea = Idea(title: ideaTitle, body: ideaBody)
    context.insert(idea)

    let run = Run(
        id: workspace.runID,
        workflowID: workflowID,
        workflowTitle: workflowTitle,
        workflowSnapshotHash: "test_hash_wf",
        catalogSnapshotHash: "test_hash_cat",
        workflowSourcePath: "test/workflow.yaml",
        catalogSourcePath: "test/agents.yaml",
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

// MARK: - Fixture Loading

/// Marker class to locate the test bundle.
final class TestBundleMarker: NSObject {}

/// Loads the canonical workflow fixture from the test bundle.
func loadTestCanonicalWorkflow() throws -> WorkflowDefinition {
    let url = Bundle(for: TestBundleMarker.self).url(forResource: "workflow", withExtension: "yaml")!
    return try YAMLParser.loadWorkflow(from: url)
}

/// Loads the canonical agent catalog fixture from the test bundle.
func loadTestCanonicalCatalog() throws -> AgentCatalog {
    let url = Bundle(for: TestBundleMarker.self).url(forResource: "agents", withExtension: "yaml")!
    return try YAMLParser.loadAgentCatalog(from: url)
}

/// Loads the live proposal loop workflow fixture from the test bundle.
func loadTestLiveWorkflow() throws -> WorkflowDefinition {
    let url = try XCTUnwrap(
        Bundle(for: TestBundleMarker.self).url(forResource: "proposal-loop-live", withExtension: "yaml"),
        "proposal-loop-live.yaml fixture must be bundled with tests"
    )
    return try YAMLParser.loadWorkflow(from: url)
}

/// Loads the compact workflow fixture from the test bundle.
func loadTestCompactWorkflow() throws -> CompactWorkflowDefinition {
    let url = Bundle(for: TestBundleMarker.self).url(forResource: "proposal-to-release", withExtension: "yaml")!
    return try YAMLParser.loadCompactWorkflow(from: url)
}

// MARK: - Custom Assertion Helpers

/// Asserts that a Run has reached the `.completed` status.
/// Provides a clear diagnostic message on failure.
func assertRunCompleted(_ run: Run, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertEqual(
        run.status, .completed,
        "Expected run to be .completed, but got .\(run.status.rawValue). "
        + "Stage executions: \(run.stageExecutions.map { "\($0.stageID)=\($0.status.rawValue)" }.joined(separator: ", "))",
        file: file, line: line
    )
}

/// Asserts that a Run has reached the `.blocked` status.
func assertRunBlocked(_ run: Run, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertEqual(
        run.status, .blocked,
        "Expected run to be .blocked, but got .\(run.status.rawValue)",
        file: file, line: line
    )
}

/// Asserts that a Run is waiting for approval.
func assertRunWaitingApproval(_ run: Run, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertEqual(
        run.status, .waitingApproval,
        "Expected run to be .waitingApproval, but got .\(run.status.rawValue)",
        file: file, line: line
    )
}

/// Asserts that an artifact with the given name exists in the run's stage executions.
func assertArtifactExists(
    _ name: String,
    in run: Run,
    file: StaticString = #filePath,
    line: UInt = #line
) {
    let allArtifacts = run.stageExecutions
        .flatMap(\.agentExecutions)
        .flatMap(\.artifacts)
    let found = allArtifacts.contains { $0.name == name }
    XCTAssertTrue(
        found,
        "Expected artifact '\(name)' not found. Available: \(allArtifacts.map(\.name).joined(separator: ", "))",
        file: file, line: line
    )
}

/// Asserts that an artifact with the given name exists on disk and is non-empty.
func assertArtifactNonEmpty(
    _ name: String,
    in run: Run,
    workspace: RunWorkspace,
    file: StaticString = #filePath,
    line: UInt = #line
) {
    let allArtifacts = run.stageExecutions
        .flatMap(\.agentExecutions)
        .flatMap(\.artifacts)
    guard let artifact = allArtifacts.first(where: { $0.name == name }) else {
        XCTFail("Artifact '\(name)' not found in run", file: file, line: line)
        return
    }
    XCTAssertTrue(
        FileManager.default.fileExists(atPath: artifact.filePath),
        "Artifact '\(name)' file does not exist at: \(artifact.filePath)",
        file: file, line: line
    )
    XCTAssertGreaterThan(
        artifact.sizeBytes ?? 0, 0,
        "Artifact '\(name)' is empty (0 bytes)",
        file: file, line: line
    )
}

// MARK: - Async Polling Helper

/// Polls a condition with timeout, yielding between checks.
/// Replaces fragile `Task.sleep` polling loops throughout the test suite.
///
/// Usage:
/// ```
/// try await pollUntil(timeout: 2.0) { run.status == .completed }
/// ```
@MainActor
func pollUntil(
    timeout: TimeInterval = 3.0,
    interval: TimeInterval = 0.05,
    file: StaticString = #filePath,
    line: UInt = #line,
    message: String = "Condition not met within timeout",
    condition: @escaping () -> Bool
) async throws {
    let deadline = Date().addingTimeInterval(timeout)
    while !condition() {
        if Date() > deadline {
            XCTFail(message, file: file, line: line)
            return
        }
        try await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
    }
}
