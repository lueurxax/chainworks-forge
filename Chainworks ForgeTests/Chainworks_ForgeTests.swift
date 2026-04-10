import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

// MARK: - Helpers

private func makeContext() throws -> ModelContext {
    let config = ModelConfiguration("ForgeTests-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        configurations: config
    )
    TestModelContainerRetainer.retain(container)
    return ModelContext(container)
}

private let dummyWorkflow = WorkflowDefinition(
    schemaVersion: 1,
    workflow: WorkflowMeta(
        id: "wf-test", name: "Test Workflow", usesAgentCatalog: nil,
        description: "test", ideaInput: nil,
        execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
        requiredProviders: []
    ),
    variables: nil, failurePolicy: nil, scoring: nil,
    initialState: "s1",
    states: [
        "s1": WorkflowState(label: "Start", type: "start", owner: "agent1",
                             approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)
    ]
)

private let dummyCatalog = AgentCatalog(
    schemaVersion: 1,
    app: AppConfig(
        name: "test", runtime: "claude_agent", transport: "http",
        description: "test", ideaInputMode: "text",
        singleActiveRunPerIdea: true, runResumePolicy: "automatic_on_launch",
        requiredProviders: []
    ),
    paths: [:], artifacts: [:], skills: [:], contracts: [:],
    backendProfiles: [:], permissionProfiles: [:], agents: []
)

@MainActor
private func makeRun(in context: ModelContext, for idea: Idea) throws -> Run {
    let repo = RunRepository(context: context)
    return try repo.createRun(
        for: idea,
        workflow: dummyWorkflow,
        catalog: dummyCatalog,
        workflowSourcePath: "p",
        catalogSourcePath: "p"
    )
}

private nonisolated func fixtureURL(_ filename: String) -> URL {
    let bundle = Bundle(for: _TestBundleMarker.self)
    if let url = bundle.url(forResource: filename, withExtension: nil) {
        return url
    }
    let sourceRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("Fixtures", isDirectory: true)
    return sourceRoot.appendingPathComponent(filename, isDirectory: false)
}

private final class _TestBundleMarker: NSObject {}

// MARK: - Synthetic Builder Helpers

private func makeWorkflow(
    initialState: String = "s1",
    states: [String: WorkflowState] = ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)],
    requiredProviders: [String] = []
) -> WorkflowDefinition {
    WorkflowDefinition(
        schemaVersion: 1,
        workflow: WorkflowMeta(
            id: "wf", name: "WF", usesAgentCatalog: nil, description: "test",
            ideaInput: nil,
            execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
            requiredProviders: requiredProviders
        ),
        variables: nil, failurePolicy: nil, scoring: nil,
        initialState: initialState,
        states: states
    )
}

private func makeCatalog(
    paths: [String: String] = [:],
    artifacts: [String: String] = [:],
    skills: [String: SkillRef] = [:],
    mcpPolicy: MCPPolicyConfig = .defaultDeny,
    mcpServerRegistry: [String: MCPServerRegistryEntry] = [:],
    mcpProfiles: [String: MCPProfile] = [:],
    contracts: [String: ArtifactContract] = [:],
    backendProfiles: [String: BackendProfile] = [:],
    permissionProfiles: [String: PermissionProfile] = [:],
    agents: [AgentDefinition] = []
) -> AgentCatalog {
    AgentCatalog(
        schemaVersion: 1,
        app: AppConfig(
            name: "test", runtime: "claude_agent", transport: "http",
            description: "test", ideaInputMode: "text",
            singleActiveRunPerIdea: true, runResumePolicy: "automatic_on_launch",
            requiredProviders: []
        ),
        paths: paths, artifacts: artifacts, skills: skills,
        mcpPolicy: mcpPolicy, mcpServerRegistry: mcpServerRegistry, mcpProfiles: mcpProfiles, contracts: contracts,
        backendProfiles: backendProfiles, permissionProfiles: permissionProfiles, agents: agents
    )
}

private func makeAgent(
    id: String = "test_agent",
    backendProfile: String = "bp1",
    permissionProfile: String = "pp1",
    mcpProfile: String? = nil,
    skillRef: String = "sk1",
    inputs: [String] = [],
    outputs: [String] = [],
    outputContract: String? = nil
) -> AgentDefinition {
    AgentDefinition(
        id: id, title: "Test Agent", mode: "tool_use",
        backendProfile: backendProfile, permissionProfile: permissionProfile,
        mcpProfile: mcpProfile,
        skillRef: skillRef, skillRole: nil, worktreePolicy: nil,
        requiredTools: nil, inputs: inputs, outputs: outputs,
        outputContract: outputContract, requiresHumanApproval: false,
        prompt: "test prompt", notes: nil,
        sessionReuseScope: nil, sessionFamilyID: nil
    )
}

private let dummyPermissionProfile = PermissionProfile(
    filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
    git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
    shell: ShellPermissions(allow: nil, deny: nil),
    network: NetworkPermissions(allow: nil),
    mcp: MCPPermissions(allow: nil)
)

private let dummyBackendProfile = BackendProfile(
    provider: "claude_code", model: "opus", effort: "high",
    temperature: 0.0, maxTurns: 10, structuredOutput: "json"
)

private let dummySkillRef = SkillRef(type: "builtin", path: nil, name: nil, description: nil)

private func makeCompact(stages: [CompactStage]) -> CompactWorkflowDefinition {
    CompactWorkflowDefinition(
        version: 1,
        workflow: CompactWorkflowMeta(
            id: "cw", title: "Compact",
            execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
            requiredProviders: [],
            stages: stages
        ),
        agentAliases: nil
    )
}

// MARK: - Model Tests

@Suite("Idea Model")
struct IdeaTests {
    @Test func creation() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test Idea", body: "Test body")
        context.insert(idea)
        try context.save()

        #expect(idea.title == "Test Idea")
        #expect(idea.status == .draft)
        #expect(idea.runs.isEmpty)
    }
}

