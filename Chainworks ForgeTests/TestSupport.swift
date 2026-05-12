import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

enum TestModelContainerRetainer {
    static var containers: [ModelContainer] = []

    static func retain(_ container: ModelContainer) {
        containers.append(container)
    }
}

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
    _ = installPortableMCPRegistryFixtureIfNeeded()
    let schema = Schema([
        Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        // Proposal 018
        AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self
    ])
    let config = ModelConfiguration("TestContext-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
    let container = try ModelContainer(for: schema, configurations: [config])
    TestModelContainerRetainer.retain(container)
    return container.mainContext
}

/// Creates an in-memory ModelContainer + ModelContext pair suitable for XCTestCase setUp.
@MainActor
func makeTestModelContainer() throws -> (container: ModelContainer, context: ModelContext) {
    _ = installPortableMCPRegistryFixtureIfNeeded()
    let schema = Schema([
        Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        // Proposal 018
        AgentSessionLineage.self, AgentSessionGeneration.self, AgentSessionEvent.self
    ])
    let config = ModelConfiguration("TestContainer-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
    let container = try ModelContainer(for: schema, configurations: [config])
    TestModelContainerRetainer.retain(container)
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
    outputs: [String] = ["output_1"],
    worktreeWriteEnabled: Bool = false
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
        outputs: outputs,
        worktreeWriteEnabled: worktreeWriteEnabled
    )
}

// MARK: - ExecutionContext Factory

/// Creates an ExecutionContext with sensible defaults for testing.
func makeTestExecutionContext(
    runID: UUID = UUID(),
    stageID: String = "stage_1",
    ownerExecutionLineageID: UUID = UUID(),
    stageLineageID: String? = nil,
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
        stageLineageID: stageLineageID,
        ownerExecutionLineageID: ownerExecutionLineageID,
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

@discardableResult
private func installPortableMCPRegistryFixtureIfNeeded(file: StaticString = #filePath) -> String? {
    let repoRoot = testRepositoryRootURL(file: file)
    let fixturePath = repoRoot.appendingPathComponent("examples/mcp/mcp-config-fixture.yaml").path
    guard FileManager.default.isReadableFile(atPath: fixturePath) else {
        return nil
    }

    setenv(CodexExtensionRegistryReader.environmentConfigPathKey, fixturePath, 1)
    return fixturePath
}

func testRepositoryRootURL(file: StaticString = #filePath) -> URL {
    URL(fileURLWithPath: "\(file)", isDirectory: false)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}

private func portableExternalSkillRewrites(
    localizedTriadPath: String,
    localizedAuditPath: String
) -> [(String, String)] {
    let legacyRoot = ["", "Users", "user", ".codex", "skills"].joined(separator: "/")

    return [
        ("\(legacyRoot)/proposal-review-triad", localizedTriadPath),
        ("\(legacyRoot)/proposal-implementation-audit", localizedAuditPath),
        ("../skills/proposal-review-triad", localizedTriadPath),
        ("../skills/proposal-implementation-audit", localizedAuditPath),
        ("../../examples/skills/proposal-review-triad", localizedTriadPath),
        ("../../examples/skills/proposal-implementation-audit", localizedAuditPath)
    ]
}

private func materializePortableSkillBundles(
    nextTo destinationURL: URL,
    file: StaticString = #filePath
) throws -> (triadPath: String, auditPath: String) {
    let repoRoot = testRepositoryRootURL(file: file)
    let sourceSkillsRoot = repoRoot.appendingPathComponent("examples/skills", isDirectory: true)
    let localizedSkillsRoot = destinationURL.deletingLastPathComponent().appendingPathComponent(
        "portable-skills-\(destinationURL.deletingPathExtension().lastPathComponent)",
        isDirectory: true
    )
    let triadDestination = localizedSkillsRoot.appendingPathComponent("proposal-review-triad", isDirectory: true)
    let auditDestination = localizedSkillsRoot.appendingPathComponent("proposal-implementation-audit", isDirectory: true)

    try FileManager.default.createDirectory(at: localizedSkillsRoot, withIntermediateDirectories: true)
    if FileManager.default.fileExists(atPath: triadDestination.path) {
        try FileManager.default.removeItem(at: triadDestination)
    }
    if FileManager.default.fileExists(atPath: auditDestination.path) {
        try FileManager.default.removeItem(at: auditDestination)
    }

    try FileManager.default.copyItem(
        at: sourceSkillsRoot.appendingPathComponent("proposal-review-triad", isDirectory: true),
        to: triadDestination
    )
    try FileManager.default.copyItem(
        at: sourceSkillsRoot.appendingPathComponent("proposal-implementation-audit", isDirectory: true),
        to: auditDestination
    )

    return (triadDestination.path, auditDestination.path)
}

@discardableResult
func writePortableCatalogCopy(from sourceURL: URL, to destinationURL: URL) throws -> URL {
    _ = installPortableMCPRegistryFixtureIfNeeded()
    let source = try String(contentsOf: sourceURL, encoding: .utf8)
    let localizedBundles = try materializePortableSkillBundles(nextTo: destinationURL)
    let rewritten = portableExternalSkillRewrites(
        localizedTriadPath: localizedBundles.triadPath,
        localizedAuditPath: localizedBundles.auditPath
    ).reduce(source) { partial, mapping in
        partial.replacingOccurrences(of: mapping.0, with: mapping.1)
    }
    try rewritten.write(to: destinationURL, atomically: true, encoding: .utf8)
    return destinationURL
}

@discardableResult
func writePortableWorkflowCopy(from sourceURL: URL, to destinationURL: URL) throws -> URL {
    let source = try String(contentsOf: sourceURL, encoding: .utf8)
    try FileManager.default.createDirectory(
        at: destinationURL.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    try source.write(to: destinationURL, atomically: true, encoding: .utf8)
    return destinationURL
}

/// Loads the canonical workflow fixture from the test bundle.
func loadTestCanonicalWorkflow() throws -> WorkflowDefinition {
    if let bundledURL = Bundle(for: TestBundleMarker.self).url(forResource: "workflow", withExtension: "yaml") {
        let portableURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("examples/workflows", isDirectory: true)
            .appendingPathComponent("chainworks-test-workflow-\(UUID().uuidString).yaml")
        try writePortableWorkflowCopy(from: bundledURL, to: portableURL)
        return try YAMLParser.loadWorkflow(from: portableURL)
    }

    let repoURL = testRepositoryRootURL().appendingPathComponent("examples/workflows/workflow.yaml")
    let url = try #require(
        FileManager.default.isReadableFile(atPath: repoURL.path) ? repoURL : nil,
        "workflow.yaml fixture must be bundled with tests or readable from the repository"
    )
    let portableURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("examples/workflows", isDirectory: true)
        .appendingPathComponent("chainworks-test-workflow-\(UUID().uuidString).yaml")
    try writePortableWorkflowCopy(from: url, to: portableURL)
    return try YAMLParser.loadWorkflow(from: portableURL)
}

/// Loads the canonical agent catalog fixture from the test bundle.
func loadTestCanonicalCatalog() throws -> AgentCatalog {
    _ = installPortableMCPRegistryFixtureIfNeeded()
    if let bundledURL = Bundle(for: TestBundleMarker.self).url(forResource: "agents", withExtension: "yaml") {
        let portableURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("examples/agents", isDirectory: true)
            .appendingPathComponent("chainworks-test-catalog-\(UUID().uuidString).yaml")
        try writePortableCatalogCopy(from: bundledURL, to: portableURL)
        return try YAMLParser.loadAgentCatalog(from: portableURL)
    }

    let repoURL = testRepositoryRootURL().appendingPathComponent("examples/agents/agents.yaml")
    let url = try #require(
        FileManager.default.isReadableFile(atPath: repoURL.path) ? repoURL : nil,
        "agents.yaml fixture must be bundled with tests or readable from the repository"
    )
    let portableURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("examples/agents", isDirectory: true)
        .appendingPathComponent("chainworks-test-catalog-\(UUID().uuidString).yaml")
    try writePortableCatalogCopy(from: url, to: portableURL)
    return try YAMLParser.loadAgentCatalog(from: portableURL)
}

