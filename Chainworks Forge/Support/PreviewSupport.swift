import Foundation
import SwiftData
import SwiftUI

enum PreviewSupport {
    static let schema = Schema([
        Idea.self,
        Run.self,
        StageExecution.self,
        AgentExecution.self,
        Approval.self,
        Artifact.self
    ])

    @MainActor
    static func makeModelContainer(seed: ((ModelContext) -> Void)? = nil) -> ModelContainer {
        let configuration = ModelConfiguration(isStoredInMemoryOnly: true)
        let container = try! ModelContainer(for: schema, configurations: [configuration])
        if let seed {
            seed(container.mainContext)
            try? container.mainContext.save()
        }
        return container
    }

    @MainActor
    static func makeAppConfigurationStore() -> AppConfigurationStore {
        AppConfigurationStore(
            fileURL: FileManager.default.temporaryDirectory.appendingPathComponent("preview-app-configuration.json"),
            initialConfiguration: AppConfiguration(
                runStorageBasePath: "/Users/user/Library/Application Support/Chainworks Forge/runs",
                worktreeBasePath: "/Users/user/Library/Application Support/Chainworks Forge/worktrees",
                workflowSourcePath: repoExampleURL("workflows/workflow.yaml").path,
                agentCatalogSourcePath: repoExampleURL("agents/agents.yaml").path,
                supportBundleExportPath: "/Users/user/Library/Application Support/Chainworks Forge/exports",
                gooseServerHost: "127.0.0.1",
                gooseServerPort: 51200,
                gooseServerTLS: true,
                gooseServerAutostart: true,
                gooseServerBinaryPath: "/Applications/Goose.app/Contents/Resources/bin/goosed",
                gooseServerSecretKey: "preview-secret",
                activeConfigurationSource: .persistedSettings
            )
        )
    }