@Suite("Run Model", .serialized, .tags(.fast))
@MainActor
struct RunTests {
    @Test func creationWithProvenance() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        let run = try makeRun(in: context, for: idea)

        #expect(run.workflowID == "wf-test")
        #expect(!run.workflowSnapshotHash.isEmpty)
        #expect(!run.catalogSnapshotHash.isEmpty)
        #expect(run.status == .pending)
        #expect(run.idea === idea)
    }

    @Test func provenanceIsImmutable() throws {
        // Provenance fields are private(set) — enforced at compile time.
        // This test documents the contract and verifies values are set at creation.
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        let run = try makeRun(in: context, for: idea)

        #expect(run.workflowID == "wf-test")
        #expect(run.workflowTitle == "Test Workflow")
        #expect(!run.workflowSnapshotHash.isEmpty)
        #expect(!run.catalogSnapshotHash.isEmpty)
        #expect(run.workflowSourcePath == "p")
        #expect(run.catalogSourcePath == "p")
        #expect(!run.workflowSnapshotJSON.isEmpty)
        #expect(!run.catalogSnapshotJSON.isEmpty)
        // private(set) prevents modification from outside Run.swift — compiler enforces this
    }

    @Test func sequentialCreationBlocked() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        _ = try makeRun(in: context, for: idea)

        #expect(throws: RunRepositoryError.self) {
            _ = try makeRun(in: context, for: idea)
        }
    }

    // REQ-005: Parallel-start test using concurrent tasks for true concurrency proof
    @Test func parallelRunCreationSerializes() async throws {
        // R4-002: @MainActor serialization ensures exactly one succeeds.
        // Two Task instances are created concurrently; both must hop to @MainActor
        // to call createRun, proving serialization prevents double-creation.
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        let repo = RunRepository(context: context)

        let task1 = Task { @MainActor in
            try repo.createRun(
                for: idea, workflow: dummyWorkflow, catalog: dummyCatalog,
                workflowSourcePath: "p", catalogSourcePath: "p"
            )
        }

        let task2 = Task { @MainActor in
            try repo.createRun(
                for: idea, workflow: dummyWorkflow, catalog: dummyCatalog,
                workflowSourcePath: "p", catalogSourcePath: "p"
            )
        }

        var successCount = 0
        var failureCount = 0

        do { _ = try await task1.value; successCount += 1 } catch { failureCount += 1 }
        do { _ = try await task2.value; successCount += 1 } catch { failureCount += 1 }

        #expect(successCount == 1, "Exactly one run should succeed")
        #expect(failureCount == 1, "Exactly one run should fail")
    }

    @Test func allowedAfterCompletion() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        let run1 = try makeRun(in: context, for: idea)
        run1.status = .completed

        let run2 = try makeRun(in: context, for: idea)
        #expect(run2.status == .pending)
    }

    @Test func currentStageIDDerived() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        #expect(run.currentStageID == nil)

        let stage = StageExecution(stageID: "state_1", label: "Idea received", status: .running)
        stage.run = run
        context.insert(stage)

        #expect(run.currentStageID == "state_1")
    }

    @Test func currentStageIDNilWhenEmpty() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        #expect(run.currentStageID == nil)
    }

    @Test func currentStageIDReflectsRetry() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        // First attempt completes
        let stage1 = StageExecution(stageID: "s1", label: "S1", status: .completed)
        stage1.run = run
        context.insert(stage1)

        // Second stage running
        let stage2 = StageExecution(stageID: "s2", label: "S2", status: .running)
        stage2.run = run
        context.insert(stage2)

        #expect(run.currentStageID == "s2")
    }

    @Test func currentStageIDPrefersMostRecentBlockedStageOverOlderRunningStage() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        let staleRunningStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .running
        )
        staleRunningStage.run = run
        context.insert(staleRunningStage)

        let latestBlockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        latestBlockedStage.run = run
        context.insert(latestBlockedStage)

        #expect(run.currentStageID == "state_4_proposal_reviewed")
    }

    @Test func resumeContinuationStateIDMatchesLatestInterruptibleStageWithoutSnapshotLoader() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        let completedStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .completed
        )
        completedStage.run = run
        context.insert(completedStage)

        let blockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        blockedStage.run = run
        context.insert(blockedStage)

        #expect(run.currentStageID == "state_4_proposal_reviewed")
        #expect(run.resumeContinuationStateID == "state_4_proposal_reviewed")
    }

    @Test func resumeContinuationStateIDPrefersMaterializedReadyStageOverOlderRunStateFallback() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        let tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("chainworks-run-state-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let completedStage = StageExecution(
            stageID: "state_9_implementation_reviewed",
            label: "Implementation reviewed",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        completedStage.run = run
        context.insert(completedStage)

        let orchestrator = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead Orchestrator",
            taskName: "aggregate_review",
            startedAt: Date(timeIntervalSince1970: 11),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        orchestrator.stageExecution = completedStage
        context.insert(orchestrator)

        let readyNextStage = StageExecution(
            stageID: "state_10_implementation_refined",
            label: "Implementation refined",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .ready,
            iteration: 1,
            attemptNumber: 2
        )
        readyNextStage.run = run
        context.insert(readyNextStage)

        let runStateURL = tempDirectory.appendingPathComponent("run_state.json")
        try Data("""
        {
          "current_stage": "state_9_implementation_reviewed",
          "next_stage": "state_7_implementation_started",
          "next_stage_reason": "loop back to implementation"
        }
        """.utf8).write(to: runStateURL, options: .atomic)

        let runStateArtifact = Artifact(
            name: "run_state",
            contractID: "run_state",
            format: .json,
            filePath: runStateURL.path,
            runID: run.id,
            stageID: completedStage.stageID,
            agentID: "lead_orchestrator",
            provider: "claude_code"
        )
        runStateArtifact.agentExecution = orchestrator
        context.insert(runStateArtifact)

        let pollutedBlockedStage = StageExecution(
            stageID: "state_7_implementation_started",
            label: "Implementation started",
            startedAt: Date(timeIntervalSince1970: 30),
            status: .blocked,
            iteration: 3,
            attemptNumber: 1
        )
        pollutedBlockedStage.run = run
        context.insert(pollutedBlockedStage)

        let failedAgent = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "refine_implementation",
            startedAt: Date(timeIntervalSince1970: 31),
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        failedAgent.stageExecution = pollutedBlockedStage
        context.insert(failedAgent)

        try context.save()

        #expect(run.currentStageID == "state_7_implementation_started")
        #expect(run.resumeContinuationStateID == "state_10_implementation_refined")
    }

    @Test func listPresentationStatusLabelUsesStoredListTruth() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        run.status = .running

        let blockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        blockedStage.run = run
        context.insert(blockedStage)

        #expect(run.presentationStatusLabel == "blocked")
        #expect(run.listPresentationStatusLabel == "running")
    }

    @Test func presentationStatusPrefersLatestBlockedStageWhenRunStatusIsStaleRunning() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        run.status = .running

        let staleRunningStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .running
        )
        staleRunningStage.run = run
        context.insert(staleRunningStage)

        let latestBlockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        latestBlockedStage.run = run
        context.insert(latestBlockedStage)

        #expect(run.presentationStatus == .blocked)
        #expect(run.presentationStatusLabel == "blocked")
    }

    @Test func listPresentationStatusStaysOnStoredRunStatusWhenStageTruthIsHeavierThanSidebarNeeds() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        run.status = .running

        let staleRunningStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .running
        )
        staleRunningStage.run = run
        context.insert(staleRunningStage)

        let latestBlockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        latestBlockedStage.run = run
        context.insert(latestBlockedStage)

        #expect(run.presentationStatus == .blocked)
        #expect(run.listPresentationStatus == .running)
    }

    @Test func sidebarLifecycleStatusLabelUsesLightweightRunTruth() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        run.status = .running

        let blockedStage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .blocked
        )
        blockedStage.run = run
        context.insert(blockedStage)

        #expect(run.presentationStatus == .blocked)
        #expect(idea.lifecycleStatusLabel == "Blocked")
        #expect(idea.sidebarLifecycleStatusLabel == "Running")
    }

    @Test func operatorStopAvailabilityMatchesNonTerminalRunTruth() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        run.status = .running
        #expect(run.canBeCancelledByOperator)

        run.status = .blocked
        #expect(run.canBeCancelledByOperator)

        run.status = .waitingApproval
        #expect(run.canBeCancelledByOperator)

        run.status = .completed
        #expect(!run.canBeCancelledByOperator)

        run.status = .failed
        #expect(!run.canBeCancelledByOperator)

        run.status = .cancelled
        #expect(!run.canBeCancelledByOperator)
    }

    @Test func operatorStopAvailabilityStaysTrueWhileCancellationIsSettling() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        run.status = .running
        run.cancellationRequestedAt = Date()
        run.cancellationSettledAt = nil

        #expect(run.presentationStatus == .cancelling)
        #expect(run.canBeCancelledByOperator)
    }

    @Test func blockedStateForDrift() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        run.status = .blocked
        run.driftDetectedAt = Date()
        run.driftDetails = "workflow.yaml changed"

        #expect(run.status == .blocked)
        #expect(run.driftDetectedAt != nil)
    }

    @Test func driftDecisionPersists() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        run.driftDecision = .continueWithOriginal
        try context.save()

        #expect(run.driftDecision == .continueWithOriginal)
    }

    // REQ-010: Fixed snapshotDeserializable to round-trip Run.workflowSnapshotJSON → WorkflowDefinition
    @Test func snapshotDeserializable() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)

        let run = try makeRun(in: context, for: idea)

        // Round-trip workflowSnapshotJSON back into WorkflowDefinition
        let decoded = try JSONDecoder().decode(WorkflowDefinition.self, from: run.workflowSnapshotJSON)
        #expect(decoded.workflow.id == "wf-test")
        #expect(decoded.initialState == "s1")
        #expect(decoded.states.count == 1)
        #expect(decoded.states["s1"]?.label == "Start")
    }

    @Test func costCentsAggregation() {
        let c1: Int64 = 73
        let c2: Int64 = 142
        let c3: Int64 = 9999
        let total = c1 + c2 + c3
        #expect(total == 10214) // $102.14 exactly, no floating-point drift
    }

    @Test func cascadeDelete() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)
        run.status = .completed

        let stage = StageExecution(stageID: "s1", label: "S1")
        stage.run = run
        context.insert(stage)
        try context.save()

        context.delete(idea)
        try context.save()

        let runs = try context.fetch(FetchDescriptor<Run>())
        #expect(runs.isEmpty)
    }

    // MARK: - ARCH-PA-003: No direct Run construction outside RunRepository

    @Test func noDirectRunConstruction() throws {
        // ARCH-PA-006: recursive scan of ALL .swift files in app source tree.
        // Mirrors the sanitization logic in scripts/test-gate.sh guard_direct_run_insertion():
        // 1. Strip block comments (/* ... */)
        // 2. Strip line comments (// ...)
        // 3. Replace string literals with empty placeholders
        // This prevents false positives from natural-language text like "Clone Run (Frozen Snapshot)".
        let testFilePath = URL(fileURLWithPath: #filePath)
        let sourceDir = testFilePath
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Chainworks Forge")
        let directRunPattern = try Regex(#"\bRun\s*\("#)
        let blockCommentPattern = try Regex(#"(?s)/\*.*?\*/"#)
        let stringLiteralPattern = try Regex(#""(?:\\.|[^"\\])*""#)

        let enumerator = FileManager.default.enumerator(
            at: sourceDir,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        )!

        let exempted = Set(["RunRepository.swift", "Run.swift"])
        var violations: [String] = []

        for case let file as URL in enumerator where file.pathExtension == "swift" {
            guard !exempted.contains(file.lastPathComponent) else { continue }
            let raw = try String(contentsOf: file, encoding: .utf8)

            // Strip block comments, then line comments, then string literals
            var sanitized = raw.replacing(blockCommentPattern, with: "")
            sanitized = sanitized
                .split(separator: "\n", omittingEmptySubsequences: false)
                .map { line in
                    let trimmed = line.drop(while: { $0 == " " || $0 == "\t" })
                    return trimmed.hasPrefix("//") ? "" : String(line)
                }
                .joined(separator: "\n")
            sanitized = sanitized.replacing(stringLiteralPattern, with: #""""#)

            if sanitized.contains(directRunPattern)
                && !sanitized.contains("RunStatus")
                && !sanitized.contains("RunRepositoryError")
                && !raw.contains("// RunRepository-exempt") {
                violations.append(file.lastPathComponent)
            }
        }
        #expect(violations.isEmpty, "Direct Run construction outside RunRepository: \(violations)")
    }

    // MARK: - REQ-010: Stage execution relationships

    @Test func testStageExecutionRelationships() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        // Create a stage and attach agent executions
        let stage = StageExecution(stageID: "s1", label: "Build stage", status: .running)
        stage.run = run
        context.insert(stage)

        let agent1 = AgentExecution(agentID: "code_writer", agentTitle: "Code Writer",
                                     taskName: "implement", provider: "claude_code", effort: "high")
        agent1.stageExecution = stage
        context.insert(agent1)

        let agent2 = AgentExecution(agentID: "security_checker", agentTitle: "Security Checker",
                                     taskName: "scan", provider: "codex", effort: "medium")
        agent2.stageExecution = stage
        context.insert(agent2)

        try context.save()

        #expect(stage.agentExecutions.count == 2)
        #expect(Set(stage.agentExecutions.map(\.agentID)) == Set(["code_writer", "security_checker"]))
        #expect(agent1.stageExecution?.id == stage.id)
        #expect(agent2.stageExecution?.id == stage.id)
        #expect(stage.run?.id == run.id)
    }

    // MARK: - REQ-010: Approval decision flow

    @Test func testApprovalDecisionFlow() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        let approval = Approval(stageID: "gate_1")
        approval.run = run
        context.insert(approval)

        // Initial state: pending
        #expect(approval.decision == .pending)
        #expect(approval.decidedAt == nil)

        // Transition to requested
        approval.decision = .requested
        #expect(approval.decision == .requested)

        // Transition to granted
        approval.decision = .granted
        approval.decidedAt = Date()
        approval.comment = "Looks good, proceed"
        try context.save()

        #expect(approval.decision == .granted)
        #expect(approval.decidedAt != nil)
        #expect(approval.comment == "Looks good, proceed")
        #expect(approval.run?.id == run.id)
        #expect(run.approvals.count == 1)
    }

    // MARK: - REQ-010: Artifact attachment to AgentExecution

    @Test func testArtifactAttachmentToAgentExecution() throws {
        let context = try makeContext()
        let idea = Idea(title: "Test", body: "Body")
        context.insert(idea)
        let run = try makeRun(in: context, for: idea)

        let stage = StageExecution(stageID: "s1", label: "Build", status: .running)
        stage.run = run
        context.insert(stage)

        let agentExec = AgentExecution(agentID: "code_writer", agentTitle: "Code Writer",
                                        taskName: "implement", provider: "claude_code", effort: "high")
        agentExec.stageExecution = stage
        context.insert(agentExec)

        let artifact = Artifact(
            name: "proposal.md",
            contractID: "proposal_current",
            format: .markdown,
            filePath: "/output/proposal.md",
            runID: run.id,
            stageID: "s1",
            agentID: "code_writer",
            provider: "claude_code"
        )
        artifact.agentExecution = agentExec
        context.insert(artifact)
        try context.save()

        #expect(agentExec.artifacts.count == 1)
        #expect(agentExec.artifacts.first?.name == "proposal.md")
        #expect(agentExec.artifacts.first?.format == .markdown)
        #expect(artifact.agentExecution?.id == agentExec.id)
        #expect(artifact.contractID == "proposal_current")
    }
}

// MARK: - Parser Tests

@Suite("YAML Parser", .serialized)
struct YAMLParserTests {
    @Test func parseAgentCatalog() throws {
        let url = fixtureURL("agents.yaml")
        #expect(FileManager.default.fileExists(atPath: url.path), "File must exist at \(url.path)")
        do {
            let catalog = try YAMLParser.loadAgentCatalog(from: url)
            #expect(catalog.agents.count == 16)  // 13 original + 3 Steward agents (Proposal 003)
            #expect(catalog.backendProfiles.count == 14)  // 11 original + 3 Steward profiles
            #expect(catalog.permissionProfiles.count == 8)
            #expect(catalog.contracts.count == 28)  // Canonical catalog now includes proposal-loop, strategy, and exact output contracts
        } catch {
            Issue.record("Parse failed: \(error)")
        }
    }

    @Test func parseAgentCatalogWithMCPProfilesV2() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let url = repoRoot.appendingPathComponent("examples/agents/agents_mcp_profiles_v2.yaml")
        let catalog = try YAMLParser.loadAgentCatalog(from: url)
        #expect(catalog.mcpPolicy.defaultProfile == "none")
        #expect(catalog.mcpServerRegistry["xcode"]?.runtimeIDs["claude_agent"] == "xcode")
        #expect(catalog.mcpProfiles["code_build_rich"] != nil)
        let codeWriter = try #require(catalog.agents.first(where: { $0.id == "code_writer" }))
        #expect(codeWriter.mcpProfile == "code_build_rich")
    }

    @Test func reviewVisualProfileDoesNotOptIntoAutovisualiser() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let catalogURLs = [
            repoRoot.appendingPathComponent("examples/agents/agents.yaml"),
            repoRoot.appendingPathComponent("examples/agents/agents_mcp_profiles_v2.yaml")
        ]

        for url in catalogURLs {
            let catalog = try YAMLParser.loadAgentCatalog(from: url)
            let reviewVisual = try #require(catalog.mcpProfiles["review_visual"])
            #expect(reviewVisual.requiredExtensions == ["xcode"])
            #expect(reviewVisual.optionalExtensions.isEmpty)
        }
    }

    @Test func exampleCatalogRoutesClaudeAndGeminiThroughACPAndUsesGeminiReviewPro() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let catalogURLs = [
            repoRoot.appendingPathComponent("examples/agents/agents.yaml"),
            repoRoot.appendingPathComponent("examples/agents/agents_mcp_profiles_v2.yaml")
        ]

        for url in catalogURLs {
            let catalog = try YAMLParser.loadAgentCatalog(from: url)

            #expect(catalog.runtimeProfiles["claude_agent_acp"] != nil)
            #expect(catalog.runtimeProfiles["gemini_cli_acp"] != nil)

            for (id, profile) in catalog.backendProfiles where profile.provider == "claude_code" {
                #expect(profile.runtimeProfile == "claude_agent_acp", "Expected \(id) to use claude_agent_acp in \(url.lastPathComponent)")
            }

            for (id, profile) in catalog.backendProfiles where profile.provider == "gemini" {
                #expect(profile.runtimeProfile == "gemini_cli_acp", "Expected \(id) to use gemini_cli_acp in \(url.lastPathComponent)")
            }

            #expect(catalog.backendProfiles["gemini_review_pro"]?.model == "gemini-2.5-pro")
            #expect(catalog.backendProfiles["gemini_review_flash"] == nil)

            let uxReviewer = try #require(catalog.agents.first(where: { $0.id == "proposal_reviewer_ux" }))
            let uiReviewer = try #require(catalog.agents.first(where: { $0.id == "proposal_reviewer_ui" }))
            #expect(uxReviewer.backendProfile == "gemini_review_pro")
            #expect(uiReviewer.backendProfile == "gemini_review_pro")
        }
    }

    @Test func parseFullWorkflow() throws {
        let workflow = try YAMLParser.loadWorkflow(
            from: fixtureURL("workflow.yaml")
        )
        #expect(workflow.states.count == 12)
        #expect(workflow.initialState == "state_1_idea_received")
    }

    @Test func parseCompactWorkflow() throws {
        let compact = try YAMLParser.loadCompactWorkflow(
            from: fixtureURL("proposal-to-release.yaml")
        )
        #expect(compact.workflow.stages.count == 10)
        #expect(compact.workflow.requiredProviders.contains("codex"))
        #expect(compact.workflow.requiredProviders.contains("claude_code"))
    }

    @Test func parseAgentDefinitions() throws {
        let catalog = try YAMLParser.loadAgentCatalog(
            from: fixtureURL("agents.yaml")
        )
        let ids = Set(catalog.agents.map(\.id))
        #expect(ids.contains("lead_orchestrator"))
        #expect(ids.contains("code_writer"))
        #expect(ids.contains("security_checker"))
        #expect(ids.contains("proposal_writer"))
    }

    @Test func parsePermissionProfiles() throws {
        let catalog = try YAMLParser.loadAgentCatalog(
            from: fixtureURL("agents.yaml")
        )
        let profiles = Set(catalog.permissionProfiles.keys)
        #expect(profiles.contains("ORCH"))
        #expect(profiles.contains("RO_REVIEW"))
        #expect(profiles.contains("CODE_WRITE"))
        #expect(profiles.contains("RELEASE_GIT"))
    }

    @Test func parseBackendProfiles() throws {
        let catalog = try YAMLParser.loadAgentCatalog(
            from: fixtureURL("agents.yaml")
        )
        #expect(catalog.backendProfiles["claude_orchestrator_high"]?.provider == "claude_code")
        #expect(catalog.backendProfiles["codex_builder_high"]?.provider == "codex")
    }

    @Test func exampleCatalogKeepsProposalWriterOnCurrentWriterProfile() throws {
        let repoRoot = testRepositoryRootURL()
        let catalog = try YAMLParser.loadAgentCatalog(
            from: repoRoot.appendingPathComponent("examples/agents/agents.yaml")
        )
        let writer = try #require(catalog.agents.first(where: { $0.id == "proposal_writer" }))
        #expect(writer.backendProfile == "codex_writer_high")
    }

    @Test func parseWorkflowStates() throws {
        let workflow = try YAMLParser.loadWorkflow(
            from: fixtureURL("workflow.yaml")
        )
        let state1 = workflow.states["state_1_idea_received"]
        #expect(state1?.type == "start")
        #expect(state1?.owner == "lead_orchestrator")

        let gate = workflow.states["state_3_initial_proposal_approval"]
        #expect(gate?.type == "manual_gate")
        #expect(gate?.approval == "required")
    }

    @Test func invalidYAMLThrows() throws {
        let tmpURL = FileManager.default.temporaryDirectory.appendingPathComponent("bad_\(UUID()).yaml")
        try "{{{{not yaml".write(to: tmpURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tmpURL) }

        #expect(throws: YAMLParserError.self) {
            _ = try YAMLParser.loadAgentCatalog(from: tmpURL)
        }
    }

    @Test func fileNotFoundThrows() {
        let url = URL(fileURLWithPath: "/nonexistent/\(UUID()).yaml")
        #expect(throws: YAMLParserError.self) {
            _ = try YAMLParser.loadAgentCatalog(from: url)
        }
    }

    // MARK: - REQ-011: Artifact contracts

    @Test func testParseArtifactContracts() throws {
        let catalog = try YAMLParser.loadAgentCatalog(from: fixtureURL("agents.yaml"))
        #expect(catalog.contracts.count == 28)  // Canonical catalog now includes proposal-loop, strategy, and exact output contracts
    }

    // MARK: - REQ-011: Transitions

    @Test func testParseTransitions() throws {
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        let state1 = workflow.states["state_1_idea_received"]
        #expect(state1 != nil)
        #expect(state1?.transitions != nil)
        #expect(state1!.transitions!.count >= 1)

        let firstTransition = state1!.transitions!.first!
        #expect(firstTransition.to == "state_2_proposal_drafted")
        #expect(firstTransition.when == "exists('idea_brief')")
    }

    // MARK: - REQ-011: Loop config

    @Test func testParseLoopConfig() throws {
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        let state5 = workflow.states["state_5_proposal_refined"]
        #expect(state5 != nil)
        #expect(state5?.loop != nil)
        #expect(state5!.loop!.counter == "proposal_revision_cycles")
        #expect(state5!.loop!.max == "vars.max_proposal_revision_cycles")
    }

    // MARK: - REQ-011: Failure policy

    @Test func testParseFailurePolicy() throws {
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        #expect(workflow.failurePolicy != nil)
        #expect(workflow.failurePolicy!.onError == "pause_and_require_human")
        #expect(workflow.failurePolicy!.onLoopBudgetExhausted == "pause_and_require_human")
        #expect(workflow.failurePolicy!.preserveArtifacts == true)
    }

    // MARK: - REQ-011: Missing required fields throws

    @Test func testMissingRequiredFieldsThrows() throws {
        let tmpURL = FileManager.default.temporaryDirectory.appendingPathComponent("incomplete_\(UUID()).yaml")
        try "schema_version: 1\n".write(to: tmpURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tmpURL) }

        #expect(throws: YAMLParserError.self) {
            _ = try YAMLParser.loadWorkflow(from: tmpURL)
        }
    }

    // MARK: - REQ-011: Compact stages parsed

    @Test func testCompactStagesParsed() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        #expect(compact.workflow.stages.count == 10)

        let expectedIDs: Set<String> = [
            "draft_initial_proposal",
            "approve_initial_proposal",
            "initial_review",
            "rewrite_proposal",
            "approve_implementation_start",
            "start_implementation",
            "verification",
            "approve_release",
            "commit_and_push",
            "build_and_publish"
        ]
        let actualIDs = Set(compact.workflow.stages.map(\.id))
        #expect(actualIDs == expectedIDs)
    }

    // MARK: - REQ-011: Compact required providers

    @Test func testCompactRequiredProvidersParsed() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        #expect(compact.workflow.requiredProviders.contains("codex"))
        #expect(compact.workflow.requiredProviders.contains("claude_code"))
        #expect(compact.workflow.requiredProviders.count == 2)
    }

    // MARK: - REQ-011: Compact approval stages

    @Test func testCompactApprovalStages() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        let approvalStages = compact.workflow.stages.filter { $0.type == "approval" }
        #expect(!approvalStages.isEmpty)
        for stage in approvalStages {
            #expect(stage.approval == "required", "Approval stage '\(stage.id)' must have approval: required")
        }
    }

    // MARK: - REQ-011: Compact fanout stages

    @Test func testCompactFanoutStages() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        let fanoutStages = compact.workflow.stages.filter { $0.type == "fanout" }
        #expect(!fanoutStages.isEmpty)
        for stage in fanoutStages {
            #expect(stage.agents != nil && !stage.agents!.isEmpty, "Fanout stage '\(stage.id)' must have non-empty agents array")
        }
    }

    // MARK: - REQ-011: Compact needs references

    @Test func testCompactNeedsReferences() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        let allIDs = Set(compact.workflow.stages.map(\.id))
        for stage in compact.workflow.stages {
            guard let needs = stage.needs else { continue }
            for needed in needs {
                #expect(allIDs.contains(needed), "Stage '\(stage.id)' needs '\(needed)' which must be a real stage ID")
            }
        }
    }
}