/// Loads the steward config fixture from the test bundle.
func loadTestStewardConfig() throws -> StewardConfig {
    let url = try #require(
        Bundle(for: TestBundleMarker.self).url(forResource: "steward_config", withExtension: "yaml"),
        "steward_config.yaml fixture must be bundled with tests"
    )
    return try YAMLParser.loadStewardConfig(from: url)
}

/// Loads the live proposal loop workflow fixture from the test bundle.
func loadTestLiveWorkflow() throws -> WorkflowDefinition {
    _ = installPortableMCPRegistryFixtureIfNeeded()
    if let bundledURL = Bundle(for: TestBundleMarker.self).url(forResource: "proposal-loop-live", withExtension: "yaml") {
        let portableURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("chainworks-test-proposal-loop-live-\(UUID().uuidString).yaml")
        try writePortableWorkflowCopy(from: bundledURL, to: portableURL)
        return try YAMLParser.loadWorkflow(from: portableURL)
    }

    let repoURL = testRepositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
    let url = try #require(
        FileManager.default.isReadableFile(atPath: repoURL.path) ? repoURL : nil,
        "proposal-loop-live.yaml fixture must be bundled with tests or readable from the repository"
    )
    let portableURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("chainworks-test-proposal-loop-live-\(UUID().uuidString).yaml")
    try writePortableWorkflowCopy(from: url, to: portableURL)
    return try YAMLParser.loadWorkflow(from: portableURL)
}

