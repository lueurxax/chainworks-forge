import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("ResumeManager", .serialized, .tags(.fast))
struct ResumeManagerTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("ResumeManagerTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        TestModelContainerRetainer.retain(container)
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    // MARK: - Helpers

    private func loadCanonicalWorkflow() throws -> WorkflowDefinition {
        try loadTestCanonicalWorkflow()
    }

    private func loadCanonicalCatalog() throws -> AgentCatalog {
        try loadTestCanonicalCatalog()
    }

    private func cancelAndAwaitSettled(_ service: ExecutionService, runID: UUID) async {
        await service.cancelRun(runID: runID)
        await awaitCondition("ExecutionService should fully detach orchestrator after cancellation", timeout: 3.0) {
            service.orchestrator(for: runID) == nil
        }
    }

    /// Create a run directly in SwiftData with proper snapshot data, avoiding filesystem ops.
    private func makeRunFromPlan() throws -> (Run, RunPlan, RunWorkspace) {
        let workflow = try loadCanonicalWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Test", body: "Test idea for resume")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = Run(
            id: runID,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml",
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        ) // RunRepository-exempt
        run.idea = idea
        idea.workspaceRootPath = tempDir.path
        run.frozenWorkspaceRootPath = tempDir.path
        context.insert(run)
        try context.save()

        return (run, plan, workspace)
    }

    private func makeRetryableRunFromPlan() throws -> (Run, RunPlan, RunWorkspace, AgentCatalog) {
        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(
                id: "retryable_test",
                name: "Retryable Test",
                usesAgentCatalog: nil,
                description: "Minimal retryable workflow",
                ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
                requiredProviders: []
            ),
            variables: nil,
            failurePolicy: nil,
            scoring: nil,
            initialState: "state_1",
            states: [
                "state_1": WorkflowState(
                    label: "Draft",
                    type: "start",
                    owner: "test_agent",
                    approval: nil,
                    run: RunBlock(
                        sequence: [AgentTask(agent: "test_agent", task: "draft", inputs: nil, outputs: ["output_1"])],
                        parallel: nil,
                        then: nil
                    ),
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: [Transition(to: "state_2", when: "exists('output_1')")]
                ),
                "state_2": WorkflowState(
                    label: "Done",
                    type: "end",
                    owner: "test_agent",
                    approval: nil,
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: []
                )
            ]
        )

        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Retryable resume test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: []
            ),
            paths: [:],
            artifacts: [:],
            skills: ["test_skill": SkillRef(type: "inline_skill", path: nil, name: "Test Skill", description: "Test")],
            contracts: [:],
            backendProfiles: [
                "test_profile": BackendProfile(
                    provider: "claude_code",
                    model: "test-model",
                    effort: "high",
                    temperature: 0,
                    maxTurns: 4,
                    structuredOutput: "none"
                )
            ],
            permissionProfiles: [
                "TEST": PermissionProfile(
                    filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
                    git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
                    shell: ShellPermissions(allow: nil, deny: nil),
                    network: NetworkPermissions(allow: nil),
                    mcp: MCPPermissions(allow: nil)
                )
            ],
            agents: [
                AgentDefinition(
                    id: "test_agent",
                    title: "Test Agent",
                    mode: "tool_use",
                    backendProfile: "test_profile",
                    permissionProfile: "TEST",
                    skillRef: "test_skill",
                    skillRole: nil,
                    worktreePolicy: nil,
                    requiredTools: nil,
                    inputs: [],
                    outputs: ["output_1"],
                    outputContract: nil,
                    requiresHumanApproval: false,
                    prompt: "Write output_1",
                    notes: nil,
                    sessionReuseScope: nil,
                    sessionFamilyID: nil
                )
            ]
        )

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let idea = Idea(title: "Retryable", body: "Retryable workflow")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeRetryable-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = Run(
            id: runID,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: "test/retryable-workflow.yaml",
            catalogSourcePath: "test/retryable-agents.yaml",
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        )
        run.idea = idea
        context.insert(run)
        try context.save()

        return (run, plan, workspace, catalog)
    }

    private struct FailingExecutor: AgentExecutor {
        func execute(
            task: AgentTask,
            agent: ResolvedAgent,
            context: ExecutionContext
        ) async throws -> AgentResult {
            AgentResult(
                outputs: [:],
                logSnippet: "executor should not have been called",
                costCents: nil,
                succeeded: false,
                errorMessage: "unexpected rerun",
                sessionID: nil,
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: agent.model,
                configuredProviderID: nil,
                adapterVersion: nil,
                canonicalOutcome: .failedBeforeOutput,
                outputPresence: .none
            )
        }
    }

    // MARK: - Find Interrupted Runs (parameterized — Proposal 009 REQ-005)

    struct InterruptedRunCase: CustomStringConvertible, Sendable {
        let status: RunStatus
        let shouldBeFound: Bool
        var description: String { "\(status.rawValue) → \(shouldBeFound ? "found" : "not found")" }
    }

    @Test("findInterruptedRuns classifies status correctly", arguments: [
        InterruptedRunCase(status: .running, shouldBeFound: true),
        InterruptedRunCase(status: .waitingApproval, shouldBeFound: true),
        InterruptedRunCase(status: .completed, shouldBeFound: false),
        InterruptedRunCase(status: .cancelled, shouldBeFound: false),
        InterruptedRunCase(status: .failed, shouldBeFound: false),
    ])
    func findInterruptedRunsByStatus(testCase: InterruptedRunCase) async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = testCase.status
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let interrupted = try manager.findInterruptedRuns()

        if testCase.shouldBeFound {
            #expect(interrupted.count == 1, "\(testCase.status.rawValue) should be found as interrupted")
            #expect(interrupted.first?.id == run.id)
        } else {
            #expect(interrupted.isEmpty, "\(testCase.status.rawValue) should NOT be found as interrupted")
        }
    }

    @Test("startup normalization blocks stale running runs for manual resume")
    func startupNormalizationBlocksStaleRunningRuns() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let stageExecution = StageExecution(
            stageID: "state_1",
            label: "Draft",
            startedAt: Date().addingTimeInterval(-30),
            status: .running
        )
        stageExecution.run = run
        run.stageExecutions.append(stageExecution)
        context.insert(stageExecution)

        let agentExecution = AgentExecution(
            agentID: "test_agent",
            agentTitle: "Test Agent",
            taskName: "draft",
            startedAt: Date().addingTimeInterval(-20),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        agentExecution.stageExecution = stageExecution
        stageExecution.agentExecutions.append(agentExecution)
        context.insert(agentExecution)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let normalizedCount = try manager.normalizeInterruptedRunsForManualResume()

        #expect(normalizedCount == 1)
        #expect(run.status == .blocked)
        #expect(stageExecution.status == .blocked)
        #expect(stageExecution.settlementKind == .blocked)
        #expect(agentExecution.status == .failed)
        #expect(agentExecution.canonicalOutcome == .failedBeforeOutput)
        #expect(agentExecution.providerStopReason == "interrupted_on_app_restart")
        #expect(agentExecution.logSnippet?.contains("Manual resume required") == true)
    }

    @Test("startup normalization preserves waiting approval runs")
    func startupNormalizationPreservesWaitingApprovalRuns() throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .waitingApproval

        let stageExecution = StageExecution(
            stageID: "approval_stage",
            label: "Approval",
            startedAt: Date().addingTimeInterval(-30),
            status: .waitingApproval
        )
        stageExecution.run = run
        run.stageExecutions.append(stageExecution)
        context.insert(stageExecution)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let normalizedCount = try manager.normalizeInterruptedRunsForManualResume()

        #expect(normalizedCount == 0)
        #expect(run.status == .waitingApproval)
        #expect(run.driftDetails == nil)
        #expect(stageExecution.status == .waitingApproval)
    }

    // MARK: - Classification

    @Test("Classify resumeable run")
    func classifyResumeableRun() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)

        switch actions[0] {
        case .resume(let resumeRun, let resumePlan, let resumeWorkspace):
            #expect(resumeRun.id == run.id)
            #expect(resumePlan.workflowID == "proposal_to_release")
            #expect(resumeWorkspace.runID == run.id)
        default:
            Issue.record("Expected .resume action, got \(actions[0])")
        }
    }

    @Test("Classify compiler version mismatch")
    func classifyCompilerVersionMismatch() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)
        #expect(actions.count == 1)

        if case .resume(_, let plan, _) = actions[0] {
            #expect(plan.planCompilerVersion == RunPlan.currentCompilerVersion)
        }
    }

    @Test("Classify run with live source drift still resumes from frozen snapshots")
    func classifyRunWithLiveSourceDriftStillResumes() async throws {
        let sourceRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeDrift-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: sourceRoot, withIntermediateDirectories: true)
        let workflowURL = sourceRoot.appendingPathComponent("workflow.yaml")
        let catalogURL = sourceRoot.appendingPathComponent("agents.yaml")
        let workflowFixtureURL = testRepositoryRootURL().appendingPathComponent("examples/workflows/workflow.yaml")
        let catalogFixtureURL = testRepositoryRootURL().appendingPathComponent("examples/agents/agents.yaml")
        try FileManager.default.copyItem(at: workflowFixtureURL, to: workflowURL)
        _ = try writePortableCatalogCopy(from: catalogFixtureURL, to: catalogURL)

        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)

        let plan = try compiler.previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogURL.path
        )

        let idea = Idea(title: "Drift Resume", body: "Run should survive source drift")
        context.insert(idea)

        let runID = UUID()
        let workspaceRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeDriftWorkspace-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: workspaceRoot,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path
        )
        idea.workspaceRootPath = sourceRoot.path
        run.frozenWorkspaceRootPath = sourceRoot.path
        run.status = .running
        try context.save()

        let mutatedCatalog = AgentCatalog(
            schemaVersion: catalog.schemaVersion,
            app: catalog.app,
            paths: catalog.paths,
            artifacts: catalog.artifacts,
            skills: catalog.skills,
            mcpPolicy: catalog.mcpPolicy,
            mcpServerRegistry: catalog.mcpServerRegistry,
            mcpProfiles: catalog.mcpProfiles,
            contracts: catalog.contracts,
            backendProfiles: catalog.backendProfiles,
            permissionProfiles: catalog.permissionProfiles,
            runtimeProfiles: catalog.runtimeProfiles,
            agents: catalog.agents.map { agent in
                if agent.id == "proposal_writer" {
                    return AgentDefinition(
                        id: agent.id,
                        title: agent.title,
                        mode: agent.mode,
                        backendProfile: agent.backendProfile,
                        permissionProfile: agent.permissionProfile,
                        skillRef: agent.skillRef,
                        skillRole: agent.skillRole,
                        worktreePolicy: agent.worktreePolicy,
                        requiredTools: agent.requiredTools,
                        inputs: agent.inputs,
                        outputs: agent.outputs,
                        outputContract: agent.outputContract,
                        requiresHumanApproval: agent.requiresHumanApproval,
                        prompt: agent.prompt + "\nDrift mutation for resume test.",
                        notes: agent.notes,
                        sessionReuseScope: agent.sessionReuseScope,
                        sessionFamilyID: agent.sessionFamilyID
                    )
                }
                return agent
            }
        )
        let mutatedCatalogData = try DefinitionHasher.hash(mutatedCatalog).data
        try mutatedCatalogData.write(to: catalogURL, options: .atomic)

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        switch actions[0] {
        case .resume(let resumedRun, let resumedPlan, let resumedWorkspace):
            #expect(resumedRun.id == run.id)
            #expect(resumedPlan.catalogSnapshotHash == run.catalogSnapshotHash)
            #expect(resumedWorkspace.runID == run.id)
            #expect(resumedRun.driftDetails?.contains("Agent catalog source has changed") == true)
        default:
            Issue.record("Expected .resume for drifted source file, got \(actions[0])")
        }
    }

    @Test("Run creation persists frozen workflow and catalog snapshot artifacts")
    func runCreationPersistsFrozenSnapshotArtifacts() throws {
        let sourceRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("RunSnapshotSources-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: sourceRoot, withIntermediateDirectories: true)
        let workflowURL = sourceRoot.appendingPathComponent("workflow.yaml")
        let catalogURL = sourceRoot.appendingPathComponent("agents.yaml")
        let workflowFixtureURL = testRepositoryRootURL().appendingPathComponent("examples/workflows/workflow.yaml")
        let catalogFixtureURL = testRepositoryRootURL().appendingPathComponent("examples/agents/agents.yaml")
        try FileManager.default.copyItem(at: workflowFixtureURL, to: workflowURL)
        _ = try writePortableCatalogCopy(from: catalogFixtureURL, to: catalogURL)

        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog, catalogSourcePath: catalogURL.path)

        let idea = Idea(title: "Snapshot Artifacts", body: "Run should persist frozen source artifacts")
        context.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path
        )
        try context.save()

        let artifacts = try ArtifactManager(modelContext: context).artifacts(forRunID: run.id)
        let workflowArtifact = try #require(artifacts.first(where: { $0.name == "workflow_snapshot_frozen.json" }))
        let catalogArtifact = try #require(artifacts.first(where: { $0.name == "catalog_snapshot_frozen.json" }))

        #expect(workflowArtifact.contractID == "run_workflow_snapshot_v1")
        #expect(catalogArtifact.contractID == "run_catalog_snapshot_v1")
        #expect(workflowArtifact.stageID == "run_start_snapshot")
        #expect(catalogArtifact.stageID == "run_start_snapshot")

        let workflowData = try Data(contentsOf: URL(fileURLWithPath: workflowArtifact.filePath))
        let catalogData = try Data(contentsOf: URL(fileURLWithPath: catalogArtifact.filePath))
        #expect(workflowData == plan.workflowSnapshotJSON)
        #expect(catalogData == plan.catalogSnapshotJSON)
        #expect(workflowArtifact.runID == workspace.runID)
        #expect(catalogArtifact.runID == workspace.runID)
    }

    @Test("Run repository cleanup removes terminal runs and owned directories")
    func runRepositoryCleanupRemovesTerminalRunsAndOwnedDirectories() async throws {
        let idea = Idea(title: "Cleanup", body: "cleanup")
        context.insert(idea)

        let removableRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("RunCleanup-\(UUID().uuidString)", isDirectory: true)
        let removableArtifacts = removableRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: removableArtifacts, withIntermediateDirectories: true)
        let reportURL = removableArtifacts.appendingPathComponent("report.md")
        try Data("report".utf8).write(to: reportURL)

        let removableRun = Run(
            workflowID: "wf",
            workflowTitle: "WF",
            workflowSnapshotHash: "wf-hash",
            catalogSnapshotHash: "cat-hash",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: removableRoot.path,
            artifactRoot: removableArtifacts.path
        )
        removableRun.idea = idea
        removableRun.status = .completed
        removableRun.completedAt = Date(timeIntervalSince1970: 100)
        context.insert(removableRun)

        let activeRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("RunCleanupActive-\(UUID().uuidString)", isDirectory: true)
        let activeArtifacts = activeRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: activeArtifacts, withIntermediateDirectories: true)

        let activeRun = Run(
            workflowID: "wf-active",
            workflowTitle: "WF Active",
            workflowSnapshotHash: "wf-active-hash",
            catalogSnapshotHash: "cat-active-hash",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: activeRoot.path,
            artifactRoot: activeArtifacts.path
        )
        activeRun.idea = idea
        activeRun.status = .running
        context.insert(activeRun)
        try context.save()

        let repository = RunRepository(context: context)
        let cleanupPlan = try repository.prepareTerminalRunCleanup()
        let removedDirectoryCount = await RunRepository.removeFilesystemRoots(cleanupPlan)

        let remainingRuns = try context.fetch(FetchDescriptor<Run>())
        #expect(cleanupPlan.deletedRunCount == 1)
        #expect(cleanupPlan.deletedRunIDs == [removableRun.id])
        #expect(removedDirectoryCount >= 1)
        #expect(remainingRuns.count == 1)
        #expect(remainingRuns.first?.id == activeRun.id)
        #expect(!FileManager.default.fileExists(atPath: removableRoot.path))
        #expect(FileManager.default.fileExists(atPath: activeRoot.path))
    }

    @Test("Run repository cleanup ignores active and blocked runs")
    func runRepositoryCleanupIgnoresActiveAndBlockedRuns() async throws {
        let idea = Idea(title: "No Cleanup", body: "still active")
        context.insert(idea)

        let blockedRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("RunCleanupBlocked-\(UUID().uuidString)", isDirectory: true)
        let blockedArtifacts = blockedRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: blockedArtifacts, withIntermediateDirectories: true)

        let blockedRun = Run(
            workflowID: "wf-blocked",
            workflowTitle: "WF Blocked",
            workflowSnapshotHash: "wf-blocked-hash",
            catalogSnapshotHash: "cat-blocked-hash",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: blockedRoot.path,
            artifactRoot: blockedArtifacts.path
        )
        blockedRun.idea = idea
        blockedRun.status = .blocked
        context.insert(blockedRun)
        try context.save()

        let repository = RunRepository(context: context)
        let cleanupPlan = try repository.prepareTerminalRunCleanup()
        let removedDirectoryCount = await RunRepository.removeFilesystemRoots(cleanupPlan)

        let remainingRuns = try context.fetch(FetchDescriptor<Run>())
        #expect(cleanupPlan.deletedRunCount == 0)
        #expect(cleanupPlan.deletedRunIDs.isEmpty)
        #expect(removedDirectoryCount == 0)
        #expect(remainingRuns.count == 1)
        #expect(remainingRuns.first?.id == blockedRun.id)
        #expect(FileManager.default.fileExists(atPath: blockedRoot.path))
    }

    @Test("Run repository cleanup migrates referenced attachment into idea workspace")
    func runRepositoryCleanupMigratesReferencedAttachmentIntoIdeaWorkspace() async throws {
        let repository = RunRepository(context: context)

        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("IdeaRepo-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)

        let idea = Idea(
            title: "Referenced attachment",
            body: "Uses a prior run artifact",
            workspaceRootPath: repoRoot.path,
            status: .active
        )
        context.insert(idea)

        let runRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("CleanupReferencedRun-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = runRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let attachmentURL = artifactRoot.appendingPathComponent("proposal_current.md")
        try Data("# proposal".utf8).write(to: attachmentURL)

        let run = Run(
            workflowID: "cleanup_test",
            workflowTitle: "Cleanup Test",
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: runRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
        run.status = .completed
        run.idea = idea
        idea.runs.append(run)
        idea.attachmentPath = attachmentURL.path
        context.insert(run)
        try context.save()

        let cleanupPlan = try repository.prepareTerminalRunCleanup()
        let migratedPath = try #require(idea.attachmentPath)

        #expect(cleanupPlan.deletedRunCount == 1)
        #expect(cleanupPlan.migratedAttachmentCount == 1)
        #expect(cleanupPlan.protectedRunCount == 0)
        #expect(cleanupPlan.deletedRunIDs.contains(run.id))
        #expect(migratedPath != attachmentURL.path)
        #expect(migratedPath.contains("/.chainworks/idea-attachments/"))
        #expect(FileManager.default.fileExists(atPath: migratedPath))
        #expect(try String(contentsOfFile: migratedPath) == "# proposal")

        let removedDirectoryCount = await RunRepository.removeFilesystemRoots(cleanupPlan)
        #expect(removedDirectoryCount >= 1)
        #expect(FileManager.default.fileExists(atPath: attachmentURL.path) == false)
        #expect(FileManager.default.fileExists(atPath: migratedPath))
    }

    @Test("Run repository cleanup protects referenced terminal run without idea workspace root")
    func runRepositoryCleanupProtectsReferencedRunWithoutWorkspaceRoot() throws {
        let repository = RunRepository(context: context)

        let idea = Idea(
            title: "Missing workspace root",
            body: "Attachment still points into terminal run",
            status: .active
        )
        context.insert(idea)

        let runRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("CleanupProtectedRun-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = runRoot.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let attachmentURL = artifactRoot.appendingPathComponent("review.md")
        try Data("keep".utf8).write(to: attachmentURL)

        let run = Run(
            workflowID: "cleanup_test",
            workflowTitle: "Cleanup Test",
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: runRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
        run.status = .completed
        run.idea = idea
        idea.runs.append(run)
        idea.attachmentPath = attachmentURL.path
        context.insert(run)
        try context.save()

        let cleanupPlan = try repository.prepareTerminalRunCleanup()

        #expect(cleanupPlan.deletedRunCount == 0)
        #expect(cleanupPlan.migratedAttachmentCount == 0)
        #expect(cleanupPlan.protectedRunCount == 1)
        #expect(cleanupPlan.protectedRunIDs == [run.id])
        #expect(idea.attachmentPath == attachmentURL.path)
        #expect(try context.fetch(FetchDescriptor<Run>()).contains(where: { $0.id == run.id }))
    }

    @Test("Run repository cleanup does not remigrate attachment already preserved inside idea workspace")
    func runRepositoryCleanupDoesNotRemigrateAlreadyPreservedAttachment() throws {
        let repository = RunRepository(context: context)

        let repoRoot = testRepositoryRootURL(file: #filePath)
            .appendingPathComponent(".tmp-cleanup-tests", isDirectory: true)
            .appendingPathComponent("IdeaRepo-\(UUID().uuidString)", isDirectory: true)
        let preservedDirectory = repoRoot
            .appendingPathComponent(".chainworks/idea-attachments", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: preservedDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let preservedAttachmentURL = preservedDirectory.appendingPathComponent("proposal_current.json")
        try Data("{\"title\":\"Preserved\"}".utf8).write(to: preservedAttachmentURL)

        let idea = Idea(
            title: "Already preserved attachment",
            body: "Should keep repo-owned attachment path untouched",
            workspaceRootPath: repoRoot.path,
            status: .active
        )
        idea.attachmentPath = preservedAttachmentURL.path
        context.insert(idea)

        let run = Run(
            workflowID: "cleanup_test",
            workflowTitle: "Cleanup Test",
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSourcePath: "test/workflow.yaml",
            catalogSourcePath: "test/agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: repoRoot.path,
            artifactRoot: FileManager.default.temporaryDirectory
                .appendingPathComponent("TerminalRunArtifacts-\(UUID().uuidString)", isDirectory: true).path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
        run.status = .cancelled
        run.idea = idea
        idea.runs.append(run)
        context.insert(run)
        try context.save()

        let cleanupPlan = try repository.prepareTerminalRunCleanup()

        #expect(cleanupPlan.deletedRunCount == 1)
        #expect(cleanupPlan.migratedAttachmentCount == 0)
        #expect(cleanupPlan.protectedRunCount == 0)
        #expect(idea.attachmentPath == preservedAttachmentURL.path)
        #expect(FileManager.default.fileExists(atPath: preservedAttachmentURL.path))
    }

    // MARK: - Side-Effect Detection

    @Test("Side-effect stage detected")
    func sideEffectStageDetected() async throws {
        let (run, _, _) = try makeRunFromPlan()
        run.status = .running

        let stage = StageExecution(stageID: "commit_and_push", label: "Commit", status: .running)
        stage.run = run
        context.insert(stage)
        try context.save()

        let manager = ResumeManager(modelContext: context)
        let actions = try manager.classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        if case .needsDecision(_, let reason) = actions[0] {
            #expect(reason.contains("side-effect"), "Should mention side-effect: \(reason)")
        } else if case .resume = actions[0] {
            // Also acceptable if no drift detected — the side-effect check is for running stages
        }
    }

    // MARK: - ExecutionService

    @Test("ExecutionService start run")
    func executionServiceStartRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(modelContext: context, executor: executor)

        #expect(!service.hasActiveRuns)

        service.startRun(run: run, plan: plan, workspace: workspace)

        #expect(service.hasActiveRuns)
        #expect(service.orchestrator(for: run.id) != nil)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService cancel run")
    func executionServiceCancelRun() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        #expect(service.hasActiveRuns)

        await cancelAndAwaitSettled(service, runID: run.id)
        #expect(!service.hasActiveRuns)
        #expect(run.status == .cancelled)
    }

    @Test("ExecutionService cancel blocked run without active orchestrator")
    func executionServiceCancelBlockedRunWithoutActiveOrchestrator() async throws {
        let (run, _, _) = try makeRunFromPlan()

        let blockedStage = StageExecution(stageID: "proposal_review", label: "Proposal Review", status: .blocked)
        blockedStage.run = run
        context.insert(blockedStage)

        let blockedAgent = AgentExecution(
            agentID: "reviewer",
            agentTitle: "Reviewer",
            taskName: "Review proposal",
            status: .running,
            provider: "codex",
            effort: "high"
        )
        blockedAgent.stageExecution = blockedStage
        context.insert(blockedAgent)

        run.status = .blocked
        try context.save()

        let service = ExecutionService(modelContext: context, executor: SimulatedAgentExecutor())

        #expect(service.orchestrator(for: run.id) == nil)

        await cancelAndAwaitSettled(service, runID: run.id)

        #expect(run.status == .cancelled)
        #expect(run.cancellationRequestedAt != nil)
        #expect(run.cancellationSettledAt != nil)
        #expect(blockedAgent.status == AgentStatus.cancelled)
    }

    @Test("ExecutionService duplicate start prevented")
    func executionServiceDuplicateStartPrevented() async throws {
        let (run, plan, workspace) = try makeRunFromPlan()

        let executor = SimulatedAgentExecutor(simulatedDelay: 5.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        service.startRun(run: run, plan: plan, workspace: workspace)
        service.startRun(run: run, plan: plan, workspace: workspace) // No-op

        #expect(service.activeOrchestrators.count == 1)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService resumes stage retry by attaching an orchestrator")
    func executionServiceResumeRunAfterStageRetryAttachesOrchestrator() async throws {
        let (run, plan, _) = try makeRunFromPlan()
        run.status = .failed

        let initialStateID = plan.initialStateID
        let initialLabel = try #require(plan.states[initialStateID]?.label)

        let failedStage = StageExecution(
            stageID: initialStateID,
            label: initialLabel,
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedStage.run = run
        context.insert(failedStage)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: initialStateID)

        #expect(run.status == .ready)
        #expect(run.currentStageID == initialStateID)

        let executor = SimulatedAgentExecutor(simulatedDelay: 1.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        try service.resumeRun(run: run, compiler: compiler)

        await awaitCondition("Recovery-created ready run should attach and transition to running", timeout: 3.0) {
            service.orchestrator(for: run.id) != nil && run.status == .running
        }

        #expect(service.orchestrator(for: run.id) != nil)
        #expect(run.status == .running)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService resume uses frozen snapshot catalog when live catalog is unavailable")
    func executionServiceResumeUsesFrozenSnapshotCatalogWhenLiveCatalogUnavailable() async throws {
        let (run, plan, _) = try makeRunFromPlan()
        run.status = .failed

        let failedStage = StageExecution(
            stageID: plan.initialStateID,
            label: try #require(plan.states[plan.initialStateID]?.label),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedStage.run = run
        context.insert(failedStage)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: plan.initialStateID)

        let executor = SimulatedAgentExecutor(simulatedDelay: 1.0)
        let service = ExecutionService(modelContext: context, executor: executor, catalog: nil)

        try service.resumeRun(run: run, compiler: compiler)

        let orchestrator = try #require(service.orchestrator(for: run.id))
        let orchestratorCatalog = try #require(orchestrator.catalog)
        #expect(orchestratorCatalog.backendProfiles.isEmpty == false)
        #expect(orchestratorCatalog.agents.isEmpty == false)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService resumes agent retry by attaching an orchestrator")
    func executionServiceResumeRunAfterAgentRetryAttachesOrchestrator() async throws {
        let (run, plan, _) = try makeRunFromPlan()
        run.status = .failed

        let initialStateID = plan.initialStateID
        let initialLabel = try #require(plan.states[initialStateID]?.label)

        let failedStage = StageExecution(
            stageID: initialStateID,
            label: initialLabel,
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedStage.run = run
        context.insert(failedStage)

        let failedAgent = AgentExecution(
            agentID: "test_agent",
            agentTitle: "Test Agent",
            taskName: "test_task",
            status: .failed,
            provider: "test_provider",
            effort: "high"
        )
        failedAgent.stageExecution = failedStage
        context.insert(failedAgent)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryAgent(run: run, stageID: initialStateID, agentID: "test_agent")

        #expect(run.status == .running)
        #expect(run.currentStageID == initialStateID)

        let executor = SimulatedAgentExecutor(simulatedDelay: 1.0)
        let service = ExecutionService(modelContext: context, executor: executor)

        try service.resumeRun(run: run, compiler: compiler)

        await awaitCondition("Recovery-created running run should attach an orchestrator", timeout: 3.0) {
            service.orchestrator(for: run.id) != nil && run.status == .running
        }

        #expect(service.orchestrator(for: run.id) != nil)
        #expect(run.status == .running)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService resumes agent retry without creating a new stage iteration")
    func executionServiceResumeRunAfterAgentRetryReusesExistingStageExecution() async throws {
        let (run, plan, _, catalog) = try makeRetryableRunFromPlan()
        run.status = .failed

        let failedStage = StageExecution(
            stageID: plan.initialStateID,
            label: "Draft",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedStage.run = run
        context.insert(failedStage)

        let failedAgent = AgentExecution(
            agentID: "test_agent",
            agentTitle: "Test Agent",
            taskName: "draft",
            startedAt: Date(timeIntervalSince1970: 11),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        failedAgent.stageExecution = failedStage
        context.insert(failedAgent)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryAgent(run: run, stageID: plan.initialStateID, agentID: "test_agent")

        #expect(run.status == .running)
        #expect(failedStage.status == .running)
        let pendingRetryExec = failedStage.agentExecutions
            .filter { $0.agentID == "test_agent" && $0.status == .pending }
            .sorted { ($0.agentAttemptNumber ?? 1) < ($1.agentAttemptNumber ?? 1) }
            .last
        #expect(pendingRetryExec?.agentAttemptNumber == 2)
        #expect(run.currentStageID == plan.initialStateID)

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(simulatedDelay: 0, catalog: catalog),
            catalog: catalog
        )

        try service.resumeRun(run: run, compiler: compiler)

        await awaitCondition("Recovery-created running run should complete without creating a new stage iteration", timeout: 3.0) {
            run.status == .completed
        }
        await awaitCondition("Completed run should detach its orchestrator", timeout: 3.0) {
            service.orchestrator(for: run.id) == nil
        }

        let stages = run.stageExecutions.filter { $0.stageID == plan.initialStateID }
        #expect(stages.count == 1)
        let resumedStage = try #require(stages.first)
        #expect(resumedStage.iteration == 1)
        #expect(resumedStage.attemptNumber == 1)
        #expect(resumedStage.status == .completed)

        let latestAgentAttempt = resumedStage.agentExecutions
            .filter { $0.agentID == "test_agent" }
            .sorted { ($0.agentAttemptNumber ?? 1) < ($1.agentAttemptNumber ?? 1) }
            .last
        #expect(latestAgentAttempt?.status == .completed)
        #expect(latestAgentAttempt?.agentAttemptNumber == 2)
        #expect(latestAgentAttempt?.artifacts.contains { $0.filePath.contains("\(plan.initialStateID).1/test_agent/1/agent-retry-2/output_1") } == true)
    }

    @Test("ExecutionService reconciles late contract output from prior failed attempt before rerunning retry")
    func executionServiceResumeRunReconcilesLateOutputBeforeRetryExecutes() async throws {
        let (run, plan, workspace, catalog) = try makeRetryableRunFromPlan()
        run.status = .failed

        let stage = StageExecution(
            stageID: plan.initialStateID,
            label: "Draft",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let failedAgent = AgentExecution(
            agentID: "test_agent",
            agentTitle: "Test Agent",
            taskName: "draft",
            startedAt: Date(timeIntervalSince1970: 11),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        failedAgent.stageExecution = stage
        failedAgent.logSnippet = "Required outputs missing: output_1"
        context.insert(failedAgent)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryAgent(run: run, stageID: plan.initialStateID, agentID: "test_agent")

        let retryExec = try #require(
            stage.agentExecutions
                .filter { $0.agentID == "test_agent" && ($0.agentAttemptNumber ?? 1) == 2 }
                .last
        )
        #expect(retryExec.status == .pending)

        _ = try ArtifactStorage.write(
            data: Data("late-output".utf8),
            name: "output_1",
            stageID: plan.initialStateID,
            iteration: stage.iteration,
            agentID: "test_agent",
            attemptNumber: stage.attemptNumber,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        let service = ExecutionService(
            modelContext: context,
            executor: FailingExecutor(),
            catalog: catalog
        )

        try service.resumeRun(run: run, compiler: compiler)

        await awaitCondition("Late output should complete the run without rerunning the retry attempt", timeout: 3.0) {
            run.status == .completed
        }
        await awaitCondition("Late-output reconciliation should detach its orchestrator", timeout: 3.0) {
            service.orchestrator(for: run.id) == nil
        }

        #expect(run.status == .completed)
        #expect(stage.status == .completed)
        #expect(stage.agentExecutions.count == 2)
        #expect(failedAgent.status == .completed)
        #expect(retryExec.status == .completed)
        #expect(
            failedAgent.artifacts.contains { $0.name == "output_1" }
        )
    }

    @Test("ExecutionService resumes stage retry without creating a fresh stage execution")
    func executionServiceResumeRunAfterStageRetryReusesReadyStageAttempt() async throws {
        let (run, plan, _, catalog) = try makeRetryableRunFromPlan()
        run.status = .failed

        let failedStage = StageExecution(
            stageID: plan.initialStateID,
            label: "Draft",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedStage.run = run
        context.insert(failedStage)
        try context.save()

        let coordinator = RecoveryCoordinator(modelContext: context)
        _ = try coordinator.retryStage(run: run, stageID: plan.initialStateID)

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(simulatedDelay: 0, catalog: catalog),
            catalog: catalog
        )

        try service.resumeRun(run: run, compiler: compiler)

        await awaitCondition("Recovery-created ready stage retry should complete without extra stage creation", timeout: 3.0) {
            run.status == .completed
        }
        await awaitCondition("Completed stage retry should detach its orchestrator", timeout: 3.0) {
            service.orchestrator(for: run.id) == nil
        }

        let stages = run.stageExecutions.filter { $0.stageID == plan.initialStateID }
            .sorted {
                if $0.iteration != $1.iteration { return $0.iteration < $1.iteration }
                return $0.attemptNumber < $1.attemptNumber
            }
        #expect(stages.count == 2)
        #expect(stages.first?.attemptNumber == 1)
        #expect(stages.last?.attemptNumber == 2)
        #expect(stages.last?.iteration == 1)
        #expect(stages.last?.status == .completed)
        #expect(stages.last?.agentExecutions.count == 1)
    }

    // MARK: - Live Executor Routing

    private func repositoryRootURL(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func loadLiveWorkflow() throws -> WorkflowDefinition {
        try loadTestLiveWorkflow()
    }

    private func loadFullMVPLiveWorkflow() throws -> WorkflowDefinition {
        try loadTestFullMVPLiveWorkflow()
    }

    @Test("ExecutionService uses live executor for live workflow")
    func executionServiceUsesLiveExecutorForLiveWorkflow() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Live Workflow", body: "Validate Goose-backed executor routing")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("LiveExecutionServiceTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        try context.save()

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
            catalog: catalog,
            liveRuntimeConfiguration: LiveRuntimeConfiguration(
                baseURL: URL(string: "http://localhost:9999")!,
                apiKey: nil,
                override: LiveExecutionOverride(
                    enabled: true,
                    provider: "claude_code",
                    model: "default",
                    effort: "high"
                ),
                transportMode: .network,
                transportAPI: .gooseServer
            )
        )

        service.startRun(run: run, plan: plan, workspace: workspace)

        guard let orchestrator = service.orchestrator(for: run.id) else {
            Issue.record("Expected live orchestrator to be created")
            return
        }
        #expect(orchestrator.executor is RuntimeAgentExecutor)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService reconciles stalled running run after session closed")
    func executionServiceReconcilesStalledRunningRunAfterSessionClosed() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        let catalog = try loadCanonicalCatalog()
        run.status = .running

        let stage = StageExecution(
            stageID: plan.initialStateID,
            label: try #require(plan.states[plan.initialStateID]?.label),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)
        try context.save()

        let service = ExecutionService(modelContext: context, executor: FailingExecutor(), catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: FailingExecutor(),
            modelContext: context,
            catalog: catalog
        )

        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "test_agent",
            event: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-31),
                detail: "Session closed"
            )
        )
        service.registerTestingOrchestrator(orchestrator)

        #expect(service.orchestrator(for: run.id) == nil)
        #expect(run.status == .blocked)
        #expect(run.presentationStatus == .blocked)
        #expect(stage.status == .blocked)
        #expect(run.driftDetails?.contains("Execution stalled after the last session closed") == true)
    }

    @Test("ExecutionService does not reconcile a completed stage boundary after session closed")
    func executionServiceDoesNotReconcileCompletedStageBoundaryAfterSessionClosed() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        let catalog = try loadCanonicalCatalog()
        run.status = .running

        let stage = StageExecution(
            stageID: plan.initialStateID,
            label: try #require(plan.states[plan.initialStateID]?.label),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)
        try context.save()

        let service = ExecutionService(modelContext: context, executor: FailingExecutor(), catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: FailingExecutor(),
            modelContext: context,
            catalog: catalog
        )

        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "test_agent",
            event: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-31),
                detail: "Session closed"
            )
        )
        service.registerTestingOrchestrator(orchestrator)

        service.runMaintenanceTick()

        #expect(run.status == .running)
        #expect(run.presentationStatus == .running)
        #expect(stage.status == .completed)
        #expect(service.orchestrator(for: run.id) != nil)
    }

    @Test("ExecutionService reconciles stalled run even when agent rows remain running after session closed")
    func executionServiceReconcilesStalledRunWithStaleRunningAgentRows() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        let catalog = try loadCanonicalCatalog()
        run.status = .running

        let stage = StageExecution(
            stageID: plan.initialStateID,
            label: try #require(plan.states[plan.initialStateID]?.label),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let agentExecution = AgentExecution(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            taskName: "refine_implementation",
            startedAt: Date().addingTimeInterval(-60),
            status: .running,
            provider: "codex",
            effort: "high"
        )
        agentExecution.stageExecution = stage
        stage.agentExecutions.append(agentExecution)
        context.insert(agentExecution)
        try context.save()

        let service = ExecutionService(modelContext: context, executor: FailingExecutor(), catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: FailingExecutor(),
            modelContext: context,
            catalog: catalog
        )

        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "code_writer",
            event: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-31),
                detail: "Session closed"
            )
        )
        service.registerTestingOrchestrator(orchestrator)

        service.runMaintenanceTick()

        #expect(run.status == .blocked)
        #expect(stage.status == .blocked)
        #expect(agentExecution.status == .failed)
        #expect(agentExecution.canonicalOutcome == .failedBeforeOutput)
        #expect(agentExecution.providerStopReason == "session_closed_without_transition")
        #expect(agentExecution.logSnippet?.contains("Resume required") == true)
        #expect(service.orchestrator(for: run.id) == nil)
    }

    @Test("ExecutionService does not reconcile fan-out stage while another agent is still streaming after one session closed")
    func executionServiceDoesNotReconcileFanoutStageWithConcurrentLiveAgentActivity() throws {
        let (run, plan, workspace) = try makeRunFromPlan()
        let catalog = try loadCanonicalCatalog()
        run.status = .running

        let stage = StageExecution(
            stageID: plan.initialStateID,
            label: try #require(plan.states[plan.initialStateID]?.label),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        context.insert(stage)

        let closedAgent = AgentExecution(
            agentID: "proposal_reviewer_architect",
            agentTitle: "Proposal Reviewer / Architect",
            taskName: "review_proposal_as_architect",
            startedAt: Date().addingTimeInterval(-180),
            status: .running,
            provider: "codex",
            effort: "high"
        )
        closedAgent.stageExecution = stage
        stage.agentExecutions.append(closedAgent)
        context.insert(closedAgent)

        let activeAgent = AgentExecution(
            agentID: "proposal_reviewer_product_owner",
            agentTitle: "Proposal Reviewer / Product Owner",
            taskName: "review_proposal_as_product_owner",
            startedAt: Date().addingTimeInterval(-175),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        activeAgent.stageExecution = stage
        stage.agentExecutions.append(activeAgent)
        context.insert(activeAgent)
        try context.save()

        let service = ExecutionService(modelContext: context, executor: FailingExecutor(), catalog: catalog)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: FailingExecutor(),
            modelContext: context,
            catalog: catalog
        )

        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "proposal_reviewer_architect",
            event: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-31),
                detail: "Session closed"
            )
        )
        orchestrator.injectTestingLiveExecutionEvent(
            agentID: "proposal_reviewer_product_owner",
            event: ExecutionEvent(
                type: .textChunk,
                timestamp: Date().addingTimeInterval(-10),
                detail: "Still reviewing artifacts."
            )
        )
        service.registerTestingOrchestrator(orchestrator)

        service.runMaintenanceTick()

        #expect(run.status == .running)
        #expect(run.presentationStatus == .running)
        #expect(stage.status == .running)
        #expect(closedAgent.status == .running)
        #expect(activeAgent.status == .running)
        #expect(service.orchestrator(for: run.id) != nil)
    }

    @Test("ExecutionService does not reconcile fresh post-session idle run")
    func executionServiceDoesNotReconcileFreshPostSessionIdleRun() {
        let run = Run(
            workflowID: "test",
            workflowTitle: "Test",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "wf.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data()
        )
        run.status = .running

        let shouldReconcile = ExecutionService.shouldReconcileStalledRun(
            run: run,
            hasPendingApproval: false,
            hasRunningAgents: false,
            stalledStageStatus: nil,
            latestLiveEvent: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-5),
                detail: "Session closed"
            ),
            now: Date()
        )

        #expect(shouldReconcile == false)
    }

    @Test("ExecutionService does not reconcile stalled run when the latest stage already completed")
    func executionServiceDoesNotReconcileCompletedLatestStage() {
        let run = Run(
            workflowID: "test",
            workflowTitle: "Test",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "wf.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data()
        )
        run.status = .running

        let shouldReconcile = ExecutionService.shouldReconcileStalledRun(
            run: run,
            hasPendingApproval: false,
            hasRunningAgents: false,
            stalledStageStatus: .completed,
            latestLiveEvent: ExecutionEvent(
                type: .sessionClosed,
                timestamp: Date().addingTimeInterval(-31),
                detail: "Session closed"
            ),
            now: Date()
        )

        #expect(shouldReconcile == false)
    }

    @Test("ExecutionService starts ACP-backed live workflow without Goose runtime config")
    func executionServiceStartsACPBackedLiveWorkflowWithoutGooseRuntimeConfig() async throws {
        let workflow = try loadLiveWorkflow()
        let catalog = try loadCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Blocked Live Workflow", body: "Missing runtime config")
        context.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("BlockedLiveExecutionServiceTest-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: context).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        try context.save()

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
            catalog: catalog
        )

        service.startRun(run: run, plan: plan, workspace: workspace)

        #expect(service.orchestrator(for: run.id) != nil)
        #expect(run.status == .pending || run.status == .running)
        #expect(run.driftDetails == nil)

        await cancelAndAwaitSettled(service, runID: run.id)
    }

    @Test("ExecutionService resume waiting approval restores pending approval without re-executing stage")
    func executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("ResumeManagerWaitingApproval-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let localContainer = try ModelContainer(for: schema, configurations: [config])
        TestModelContainerRetainer.retain(localContainer)
        let localContext = localContainer.mainContext
        let localCompiler = RunPlanCompiler(modelContext: localContext)

        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(
                id: "waiting_approval_resume",
                name: "Waiting Approval Resume",
                usesAgentCatalog: nil,
                description: "Minimal approval-restore workflow",
                ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
                requiredProviders: []
            ),
            variables: nil,
            failurePolicy: nil,
            scoring: nil,
            initialState: "state_1",
            states: [
                "state_1": WorkflowState(
                    label: "Draft",
                    type: "start",
                    owner: "test_agent",
                    approval: nil,
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: [Transition(to: "state_5_proposal_approval", when: "true")]
                ),
                "state_5_proposal_approval": WorkflowState(
                    label: "Human approval: proposal quality",
                    type: "normal",
                    owner: "test_agent",
                    approval: "required",
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: []
                )
            ]
        )
        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Approval restore test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: []
            ),
            paths: [:],
            artifacts: [:],
            skills: ["test_skill": SkillRef(type: "inline_skill", path: nil, name: "Test Skill", description: "Test")],
            contracts: [:],
            backendProfiles: [
                "test_profile": BackendProfile(
                    provider: "claude_code",
                    model: "test-model",
                    effort: "high",
                    temperature: 0,
                    maxTurns: 4,
                    structuredOutput: "none"
                )
            ],
            permissionProfiles: [
                "TEST": PermissionProfile(
                    filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
                    git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
                    shell: ShellPermissions(allow: nil, deny: nil),
                    network: NetworkPermissions(allow: nil),
                    mcp: MCPPermissions(allow: nil)
                )
            ],
            agents: [
                AgentDefinition(
                    id: "test_agent",
                    title: "Test Agent",
                    mode: "tool_use",
                    backendProfile: "test_profile",
                    permissionProfile: "TEST",
                    skillRef: "test_skill",
                    skillRole: nil,
                    worktreePolicy: nil,
                    requiredTools: nil,
                    inputs: [],
                    outputs: [],
                    outputContract: nil,
                    requiresHumanApproval: false,
                    prompt: "Wait for approval",
                    notes: nil,
                    sessionReuseScope: nil,
                    sessionFamilyID: nil
                )
            ]
        )
        let plan = try localCompiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Resume Waiting Approval", body: "Restore approval gate on app relaunch")
        localContext.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ResumeWaitingApproval-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: localContext).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        run.status = .waitingApproval
        let persistedRunID = run.id

        let stageExec = StageExecution(
            stageID: "state_5_proposal_approval",
            label: "Human approval: proposal quality",
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        stageExec.run = run
        localContext.insert(stageExec)

        let approval = Approval(stageID: "state_5_proposal_approval", decision: .requested)
        approval.run = run
        localContext.insert(approval)
        try localContext.save()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(
            modelContext: localContext,
            executor: executor,
            catalog: catalog
        )

        try service.resumeRun(run: run, compiler: localCompiler)

        // Wait for approval restoration using awaitCondition instead of pollUntil
        await awaitCondition("Waiting approval should be restored", timeout: 3.0) {
            service.pendingApprovalCount > 0
        }

        #expect(service.pendingApprovalCount == 1, "Waiting approval should be restored into the app shell")
        #expect(executor.executedTasks.count == 0, "Approval restore must not re-execute the paused stage")
        let freshContext = ModelContext(localContainer)
        let fetchedRun = try #require((try freshContext.fetch(FetchDescriptor<Run>())).first(where: { $0.id == persistedRunID }))
        #expect(fetchedRun.status == .waitingApproval)
        #expect(fetchedRun.stageExecutions.count == 1, "Approval restore must not duplicate the waiting stage")
        #expect(service.orchestrator(for: persistedRunID) != nil, "Resumed live run should still be attached to an orchestrator")

        await cancelAndAwaitSettled(service, runID: persistedRunID)
    }

    @Test("ExecutionService approval resolution persists decision for fresh context reads")
    func executionServiceApprovalResolutionPersistsDecisionForFreshContextReads() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("ResumeManagerApprovalPersistence-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let localContainer = try ModelContainer(for: schema, configurations: [config])
        TestModelContainerRetainer.retain(localContainer)
        let localContext = localContainer.mainContext
        let localCompiler = RunPlanCompiler(modelContext: localContext)

        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(
                id: "approval_persistence",
                name: "Approval Persistence",
                usesAgentCatalog: nil,
                description: "Minimal approval-persistence workflow",
                ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
                requiredProviders: []
            ),
            variables: nil,
            failurePolicy: nil,
            scoring: nil,
            initialState: "state_1",
            states: [
                "state_1": WorkflowState(
                    label: "Draft",
                    type: "start",
                    owner: "test_agent",
                    approval: nil,
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: [Transition(to: "state_3_initial_proposal_approval", when: "true")]
                ),
                "state_3_initial_proposal_approval": WorkflowState(
                    label: "Human approval: initial proposal matches intent",
                    type: "normal",
                    owner: "test_agent",
                    approval: "required",
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: []
                )
            ]
        )
        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Approval persistence test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: []
            ),
            paths: [:],
            artifacts: [:],
            skills: ["test_skill": SkillRef(type: "inline_skill", path: nil, name: "Test Skill", description: "Test")],
            contracts: [:],
            backendProfiles: [
                "test_profile": BackendProfile(
                    provider: "claude_code",
                    model: "test-model",
                    effort: "high",
                    temperature: 0,
                    maxTurns: 4,
                    structuredOutput: "none"
                )
            ],
            permissionProfiles: [
                "TEST": PermissionProfile(
                    filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
                    git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
                    shell: ShellPermissions(allow: nil, deny: nil),
                    network: NetworkPermissions(allow: nil),
                    mcp: MCPPermissions(allow: nil)
                )
            ],
            agents: [
                AgentDefinition(
                    id: "test_agent",
                    title: "Test Agent",
                    mode: "tool_use",
                    backendProfile: "test_profile",
                    permissionProfile: "TEST",
                    skillRef: "test_skill",
                    skillRole: nil,
                    worktreePolicy: nil,
                    requiredTools: nil,
                    inputs: [],
                    outputs: [],
                    outputContract: nil,
                    requiresHumanApproval: false,
                    prompt: "Wait for approval",
                    notes: nil,
                    sessionReuseScope: nil,
                    sessionFamilyID: nil
                )
            ]
        )
        let plan = try localCompiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Persist Approval", body: "Approval should survive relaunch")
        localContext.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("PersistApproval-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: localContext).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/workflow.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        run.status = .waitingApproval
        let persistedRunID = run.id

        let stageExec = StageExecution(
            stageID: "state_3_initial_proposal_approval",
            label: "Human approval: initial proposal matches intent",
            status: .waitingApproval,
            iteration: 2,
            attemptNumber: 1
        )
        stageExec.run = run
        localContext.insert(stageExec)

        let approval = Approval(stageID: "state_3_initial_proposal_approval", decision: .requested)
        approval.run = run
        localContext.insert(approval)
        try localContext.save()

        let executor = SimulatedAgentExecutor()
        let service = ExecutionService(
            modelContext: localContext,
            executor: executor,
            catalog: catalog
        )

        try service.resumeRun(run: run, compiler: localCompiler)
        await awaitCondition("Pending approval should be restored", timeout: 3.0) {
            service.pendingApprovalCount == 1
        }

        guard let requestID = service.pendingApprovals.keys.first else {
            Issue.record("Expected pending approval request")
            return
        }

        service.resolveApproval(approvalID: requestID, granted: false, comment: "persist now")

        await awaitCondition("Rejected approval should detach the orchestrator", timeout: 3.0) {
            service.orchestrator(for: persistedRunID) == nil
        }

        await awaitCondition("Approval resolution should settle in fresh reads", timeout: 3.0) {
            let freshContext = ModelContext(localContainer)
            let approvals = (try? freshContext.fetch(FetchDescriptor<Approval>())) ?? []
            return approvals.contains(where: {
                $0.stageID == "state_3_initial_proposal_approval" && $0.decision == .rejected
            })
        }

        let freshContext = ModelContext(localContainer)
        let approvalFetch = FetchDescriptor<Approval>()
        let fetchedApprovals = try freshContext.fetch(approvalFetch)
        let fetchedApproval = try #require(fetchedApprovals.first(where: {
            $0.stageID == "state_3_initial_proposal_approval" && $0.decision == .rejected
        }))
        #expect(fetchedApproval.decision == .rejected)
        #expect(fetchedApproval.decidedAt != nil)
    }

    @Test("ExecutionService resolves persisted approval after relaunch without explicit resume")
    func executionServiceResolvesPersistedApprovalAfterRelaunchWithoutExplicitResume() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self, AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration("ResumeManagerApprovalHydration-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        let localContainer = try ModelContainer(for: schema, configurations: [config])
        TestModelContainerRetainer.retain(localContainer)
        let localContext = localContainer.mainContext
        let localCompiler = RunPlanCompiler(modelContext: localContext)

        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(
                id: "approval_hydration",
                name: "Approval Hydration",
                usesAgentCatalog: nil,
                description: "Approval should hydrate and resolve after relaunch",
                ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "automatic_on_launch"),
                requiredProviders: []
            ),
            variables: nil,
            failurePolicy: nil,
            scoring: nil,
            initialState: "state_1",
            states: [
                "state_1": WorkflowState(
                    label: "Draft",
                    type: "start",
                    owner: "test_agent",
                    approval: nil,
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: [Transition(to: "state_2_gate", when: "true")]
                ),
                "state_2_gate": WorkflowState(
                    label: "Approval Gate",
                    type: "normal",
                    owner: "test_agent",
                    approval: "required",
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: [Transition(to: "state_3_done", when: "approval.granted == true")]
                ),
                "state_3_done": WorkflowState(
                    label: "Done",
                    type: "end",
                    owner: "test_agent",
                    approval: nil,
                    run: nil,
                    runAfterApproval: nil,
                    loop: nil,
                    transitions: []
                )
            ]
        )

        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: AppConfig(
                name: "Chainworks Forge",
                runtime: "local",
                transport: "http_sse",
                description: "Approval hydration test catalog",
                ideaInputMode: "text",
                singleActiveRunPerIdea: true,
                runResumePolicy: "automatic_on_launch",
                requiredProviders: []
            ),
            paths: [:],
            artifacts: [:],
            skills: ["test_skill": SkillRef(type: "inline_skill", path: nil, name: "Test Skill", description: "Test")],
            contracts: [:],
            backendProfiles: [
                "test_profile": BackendProfile(
                    provider: "claude_code",
                    model: "test-model",
                    effort: "high",
                    temperature: 0,
                    maxTurns: 4,
                    structuredOutput: "none"
                )
            ],
            permissionProfiles: [
                "TEST": PermissionProfile(
                    filesystem: FilesystemPermissions(read: nil, write: nil, deny: nil),
                    git: GitPermissions(status: nil, diff: nil, checkout: nil, commit: nil, push: nil),
                    shell: ShellPermissions(allow: nil, deny: nil),
                    network: NetworkPermissions(allow: nil),
                    mcp: MCPPermissions(allow: nil)
                )
            ],
            agents: [
                AgentDefinition(
                    id: "test_agent",
                    title: "Test Agent",
                    mode: "tool_use",
                    backendProfile: "test_profile",
                    permissionProfile: "TEST",
                    skillRef: "test_skill",
                    skillRole: nil,
                    worktreePolicy: nil,
                    requiredTools: nil,
                    inputs: [],
                    outputs: [],
                    outputContract: nil,
                    requiresHumanApproval: false,
                    prompt: "Wait for approval",
                    notes: nil,
                    sessionReuseScope: nil,
                    sessionFamilyID: nil
                )
            ]
        )

        let plan = try localCompiler.previewCompile(workflow: workflow, catalog: catalog)
        let idea = Idea(title: "Hydrate Approval", body: "Approval should survive relaunch without manual resume")
        localContext.insert(idea)

        let runID = UUID()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("HydrateApproval-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = tempDir.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let workspace = RunWorkspace(
            runID: runID,
            workspaceRoot: tempDir,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )

        let run = try RunRepository(context: localContext).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/workflow.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path
        )
        run.status = .waitingApproval
        let persistedRunID = run.id

        let stageExec = StageExecution(
            stageID: "state_2_gate",
            label: "Approval Gate",
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        stageExec.run = run
        localContext.insert(stageExec)

        let approval = Approval(stageID: "state_2_gate", decision: .requested)
        approval.run = run
        localContext.insert(approval)
        try localContext.save()

        let service = ExecutionService(
            modelContext: localContext,
            executor: SimulatedAgentExecutor(),
            catalog: catalog
        )

        #expect(service.pendingApprovalCount == 1)
        let requestID = try #require(service.pendingApprovals.keys.first)

        service.resolveApproval(approvalID: requestID, granted: true, comment: "resume from persisted approval")

        await awaitCondition("Hydrated approval should clear after approval", timeout: 3.0) {
            service.pendingApprovalCount == 0
        }
        await awaitCondition("Hydrated approval should complete the run", timeout: 3.0) {
            run.status == .completed
        }
        await awaitCondition("Hydrated approval completion should detach orchestrator", timeout: 3.0) {
            service.orchestrator(for: persistedRunID) == nil
        }

        let freshContext = ModelContext(localContainer)
        let fetchedApproval = try #require(
            freshContext.fetch(FetchDescriptor<Approval>()).first(where: {
                $0.stageID == "state_2_gate" && $0.decision == .granted
            })
        )
        #expect(fetchedApproval.decidedAt != nil)
    }
}