// MARK: - Validator Tests (Full Coverage)

@Suite("YAML Validator", .serialized)
struct YAMLValidatorTests {

    // Happy path
    @Test func validConfigPassesValidation() throws {
        let catalog = try YAMLParser.loadAgentCatalog(from: fixtureURL("agents.yaml"))
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        let issues = YAMLValidator.validateAll(workflow: workflow, catalog: catalog)
        let errors = issues.filter { $0.severity == .error }
        #expect(errors.isEmpty, "Expected 0 errors but got: \(errors.map(\.message))")
    }

    // --- State Graph ---

    @Test func missingInitialState() {
        let wf = makeWorkflow(
            initialState: "nonexistent",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let issues = YAMLValidator.validateStateGraph(wf)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("initial_state") && $0.message.contains("not found") }))
    }

    @Test func brokenTransition() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil,
                                          transitions: [Transition(to: "nonexistent", when: "always")])]
        )
        let issues = YAMLValidator.validateStateGraph(wf)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("non-existent state") }))
    }

    @Test func orphanState() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: [
                "s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil),
                "s_orphan": WorkflowState(label: "Orphan", type: nil, owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)
            ]
        )
        let issues = YAMLValidator.validateStateGraph(wf)
        #expect(issues.contains(where: { $0.message.contains("unreachable") && $0.message.contains("s_orphan") }))
    }

    @Test func noEndState() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let issues = YAMLValidator.validateStateGraph(wf)
        #expect(issues.contains(where: { $0.message.contains("No state with type 'end'") }))
    }

    // --- Agent ↔ Workflow ---

    @Test func missingAgentInWorkflow() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "agent1", approval: nil,
                                          run: RunBlock(sequence: [AgentTask(agent: "missing_agent", task: "do_stuff", inputs: nil, outputs: nil)], parallel: nil, then: nil),
                                          runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(id: "agent1", backendProfile: "bp1", permissionProfile: "pp1", skillRef: "sk1")]
        )
        let issues = YAMLValidator.validateAgentReferences(workflow: wf, catalog: catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("missing_agent") }))
    }

    @Test func missingOwnerInWorkflow() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "nonexistent_owner", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let catalog = makeCatalog()
        let issues = YAMLValidator.validateAgentReferences(workflow: wf, catalog: catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent_owner") }))
    }

    // --- Catalog Internal Consistency ---

    @Test func brokenBackendProfileRef() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(backendProfile: "nonexistent_bp")]
        )
        let issues = YAMLValidator.validateBackendProfileRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent_bp") }))
    }

    @Test func brokenPermissionProfileRef() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            backendProfiles: ["bp1": dummyBackendProfile],
            agents: [makeAgent(permissionProfile: "nonexistent_pp")]
        )
        let issues = YAMLValidator.validatePermissionProfileRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent_pp") }))
    }

    @Test func brokenSkillRef() {
        let catalog = makeCatalog(
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(skillRef: "nonexistent_skill")]
        )
        let issues = YAMLValidator.validateSkillRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent_skill") }))
    }

    @Test func brokenOutputContractRef() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(outputContract: "nonexistent_contract")]
        )
        let issues = YAMLValidator.validateOutputContractRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent_contract") }))
    }

    @Test func sharedOutputContractCannotSilentlyCoverHeterogeneousOutputs() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            contracts: [
                "implementation_self_assessment_v1": ArtifactContract(format: "json", requiredFields: ["seemingly_complete"])
            ],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [
                makeAgent(
                    id: "code_writer",
                    outputs: ["implementation_progress", "implementation_self_assessment", "changed_files_manifest", "tests_result"],
                    outputContract: "implementation_self_assessment_v1"
                )
            ]
        )

        let issues = YAMLValidator.validateOutputContractRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("implementation_progress") && $0.message.contains("falls back to shared output_contract") }))
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("changed_files_manifest") && $0.message.contains("falls back to shared output_contract") }))
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("tests_result") && $0.message.contains("falls back to shared output_contract") }))
    }

    @Test func dedicatedOutputContractsMakeSharedContractSafe() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            contracts: [
                "implementation_self_assessment_v1": ArtifactContract(format: "json", requiredFields: ["seemingly_complete"]),
                "implementation_progress": ArtifactContract(format: "json", requiredFields: ["status"]),
                "changed_files_manifest": ArtifactContract(format: "json", requiredFields: ["files"]),
                "tests_result": ArtifactContract(format: "json", requiredFields: ["green"])
            ],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [
                makeAgent(
                    id: "code_writer",
                    outputs: ["implementation_progress", "implementation_self_assessment", "changed_files_manifest", "tests_result"],
                    outputContract: "implementation_self_assessment_v1"
                )
            ]
        )

        let issues = YAMLValidator.validateOutputContractRefs(catalog)
        #expect(issues.filter { $0.severity == .error }.isEmpty)
    }

    @Test func brokenMCPProfileRef() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(id: "reviewer", mcpProfile: "nonexistent_profile")]
        )

        let issues = YAMLValidator.validateMCPRefs(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("non-existent MCP profile") }))
    }

    @Test func brokenArtifactRef() {
        let catalog = makeCatalog(
            skills: ["sk1": dummySkillRef],
            backendProfiles: ["bp1": dummyBackendProfile],
            permissionProfiles: ["pp1": dummyPermissionProfile],
            agents: [makeAgent(inputs: ["nonexistent_artifact"])]
        )
        let issues = YAMLValidator.validateArtifactRefs(catalog)
        #expect(issues.contains(where: { $0.message.contains("nonexistent_artifact") }))
    }

    // --- Provider Coverage ---

    @Test func missingRequiredProvider() {
        let wf = makeWorkflow(requiredProviders: ["missing_provider"])
        let catalog = makeCatalog(
            backendProfiles: ["bp1": BackendProfile(provider: "claude_code", model: "opus", effort: "high", temperature: 0.0, maxTurns: 10, structuredOutput: "json")]
        )
        let issues = YAMLValidator.validateProviderCoverage(workflow: wf, catalog: catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("missing_provider") }))
    }

    // --- Env Placeholders ---

    @Test func validEnvPlaceholders() {
        let catalog = makeCatalog(paths: ["root": "${HOME:-/default/path}"])
        let issues = YAMLValidator.validateEnvPlaceholders(catalog)
        let errors = issues.filter { $0.severity == .error }
        #expect(errors.isEmpty)
        // Placeholder with default should not generate a warning
        let warnings = issues.filter { $0.severity == .warning }
        #expect(warnings.isEmpty)
    }

    @Test func malformedEnvPlaceholder() {
        let catalog = makeCatalog(paths: ["broken": "${UNCLOSED_VAR"])
        let issues = YAMLValidator.validateEnvPlaceholders(catalog)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("Malformed") }))
    }

    @Test func envPlaceholderWithoutDefault() {
        let catalog = makeCatalog(paths: ["root": "${MY_VAR}"])
        let issues = YAMLValidator.validateEnvPlaceholders(catalog)
        #expect(issues.contains(where: { $0.severity == .warning && $0.message.contains("no default value") }))
    }

    // --- Run Block Semantics ---

    @Test func emptySequenceBlock() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil,
                                          run: RunBlock(sequence: [], parallel: nil, then: nil),
                                          runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let issues = YAMLValidator.validateRunBlockSemantics(wf)
        #expect(issues.contains(where: { $0.message.contains("Empty sequence") }))
    }

    @Test func emptyFanoutAgents() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil,
                                          run: RunBlock(sequence: nil, parallel: [], then: nil),
                                          runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let issues = YAMLValidator.validateRunBlockSemantics(wf)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("Empty parallel") }))
    }

    @Test func duplicateAgentInThen() {
        let wf = makeWorkflow(
            initialState: "s1",
            states: ["s1": WorkflowState(label: "S1", type: "start", owner: "o", approval: nil,
                                          run: RunBlock(
                                              sequence: nil,
                                              parallel: [AgentTask(agent: "dup_agent", task: "review", inputs: nil, outputs: nil)],
                                              then: [AgentTask(agent: "dup_agent", task: "aggregate", inputs: nil, outputs: nil)]
                                          ),
                                          runAfterApproval: nil, loop: nil, transitions: nil)]
        )
        let issues = YAMLValidator.validateRunBlockSemantics(wf)
        #expect(issues.contains(where: { $0.message.contains("dup_agent") && $0.message.contains("both parallel and then") }))
    }
}