/// Loads the full MVP live workflow fixture from the test bundle.
func loadTestFullMVPLiveWorkflow() throws -> WorkflowDefinition {
    _ = installPortableMCPRegistryFixtureIfNeeded()
    if let bundledURL = Bundle(for: TestBundleMarker.self).url(forResource: "full-mvp-live", withExtension: "yaml") {
        let portableURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("chainworks-test-full-mvp-live-\(UUID().uuidString).yaml")
        try writePortableWorkflowCopy(from: bundledURL, to: portableURL)
        return try YAMLParser.loadWorkflow(from: portableURL)
    }

    let repoURL = testRepositoryRootURL().appendingPathComponent("examples/workflows/full-mvp-live.yaml")
    let url = try #require(
        FileManager.default.isReadableFile(atPath: repoURL.path) ? repoURL : nil,
        "full-mvp-live.yaml fixture must be bundled with tests or readable from the repository"
    )
    let portableURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("chainworks-test-full-mvp-live-\(UUID().uuidString).yaml")
    try writePortableWorkflowCopy(from: url, to: portableURL)
    return try YAMLParser.loadWorkflow(from: portableURL)
}

/// Loads the compact workflow fixture from the test bundle.
func loadTestCompactWorkflow() throws -> CompactWorkflowDefinition {
    let url = try #require(
        Bundle(for: TestBundleMarker.self).url(forResource: "proposal-to-release", withExtension: "yaml"),
        "proposal-to-release.yaml fixture must be bundled with tests"
    )
    return try YAMLParser.loadCompactWorkflow(from: url)
}

// MARK: - Custom Assertion Helpers (removed — all callers migrated to Swift Testing variants below)

// MARK: - Swift Testing Assertion Helpers

/// Expects that a Run has reached the `.completed` status (Swift Testing variant).
func expectRunCompleted(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    let stages = run.stageExecutions.map { "\($0.stageID)=\($0.status.rawValue)" }.joined(separator: ", ")
    #expect(
        run.status == .completed,
        Comment(rawValue: "Expected .completed, got .\(run.status.rawValue). Stages: \(stages)"),
        sourceLocation: sourceLocation
    )
}

/// Expects that a Run has reached the `.blocked` status (Swift Testing variant).
func expectRunBlocked(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    #expect(
        run.status == .blocked,
        "Expected .blocked, got .\(run.status.rawValue)",
        sourceLocation: sourceLocation
    )
}

/// Expects that a Run is waiting for approval (Swift Testing variant).
func expectRunWaitingApproval(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    #expect(
        run.status == .waitingApproval,
        "Expected .waitingApproval, got .\(run.status.rawValue)",
        sourceLocation: sourceLocation
    )
}

/// Expects that an artifact with the given name exists in the run's stage executions (Swift Testing variant).
func expectArtifactExists(
    _ name: String,
    in run: Run,
    sourceLocation: SourceLocation = #_sourceLocation
) {
    let allArtifacts = run.stageExecutions
        .flatMap(\.agentExecutions)
        .flatMap(\.artifacts)
    #expect(
        allArtifacts.contains { $0.name == name },
        "Artifact '\(name)' not found. Available: \(allArtifacts.map(\.name).joined(separator: ", "))",
        sourceLocation: sourceLocation
    )
}

/// Expects that an artifact with the given name exists on disk and is non-empty (Swift Testing variant).
func expectArtifactNonEmpty(
    _ name: String,
    in run: Run,
    workspace: RunWorkspace,
    sourceLocation: SourceLocation = #_sourceLocation
) {
    let allArtifacts = run.stageExecutions
        .flatMap(\.agentExecutions)
        .flatMap(\.artifacts)
    guard let artifact = allArtifacts.first(where: { $0.name == name }) else {
        Issue.record("Artifact '\(name)' not found in run", sourceLocation: sourceLocation)
        return
    }
    #expect(
        FileManager.default.fileExists(atPath: artifact.filePath),
        "Artifact '\(name)' file missing: \(artifact.filePath)",
        sourceLocation: sourceLocation
    )
    #expect(
        (artifact.sizeBytes ?? 0) > 0,
        "Artifact '\(name)' is empty (0 bytes)",
        sourceLocation: sourceLocation
    )
}

// MARK: - Async Polling Helper (Swift Testing)

/// Confirmation-based async polling for Swift Testing.
/// Uses `confirmation()` so that a timeout is reported as an unconfirmed
/// expectation rather than a plain `Issue.record`, matching the proposal's
/// §6.2 contract.
@MainActor
func awaitCondition(
    _ description: String = "condition met",
    timeout: TimeInterval = 3.0,
    interval: TimeInterval = 0.05,
    condition: @escaping @MainActor () -> Bool
) async {
    await confirmation(Comment(rawValue: description)) { confirm in
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            if Date() > deadline { return }
            try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
        }
        confirm()
    }
}