    @MainActor
    static func makeProviderSettingsStore() -> ProviderSettingsStore {
        let claudeProvider = ConfiguredProvider(
            family: .claude,
            displayName: "Claude Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .apiKey,
            defaultModel: "claude-opus-4"
        )
        let codexProvider = ConfiguredProvider(
            family: .codex,
            displayName: "Codex via Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .apiKey,
            defaultModel: "gpt-5-codex"
        )
        let geminiProvider = ConfiguredProvider(
            family: .gemini,
            displayName: "Gemini HTTP",
            transport: .httpAPI,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gemini-2.5-pro"
        )

        return ProviderSettingsStore(
            fileURL: FileManager.default.temporaryDirectory.appendingPathComponent("preview-provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [claudeProvider, codexProvider, geminiProvider],
                preferredProviderIDsByFamily: [
                    claudeProvider.family.rawValue: claudeProvider.id,
                    codexProvider.family.rawValue: codexProvider.id,
                    geminiProvider.family.rawValue: geminiProvider.id
                ],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        )
    }

    @MainActor
    static func makeProviderRegistry(settingsStore: ProviderSettingsStore) -> ProviderRegistry {
        ProviderRegistry(
            settingsStore: settingsStore,
            secretStore: KeychainSecretStore(useInMemoryStore: true)
        )
    }

    @MainActor
    static func makeExecutionService(
        modelContext: ModelContext,
        liveConfigured: Bool = true
    ) -> ExecutionService {
        let runtimeConfiguration = liveConfigured ? LiveRuntimeConfiguration(
            baseURL: URL(string: "https://127.0.0.1:51200")!,
            apiKey: "preview-secret",
            override: nil,
            transportMode: .network,
            transportAPI: .gooseServer
        ) : nil

        return ExecutionService(
            modelContext: modelContext,
            executor: SimulatedAgentExecutor(),
            liveRuntimeConfiguration: runtimeConfiguration,
            notificationService: NotificationService()
        )
    }

    @MainActor
    static func seedOperatorData(context: ModelContext) {
        let now = Date()

        let draftIdea = Idea(
            title: "Refine onboarding flow",
            body: "Simplify first-run setup and remove dead-end configuration states.",
            status: .draft
        )

        let activeIdea = Idea(
            title: "Provider troubleshooting",
            body: "Show why Codex and Claude fail in-app even when Goose is reachable.",
            attachmentPath: "/Users/user/Documents/specs/provider-troubleshooting.md",
            status: .active
        )

        let waitingRun = makeRun(
            idea: activeIdea,
            title: "Proposal Loop (Live)",
            status: .waitingApproval,
            trust: "server_verified",
            stageLabel: "Initial proposal approval",
            agentTitle: "Lead / Orchestrator",
            provider: "claude_code",
            model: "claude-opus-4"
        )

        let blockedIdea = Idea(
            title: "Delivery dry run",
            body: "Validate repo-backed release checkpoint and receipts.",
            status: .active
        )

        let blockedRun = makeRun(
            idea: blockedIdea,
            title: "Full MVP Live",
            status: .blocked,
            trust: "server_unverified",
            stageLabel: "Manual release",
            agentTitle: "Release Coordinator",
            provider: "codex",
            model: "gpt-5-codex"
        )

        let completedIdea = Idea(
            title: "Archive finished ideas",
            body: "Allow archive when run is terminal or idea never started.",
            status: .completed
        )

        let completedRun = makeRun(
            idea: completedIdea,
            title: "Canonical Workflow",
            status: .completed,
            trust: "fixture_verified",
            stageLabel: "Workflow complete",
            agentTitle: "Proposal Writer",
            provider: "gemini",
            model: "gemini-2.5-pro"
        )

        let archivedIdea = Idea(
            title: "Retire old provider plan",
            body: "Archive completed work once the current operator path is fully adopted.",
            status: .completed
        )
        archivedIdea.archivedAt = now.addingTimeInterval(-86_400)

        let archivedRun = makeRun(
            idea: archivedIdea,
            title: "Archived Delivery",
            status: .completed,
            trust: "fixture_verified",
            stageLabel: "Archived deliverable",
            agentTitle: "Release Coordinator",
            provider: "claude_code",
            model: "claude-opus-4"
        )

        [draftIdea, activeIdea, blockedIdea, completedIdea, archivedIdea].forEach { context.insert($0) }
        [waitingRun, blockedRun, completedRun, archivedRun].forEach { context.insert($0) }
    }

    @MainActor
    static func makeRun(
        idea: Idea,
        title: String,
        status: RunStatus,
        trust: String,
        stageLabel: String,
        agentTitle: String,
        provider: String,
        model: String
    ) -> Run {
        let now = Date()
        let previewState = previewStateMapping(for: status)
        let snapshotBundle = previewSnapshots()
        let run = Run(
            startedAt: now.addingTimeInterval(-2400),
            status: status,
            workflowID: title.lowercased().replacingOccurrences(of: " ", with: "_"),
            workflowTitle: title,
            workflowSnapshotHash: snapshotBundle?.workflowHash ?? "workflow-hash-\(title)",
            catalogSnapshotHash: snapshotBundle?.catalogHash ?? "catalog-hash-\(title)",
            workflowSourcePath: repoExampleURL("workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repoExampleURL("agents/agents.yaml").path,
            workflowSnapshotJSON: snapshotBundle?.workflowData ?? Data("workflow".utf8),
            catalogSnapshotJSON: snapshotBundle?.catalogData ?? Data("catalog".utf8),
            workspaceRoot: "/tmp/\(UUID().uuidString)",
            artifactRoot: "/tmp/\(UUID().uuidString)/artifacts",
            planCompilerVersion: 1
        )
        run.idea = idea
        idea.runs.append(run)
        run.totalCostCents = status == .completed ? 183 : 67
        run.runtimeTrustLevel = trust
        if status == .completed {
            run.completedAt = now.addingTimeInterval(-300)
        }

        let stage = StageExecution(
            stageID: previewState.stageID,
            label: stageLabel,
            startedAt: now.addingTimeInterval(-1800),
            status: previewState.stageStatus
        )
        stage.run = run
        if previewState.stageStatus == .completed {
            stage.completedAt = now.addingTimeInterval(-1200)
        }
        run.stageExecutions.append(stage)

        let agent = AgentExecution(
            agentID: previewState.agentID,
            agentTitle: agentTitle,
            taskName: previewState.taskName,
            startedAt: now.addingTimeInterval(-1700),
            status: previewState.agentStatus,
            provider: provider,
            effort: "high"
        )
        agent.resolvedModel = model
        agent.logSnippet = previewState.logSnippet
        agent.stageExecution = stage
        if previewState.agentStatus == .completed {
            agent.completedAt = now.addingTimeInterval(-1500)
        }
        stage.agentExecutions.append(agent)

        if status == .waitingApproval {
            let approval = Approval(stageID: stage.stageID, requestedAt: now.addingTimeInterval(-600), decision: .requested)
            approval.run = run
            run.approvals.append(approval)
        }

        return run
    }

    private static func previewSnapshots() -> (
        workflowData: Data,
        catalogData: Data,
        workflowHash: String,
        catalogHash: String
    )? {
        let workflowURL = repoExampleURL("workflows/proposal-loop-live.yaml")
        let catalogURL = repoExampleURL("agents/agents.yaml")

        guard
            FileManager.default.isReadableFile(atPath: workflowURL.path),
            FileManager.default.isReadableFile(atPath: catalogURL.path),
            let workflow = try? YAMLParser.loadWorkflow(from: workflowURL),
            let catalog = try? YAMLParser.loadAgentCatalog(from: catalogURL)
        else {
            return nil
        }

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]

        guard
            let workflowData = try? encoder.encode(workflow),
            let catalogData = try? encoder.encode(catalog),
            let workflowHashResult = try? DefinitionHasher.hash(workflow),
            let catalogHashResult = try? DefinitionHasher.hash(catalog)
        else {
            return nil
        }

        return (workflowData, catalogData, workflowHashResult.sha256, catalogHashResult.sha256)
    }

    private struct PreviewStateMapping {
        let stageID: String
        let stageStatus: StageStatus
        let agentID: String
        let taskName: String
        let agentStatus: AgentStatus
        let logSnippet: String?
    }

    private static func previewStateMapping(for status: RunStatus) -> PreviewStateMapping {
        switch status {
        case .pending, .ready:
            return PreviewStateMapping(
                stageID: "state_1_idea_received",
                stageStatus: mapStageStatus(from: status),
                agentID: "lead_orchestrator",
                taskName: "normalize_idea_and_prepare_proposal_brief",
                agentStatus: mapAgentStatus(from: status),
                logSnippet: "Preparing the proposal brief."
            )
        case .running:
            return PreviewStateMapping(
                stageID: "state_3_proposal_reviewed",
                stageStatus: .running,
                agentID: "proposal_reviewer_ux",
                taskName: "review_proposal_from_ux_perspective",
                agentStatus: .running,
                logSnippet: "Review in progress."
            )
        case .waitingApproval:
            return PreviewStateMapping(
                stageID: "state_5_proposal_approval",
                stageStatus: .waitingApproval,
                agentID: "lead_orchestrator",
                taskName: "aggregate_proposal_reviews",
                agentStatus: .pending,
                logSnippet: "Waiting on human approval."
            )
        case .blocked:
            return PreviewStateMapping(
                stageID: "state_4_proposal_refined",
                stageStatus: .blocked,
                agentID: "proposal_writer",
                taskName: "refine_proposal_based_on_review",
                agentStatus: .pending,
                logSnippet: "Blocked pending upstream decision."
            )
        case .completed:
            return PreviewStateMapping(
                stageID: "state_6_workflow_complete",
                stageStatus: .completed,
                agentID: "lead_orchestrator",
                taskName: "aggregate_proposal_reviews",
                agentStatus: .completed,
                logSnippet: "Workflow finished."
            )
        case .failed:
            return PreviewStateMapping(
                stageID: "state_4_proposal_refined",
                stageStatus: .failed,
                agentID: "proposal_writer",
                taskName: "refine_proposal_based_on_review",
                agentStatus: .failed,
                logSnippet: "Refinement failed."
            )
        case .cancelled:
            return PreviewStateMapping(
                stageID: "state_6_workflow_complete",
                stageStatus: .skipped,
                agentID: "lead_orchestrator",
                taskName: "aggregate_proposal_reviews",
                agentStatus: .cancelled,
                logSnippet: "Run cancelled."
            )
        case .cancelling:
            return PreviewStateMapping(
                stageID: "state_3_proposal_reviewed",
                stageStatus: .running,
                agentID: "proposal_reviewer_ux",
                taskName: "review_proposal_from_ux_perspective",
                agentStatus: .running,
                logSnippet: "Cancellation in progress\u{2026}"
            )
        }
    }

    @MainActor
    static func seedWorkflowMapPreviewData(context: ModelContext) {
        let workflowURL = repoExampleURL("workflows/proposal-loop-live.yaml")
        let catalogURL = repoExampleURL("agents/agents.yaml")

        guard
            FileManager.default.isReadableFile(atPath: workflowURL.path),
            FileManager.default.isReadableFile(atPath: catalogURL.path)
        else {
            return
        }

        let workflow = try! YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try! YAMLParser.loadAgentCatalog(from: catalogURL)
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try! compiler.previewCompile(workflow: workflow, catalog: catalog)

        let providerSettingsStore = makeProviderSettingsStore()
        let providerRegistry = makeProviderRegistry(settingsStore: providerSettingsStore)
        let bindings = try! BackendProfileResolverV2(providerRegistry: providerRegistry).resolveBindings(
            plan: plan,
            startOptions: .empty
        )

        let idea = Idea(
            title: "Workflow map preview",
            body: "Preview data used to render workflow topology, handoffs, agent panels, and loop telemetry.",
            status: .active
        )
        context.insert(idea)

        let run = Run(
            startedAt: Date().addingTimeInterval(-5400),
            status: .running,
            loopCounters: ["proposal_revision_count": 1],
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: "/tmp/\(UUID().uuidString)",
            artifactRoot: "/tmp/\(UUID().uuidString)/artifacts",
            planCompilerVersion: plan.planCompilerVersion
        )
        run.idea = idea
        idea.runs.append(run)
        run.runtimeTrustLevel = "server_verified"
        run.totalCostCents = 248
        run.providerBindingSnapshotJSON = encodeProviderBindings(bindings)
        run.startOptionsJSON = encodeStartOptions(.empty)
        context.insert(run)

        seedWorkflowMapStages(into: run)
        try? context.save()
    }

    @MainActor
    private static func seedWorkflowMapStages(into run: Run) {
        let now = Date()

        let stage1 = StageExecution(
            stageID: "state_1_idea_received",
            label: "Idea received",
            startedAt: now.addingTimeInterval(-5000),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage1.run = run
        stage1.completedAt = now.addingTimeInterval(-4800)

        let lead = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "normalize_idea_and_prepare_proposal_brief",
            startedAt: now.addingTimeInterval(-4980),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        lead.resolvedModel = "claude-opus-4"
        lead.logSnippet = "Normalized idea and prepared the proposal brief."
        lead.stageExecution = stage1
        lead.completedAt = now.addingTimeInterval(-4900)
        stage1.agentExecutions.append(lead)

        let stage2 = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: now.addingTimeInterval(-4700),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage2.run = run
        stage2.completedAt = now.addingTimeInterval(-4400)

        let writer = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: now.addingTimeInterval(-4680),
            status: .completed,
            provider: "codex",
            effort: "high"
        )
        writer.resolvedModel = "gpt-5-codex"
        writer.logSnippet = "Drafted the first proposal pass."
        writer.stageExecution = stage2
        writer.completedAt = now.addingTimeInterval(-4500)
        stage2.agentExecutions.append(writer)

        let stage3 = StageExecution(
            stageID: "state_3_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: now.addingTimeInterval(-4300),
            status: .running,
            iteration: 2,
            attemptNumber: 1
        )
        stage3.run = run

        let reviewerPO = AgentExecution(
            agentID: "proposal_reviewer_product_owner",
            agentTitle: "Proposal Reviewer / Product Owner",
            taskName: "review_proposal_from_product_perspective",
            startedAt: now.addingTimeInterval(-4260),
            status: .completed,
            provider: "claude_code",
            effort: "medium"
        )
        reviewerPO.resolvedModel = "claude-opus-4"
        reviewerPO.logSnippet = "Product-owner review completed with approval."
        reviewerPO.stageExecution = stage3
        reviewerPO.completedAt = now.addingTimeInterval(-4200)
        stage3.agentExecutions.append(reviewerPO)

        let reviewerUX = AgentExecution(
            agentID: "proposal_reviewer_ux",
            agentTitle: "Proposal Reviewer / UX",
            taskName: "review_proposal_from_ux_perspective",
            startedAt: now.addingTimeInterval(-4240),
            status: .running,
            provider: "gemini",
            effort: "medium"
        )
        reviewerUX.resolvedModel = "gemini-2.5-pro"
        reviewerUX.logSnippet = "Assessing workflow topology and operator clarity."
        reviewerUX.stageExecution = stage3
        stage3.agentExecutions.append(reviewerUX)

        let reviewerUI = AgentExecution(
            agentID: "proposal_reviewer_ui",
            agentTitle: "Proposal Reviewer / UI",
            taskName: "review_proposal_from_ui_perspective",
            startedAt: now.addingTimeInterval(-4230),
            status: .pending,
            provider: "claude_code",
            effort: "medium"
        )
        reviewerUI.resolvedModel = "claude-opus-4"
        reviewerUI.stageExecution = stage3
        stage3.agentExecutions.append(reviewerUI)

        let reviewerArchitect = AgentExecution(
            agentID: "proposal_reviewer_architect",
            agentTitle: "Proposal Reviewer / Architect",
            taskName: "review_proposal_from_architecture_perspective",
            startedAt: now.addingTimeInterval(-4250),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        reviewerArchitect.resolvedModel = "claude-opus-4"
        reviewerArchitect.logSnippet = "Confirmed runtime-derived workflow topology."
        reviewerArchitect.stageExecution = stage3
        reviewerArchitect.completedAt = now.addingTimeInterval(-4170)
        stage3.agentExecutions.append(reviewerArchitect)

        let orchestrator = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: now.addingTimeInterval(-4120),
            status: .pending,
            provider: "claude_code",
            effort: "high"
        )
        orchestrator.resolvedModel = "claude-opus-4"
        orchestrator.stageExecution = stage3
        stage3.agentExecutions.append(orchestrator)

        run.stageExecutions.append(contentsOf: [stage1, stage2, stage3])

        let approval = Approval(
            stageID: "state_5_proposal_approval",
            requestedAt: now.addingTimeInterval(-3600),
            decision: .requested
        )
        approval.run = run
        run.approvals.append(approval)
    }

    private static func encodeProviderBindings(_ bindings: [String: ResolvedProviderBinding]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(bindings)
    }

    private static func encodeStartOptions(_ options: RunStartOptions) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(options)
    }