// MARK: - Compact Validator Tests

@Suite("Compact Workflow Validator")
struct CompactWorkflowValidatorTests {

    @Test func compactValidatorCanonicalPasses() throws {
        let compact = try YAMLParser.loadCompactWorkflow(from: fixtureURL("proposal-to-release.yaml"))
        let issues = CompactWorkflowValidator.validate(compact)
        let errors = issues.filter { $0.severity == .error }
        #expect(errors.isEmpty, "Expected 0 errors but got: \(errors.map(\.message))")
    }

    @Test func uniqueStageIDs() {
        let compact = makeCompact(stages: [
            CompactStage(id: "dup", type: "single", agent: "a", agents: nil, approval: nil, needs: nil, gate: nil),
            CompactStage(id: "dup", type: "single", agent: "b", agents: nil, approval: nil, needs: nil, gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("Duplicate stage ID") }))
    }

    @Test func needsExist() {
        let compact = makeCompact(stages: [
            CompactStage(id: "s1", type: "single", agent: "a", agents: nil, approval: nil, needs: ["nonexistent"], gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("nonexistent") && $0.message.contains("doesn't exist") }))
    }

    @Test func fanoutNonEmpty() {
        let compact = makeCompact(stages: [
            CompactStage(id: "s1", type: "fanout", agent: nil, agents: [], approval: nil, needs: nil, gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("no agents") }))
    }

    @Test func hasEntryPoint() {
        let compact = makeCompact(stages: [
            CompactStage(id: "s1", type: "single", agent: "a", agents: nil, approval: nil, needs: ["s2"], gate: nil),
            CompactStage(id: "s2", type: "single", agent: "b", agents: nil, approval: nil, needs: ["s1"], gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("No entry point") }))
    }

    @Test func noCycles() {
        let compact = makeCompact(stages: [
            CompactStage(id: "entry", type: "single", agent: "a", agents: nil, approval: nil, needs: nil, gate: nil),
            CompactStage(id: "s1", type: "single", agent: "b", agents: nil, approval: nil, needs: ["s2"], gate: nil),
            CompactStage(id: "s2", type: "single", agent: "c", agents: nil, approval: nil, needs: ["s1"], gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        #expect(issues.contains(where: { $0.severity == .error && $0.message.contains("Circular") }))
    }

    @Test func noAgentCatalogCheck() {
        // Compact aliases are NOT checked against catalog — this is by design
        let compact = makeCompact(stages: [
            CompactStage(id: "s1", type: "single", agent: "aliased-agent-name", agents: nil, approval: nil, needs: nil, gate: nil)
        ])
        let issues = CompactWorkflowValidator.validate(compact)
        let errors = issues.filter { $0.severity == .error }
        // No error about agent not found in catalog
        #expect(!errors.contains(where: { $0.message.contains("not found in catalog") }))
    }
}

// MARK: - Hasher Tests

@Suite("Definition Hasher")
struct DefinitionHasherTests {
    @Test func deterministic() throws {
        let config = ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch")
        var hashes = Set<String>()
        for _ in 0..<100 {
            let result = try DefinitionHasher.hash(config)
            hashes.insert(result.sha256)
        }
        #expect(hashes.count == 1, "Hash should be deterministic")
    }

    @Test func changesOnMutation() throws {
        let c1 = ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch")
        let c2 = ExecutionConfig(singleActiveRunPerIdea: false, resumePolicy: "manual")
        let h1 = try DefinitionHasher.hash(c1).sha256
        let h2 = try DefinitionHasher.hash(c2).sha256
        #expect(h1 != h2)
    }

    @Test func snapshotRoundTripWorkflow() throws {
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        let (data, hash1) = try DefinitionHasher.hash(workflow)

        // Deserialize and re-hash
        let decoded = try JSONDecoder().decode(WorkflowDefinition.self, from: data)
        let (_, hash2) = try DefinitionHasher.hash(decoded)
        #expect(hash1 == hash2, "Snapshot round-trip must produce identical hash")
    }

    @Test func snapshotRoundTripCatalog() throws {
        let catalog = try YAMLParser.loadAgentCatalog(from: fixtureURL("agents.yaml"))
        let (data, hash1) = try DefinitionHasher.hash(catalog)

        // Deserialize and re-hash
        let decoded = try JSONDecoder().decode(AgentCatalog.self, from: data)
        let (_, hash2) = try DefinitionHasher.hash(decoded)
        #expect(hash1 == hash2, "Snapshot round-trip must produce identical hash")
    }

    @Test func sortedKeysRequired() throws {
        // Verify .sortedKeys produces stable output for [String:T] types
        let catalog = try YAMLParser.loadAgentCatalog(from: fixtureURL("agents.yaml"))
        var hashes = Set<String>()
        for _ in 0..<10 {
            let result = try DefinitionHasher.hash(catalog)
            hashes.insert(result.sha256)
        }
        #expect(hashes.count == 1, "Sorted keys should produce deterministic hash for dictionary types")
    }

    // REQ-008: Hash stability across simulated app launches (write JSON to disk, read back, re-hash)
    @Test func testDefinitionHashStableAcrossAppLaunches() throws {
        let workflow = try YAMLParser.loadWorkflow(from: fixtureURL("workflow.yaml"))
        let (data, hash1) = try DefinitionHasher.hash(workflow)

        // Write JSON to a temp file (simulating persistence across app launches)
        let tmpURL = FileManager.default.temporaryDirectory.appendingPathComponent("hash_stability_\(UUID()).json")
        try data.write(to: tmpURL)
        defer { try? FileManager.default.removeItem(at: tmpURL) }

        // Read back from disk
        let reloadedData = try Data(contentsOf: tmpURL)
        let decoded = try JSONDecoder().decode(WorkflowDefinition.self, from: reloadedData)

        // Re-hash the decoded definition
        let (_, hash2) = try DefinitionHasher.hash(decoded)
        #expect(hash1 == hash2, "Hash must be identical after writing JSON to disk and reading it back")
    }
}