    private static func repoExampleURL(_ relativePath: String) -> URL {
        let trimmedPath = relativePath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let fileManager = FileManager.default
        let sourceRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()

        let candidateRoots: [URL] = [
            URL(fileURLWithPath: "/Users/user/Documents/Chainworks Forge", isDirectory: true),
            URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true),
            Bundle.main.bundleURL,
            Bundle.main.bundleURL.deletingLastPathComponent(),
            sourceRoot
        ]

        for root in candidateRoots {
            let directCandidate = root.appendingPathComponent(trimmedPath)
            if fileManager.isReadableFile(atPath: directCandidate.path) {
                return directCandidate
            }

            let examplesCandidate = root
                .appendingPathComponent("examples", isDirectory: true)
                .appendingPathComponent(trimmedPath)
            if fileManager.isReadableFile(atPath: examplesCandidate.path) {
                return examplesCandidate
            }
        }

        return URL(fileURLWithPath: "/Users/user/Documents/Chainworks Forge/examples", isDirectory: true)
            .appendingPathComponent(trimmedPath)
    }

    private static func mapStageStatus(from status: RunStatus) -> StageStatus {
        switch status {
        case .pending: return .pending
        case .ready: return .ready
        case .running: return .running
        case .waitingApproval: return .waitingApproval
        case .blocked: return .blocked
        case .completed: return .completed
        case .failed: return .failed
        case .cancelled: return .skipped
        case .cancelling: return .running
        }
    }

    private static func mapAgentStatus(from status: RunStatus) -> AgentStatus {
        switch status {
        case .pending: return .pending
        case .ready: return .ready
        case .running, .waitingApproval, .blocked: return .running
        case .completed: return .completed
        case .failed: return .failed
        case .cancelled: return .cancelled
        case .cancelling: return .running
        }
    }
}
